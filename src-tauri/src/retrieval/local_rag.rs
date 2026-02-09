use crate::core::settings_v2::{EmbeddingsProviderSettings, LocalRagSettings, profile_dir};
use crate::retrieval::embeddings_client;
use crate::retrieval::types::RetrievalSource;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::AppHandle;
use uuid::Uuid;
use walkdir::WalkDir;

const RAG_DB_NAME: &str = "rag_index.db";
const SUPPORTED_EXTS: &[&str] = &["txt", "md", "pdf"];

#[derive(Debug, Clone, Serialize)]
pub struct LocalRagSourceRecord {
    pub id: String,
    pub path: String,
    pub kind: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalRagStats {
    pub sqlite_vec_available: bool,
    pub sources_count: usize,
    pub documents_count: usize,
    pub chunks_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalRagIndexResult {
    pub source_id: String,
    pub files_processed: usize,
    pub chunks_inserted: usize,
    pub skipped_unchanged: usize,
}

#[derive(Debug, Clone)]
struct ParsedDocument {
    path: String,
    mtime: i64,
    size: i64,
    sha256: String,
    chunks: Vec<String>,
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn rag_db_path(app: &AppHandle) -> Result<PathBuf, String> {
    let profile = profile_dir(app)?;
    fs::create_dir_all(&profile).map_err(|e| format!("Failed to create profile dir: {e}"))?;
    Ok(profile.join(RAG_DB_NAME))
}

#[allow(clippy::missing_transmute_annotations)]
fn register_sqlite_vec() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    });
}

fn open_conn(app: &AppHandle) -> Result<Connection, String> {
    register_sqlite_vec();
    let path = rag_db_path(app)?;
    let conn = Connection::open(path).map_err(|e| format!("Failed to open rag db: {e}"))?;
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        CREATE TABLE IF NOT EXISTS rag_meta (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sources (
          id TEXT PRIMARY KEY,
          path TEXT NOT NULL UNIQUE,
          kind TEXT NOT NULL,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS documents (
          id TEXT PRIMARY KEY,
          source_id TEXT NOT NULL,
          path TEXT NOT NULL UNIQUE,
          mtime INTEGER NOT NULL,
          size INTEGER NOT NULL,
          sha256 TEXT NOT NULL,
          updated_at INTEGER NOT NULL,
          FOREIGN KEY(source_id) REFERENCES sources(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS chunks (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          document_id TEXT NOT NULL,
          chunk_index INTEGER NOT NULL,
          content TEXT NOT NULL,
          token_estimate INTEGER NOT NULL DEFAULT 0,
          FOREIGN KEY(document_id) REFERENCES documents(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_documents_source ON documents(source_id);
        CREATE INDEX IF NOT EXISTS idx_chunks_document ON chunks(document_id);
        "#,
    )
    .map_err(|e| format!("Failed to init rag schema: {e}"))?;
    Ok(conn)
}

fn vec_table_exists(conn: &Connection) -> Result<bool, String> {
    let exists = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='chunk_vec' LIMIT 1",
            [],
            |_| Ok(()),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .is_some();
    Ok(exists)
}

fn ensure_vec_table(conn: &Connection, dim: usize) -> Result<(), String> {
    if dim == 0 {
        return Err("Embedding dimension must be > 0".to_string());
    }
    let existing_dim = conn
        .query_row(
            "SELECT value FROM rag_meta WHERE key='embedding_dim'",
            [],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if let Some(value) = existing_dim {
        let parsed_dim = value.parse::<usize>().unwrap_or(0);
        if parsed_dim != dim {
            return Err(format!(
                "Embedding dimension mismatch (db={parsed_dim}, provider={dim}). Clear index first."
            ));
        }
        return Ok(());
    }

    let sql =
        format!("CREATE VIRTUAL TABLE IF NOT EXISTS chunk_vec USING vec0(embedding float[{dim}])");
    conn.execute_batch(&sql)
        .map_err(|e| format!("Failed to initialize sqlite-vec table: {e}"))?;
    conn.execute(
        "INSERT OR REPLACE INTO rag_meta(key, value) VALUES('embedding_dim', ?1)",
        params![dim.to_string()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn sqlite_vec_available_now(conn: &Connection) -> bool {
    if vec_table_exists(conn).unwrap_or(false) {
        return true;
    }
    conn.execute_batch("CREATE VIRTUAL TABLE temp.__vec_probe USING vec0(embedding float[3]);")
        .is_ok()
}

fn file_ext(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
}

fn should_index(path: &Path) -> bool {
    let Some(ext) = file_ext(path) else {
        return false;
    };
    SUPPORTED_EXTS.contains(&ext.as_str())
}

fn parse_pdf_text(path: &Path) -> Result<String, String> {
    let doc = lopdf::Document::load(path).map_err(|e| format!("Failed to read pdf: {e}"))?;
    let pages = doc.get_pages().into_keys().collect::<Vec<u32>>();
    doc.extract_text(&pages)
        .map_err(|e| format!("Failed to extract pdf text: {e}"))
}

fn parse_plain_text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("Failed to read text: {e}"))
}

fn read_document_text(path: &Path) -> Result<String, String> {
    let ext = file_ext(path).unwrap_or_default();
    match ext.as_str() {
        "pdf" => parse_pdf_text(path),
        "txt" | "md" => parse_plain_text(path),
        _ => Err("Unsupported document extension".to_string()),
    }
}

fn split_chunks(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    if chunk_size == 0 {
        return Vec::new();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut index = 0usize;
    while index < chars.len() {
        let end = (index + chunk_size).min(chars.len());
        let chunk: String = chars[index..end].iter().collect();
        let trimmed = chunk.trim();
        if !trimmed.is_empty() {
            out.push(trimmed.to_string());
        }
        if end == chars.len() {
            break;
        }
        index = end.saturating_sub(overlap);
    }
    out
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

fn discover_files(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return if should_index(path) {
            vec![path.to_path_buf()]
        } else {
            Vec::new()
        };
    }

    WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
        .filter(|p| should_index(p))
        .collect()
}

async fn parse_documents(
    files: Vec<PathBuf>,
    cfg: &LocalRagSettings,
) -> Result<Vec<ParsedDocument>, String> {
    let max_size = (cfg.max_file_size_mb as u64) * 1024 * 1024;
    let mut out = Vec::new();
    for path in files {
        let cfg_local = cfg.clone();
        let parsed =
            tokio::task::spawn_blocking(move || -> Result<Option<ParsedDocument>, String> {
                let meta = fs::metadata(&path).map_err(|e| format!("Failed to stat file: {e}"))?;
                if meta.len() > max_size {
                    return Ok(None);
                }
                let bytes = fs::read(&path).map_err(|e| format!("Failed to read file: {e}"))?;
                let text = read_document_text(&path)?;
                if text.trim().is_empty() {
                    return Ok(None);
                }
                let mtime = meta
                    .modified()
                    .ok()
                    .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                let chunks = split_chunks(
                    &text,
                    cfg_local.chunk_size_chars,
                    cfg_local.chunk_overlap_chars,
                );
                Ok(Some(ParsedDocument {
                    path: path.to_string_lossy().to_string(),
                    mtime,
                    size: meta.len() as i64,
                    sha256: hash_bytes(&bytes),
                    chunks,
                }))
            })
            .await
            .map_err(|e| format!("Failed to parse document: {e}"))??;
        if let Some(doc) = parsed {
            out.push(doc);
        }
    }
    Ok(out)
}

fn upsert_source(conn: &Connection, path: &Path) -> Result<(String, bool), String> {
    let now = now_unix();
    let raw = path.to_string_lossy().to_string();
    let kind = if path.is_dir() { "directory" } else { "file" };
    if let Some(existing) = conn
        .query_row("SELECT id FROM sources WHERE path=?1", params![raw], |r| {
            r.get::<_, String>(0)
        })
        .optional()
        .map_err(|e| e.to_string())?
    {
        conn.execute(
            "UPDATE sources SET updated_at=?1 WHERE id=?2",
            params![now, existing],
        )
        .map_err(|e| e.to_string())?;
        return Ok((existing, false));
    }

    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO sources(id, path, kind, created_at, updated_at) VALUES(?1, ?2, ?3, ?4, ?4)",
        params![id, raw, kind, now],
    )
    .map_err(|e| e.to_string())?;
    Ok((id, true))
}

fn document_is_unchanged(
    conn: &Connection,
    path: &str,
    mtime: i64,
    size: i64,
    sha256: &str,
) -> Result<bool, String> {
    let row = conn
        .query_row(
            "SELECT mtime, size, sha256 FROM documents WHERE path=?1",
            params![path],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(matches!(row, Some((m, s, h)) if m == mtime && s == size && h == sha256))
}

fn delete_document_chunks(conn: &Connection, document_id: &str) -> Result<(), String> {
    let chunk_ids = {
        let mut stmt = conn
            .prepare("SELECT id FROM chunks WHERE document_id=?1")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![document_id], |r| r.get::<_, i64>(0))
            .map_err(|e| e.to_string())?;
        let mut ids = Vec::new();
        for row in rows {
            ids.push(row.map_err(|e| e.to_string())?);
        }
        ids
    };

    for chunk_id in chunk_ids {
        let _ = conn.execute("DELETE FROM chunk_vec WHERE rowid=?1", params![chunk_id]);
    }
    conn.execute(
        "DELETE FROM chunks WHERE document_id=?1",
        params![document_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn add_source(
    app: &AppHandle,
    source_path: &str,
    rag_cfg: &LocalRagSettings,
    emb_cfg: &EmbeddingsProviderSettings,
) -> Result<LocalRagIndexResult, String> {
    if !emb_cfg.is_configured() {
        return Err("Embeddings provider is not configured".to_string());
    }

    let path = PathBuf::from(source_path);
    if !path.exists() {
        return Err("Source path does not exist".to_string());
    }

    let mut conn = open_conn(app)?;
    let (source_id, _) = upsert_source(&conn, &path)?;
    let files = discover_files(&path);
    let parsed_docs = parse_documents(files, rag_cfg).await?;

    let mut prepared_docs: Vec<(ParsedDocument, Vec<Vec<f32>>)> = Vec::new();
    let mut skipped_unchanged = 0usize;

    for doc in parsed_docs {
        if document_is_unchanged(&conn, &doc.path, doc.mtime, doc.size, &doc.sha256)? {
            skipped_unchanged += 1;
            continue;
        }
        if doc.chunks.is_empty() {
            continue;
        }
        let embeddings = embeddings_client::create_embeddings(emb_cfg, &doc.chunks).await?;
        if embeddings.len() != doc.chunks.len() {
            return Err("Embeddings count mismatch for document chunks".to_string());
        }
        prepared_docs.push((doc, embeddings));
    }

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    if let Some((_, embeddings)) = prepared_docs.iter().find(|(_, e)| !e.is_empty()) {
        ensure_vec_table(&tx, embeddings[0].len())?;
    }

    let mut files_processed = 0usize;
    let mut chunks_inserted = 0usize;

    for (doc, embeddings) in prepared_docs {
        let now = now_unix();
        let existing_doc_id = tx
            .query_row(
                "SELECT id FROM documents WHERE path=?1",
                params![doc.path],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        let had_existing = existing_doc_id.is_some();
        let doc_id = existing_doc_id.unwrap_or_else(|| Uuid::new_v4().to_string());

        if had_existing {
            delete_document_chunks(&tx, &doc_id)?;
        }

        tx.execute(
            "INSERT INTO documents(id, source_id, path, mtime, size, sha256, updated_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(path) DO UPDATE SET
               source_id=excluded.source_id,
               mtime=excluded.mtime,
               size=excluded.size,
               sha256=excluded.sha256,
               updated_at=excluded.updated_at",
            params![
                doc_id, source_id, doc.path, doc.mtime, doc.size, doc.sha256, now
            ],
        )
        .map_err(|e| e.to_string())?;

        for (idx, (chunk, embedding)) in doc.chunks.iter().zip(embeddings.iter()).enumerate() {
            tx.execute(
                "INSERT INTO chunks(document_id, chunk_index, content, token_estimate)
                 VALUES(?1, ?2, ?3, ?4)",
                params![
                    doc_id,
                    idx as i64,
                    chunk,
                    chunk.chars().count().div_ceil(4) as i64
                ],
            )
            .map_err(|e| e.to_string())?;
            let chunk_id = tx.last_insert_rowid();
            let embedding_json = serde_json::to_string(embedding).map_err(|e| e.to_string())?;
            tx.execute(
                "INSERT OR REPLACE INTO chunk_vec(rowid, embedding) VALUES(?1, ?2)",
                params![chunk_id, embedding_json],
            )
            .map_err(|e| format!("Failed to insert vector embedding: {e}"))?;
            chunks_inserted += 1;
        }
        files_processed += 1;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(LocalRagIndexResult {
        source_id,
        files_processed,
        chunks_inserted,
        skipped_unchanged,
    })
}

pub fn list_sources(app: &AppHandle) -> Result<Vec<LocalRagSourceRecord>, String> {
    let conn = open_conn(app)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, path, kind, created_at, updated_at FROM sources ORDER BY updated_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(LocalRagSourceRecord {
                id: r.get(0)?,
                path: r.get(1)?,
                kind: r.get(2)?,
                created_at: r.get(3)?,
                updated_at: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub fn remove_source(app: &AppHandle, source_id: &str) -> Result<(), String> {
    let mut conn = open_conn(app)?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    let chunk_ids = {
        let mut stmt = tx
            .prepare(
                "SELECT chunks.id
                 FROM chunks
                 JOIN documents ON documents.id = chunks.document_id
                 WHERE documents.source_id = ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![source_id], |r| r.get::<_, i64>(0))
            .map_err(|e| e.to_string())?;
        let mut chunk_ids = Vec::new();
        for row in rows {
            chunk_ids.push(row.map_err(|e| e.to_string())?);
        }
        chunk_ids
    };
    for chunk_id in chunk_ids {
        let _ = tx.execute("DELETE FROM chunk_vec WHERE rowid=?1", params![chunk_id]);
    }

    tx.execute("DELETE FROM sources WHERE id=?1", params![source_id])
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

pub fn clear_index(app: &AppHandle) -> Result<(), String> {
    let conn = open_conn(app)?;
    conn.execute_batch(
        r#"
        DELETE FROM chunks;
        DELETE FROM documents;
        DELETE FROM sources;
        DELETE FROM rag_meta WHERE key='embedding_dim';
        DROP TABLE IF EXISTS chunk_vec;
        "#,
    )
    .map_err(|e| e.to_string())
}

pub fn stats(app: &AppHandle) -> Result<LocalRagStats, String> {
    let conn = open_conn(app)?;
    let sources_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM sources", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let documents_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM documents", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let chunks_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;

    Ok(LocalRagStats {
        sqlite_vec_available: sqlite_vec_available_now(&conn),
        sources_count: sources_count as usize,
        documents_count: documents_count as usize,
        chunks_count: chunks_count as usize,
    })
}

pub fn open_source_folder(app: &AppHandle, source_id: &str) -> Result<String, String> {
    let conn = open_conn(app)?;
    let source_path: String = conn
        .query_row(
            "SELECT path FROM sources WHERE id=?1",
            params![source_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let path = PathBuf::from(source_path);
    if path.is_dir() {
        return Ok(path.to_string_lossy().to_string());
    }
    let parent = path
        .parent()
        .ok_or_else(|| "source parent path is missing".to_string())?;
    Ok(parent.to_string_lossy().to_string())
}

pub async fn query_local_context(
    app: &AppHandle,
    query: &str,
    top_k: usize,
    emb_cfg: &EmbeddingsProviderSettings,
) -> Result<Vec<RetrievalSource>, String> {
    if !emb_cfg.is_configured() {
        return Ok(Vec::new());
    }
    let conn = open_conn(app)?;
    if !vec_table_exists(&conn)? {
        return Ok(Vec::new());
    }
    let vectors = embeddings_client::create_embeddings(emb_cfg, &[query.to_string()]).await?;
    let Some(query_vec) = vectors.first() else {
        return Ok(Vec::new());
    };
    let query_json = serde_json::to_string(query_vec).map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            r#"
            SELECT chunks.content, documents.path, sources.path, chunk_vec.distance
            FROM chunk_vec
            JOIN chunks ON chunks.id = chunk_vec.rowid
            JOIN documents ON documents.id = chunks.document_id
            JOIN sources ON sources.id = documents.source_id
            WHERE chunk_vec.embedding MATCH ?1
            ORDER BY chunk_vec.distance
            LIMIT ?2
            "#,
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![query_json, top_k as i64], |r| {
            let chunk: String = r.get(0)?;
            let doc_path: String = r.get(1)?;
            let source_path: String = r.get(2)?;
            let distance: f32 = r.get(3)?;
            Ok((chunk, doc_path, source_path, distance))
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        let (chunk, doc_path, source_path, distance) = row.map_err(|e| e.to_string())?;
        let score = 1.0 / (1.0 + distance.max(0.0));
        out.push(RetrievalSource {
            source_type: "local".to_string(),
            title: Path::new(&doc_path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("local document")
                .to_string(),
            url: None,
            path: Some(doc_path),
            snippet: chunk,
            score: Some(score),
        });
        let _ = source_path;
    }
    Ok(out)
}
