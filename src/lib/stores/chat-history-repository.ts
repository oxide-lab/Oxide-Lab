import { invoke } from '@tauri-apps/api/core';
import type { ChatMessage } from './chat';
import type { RetrievalSource } from '$lib/chat/types';

const DB_NAME = 'sqlite:chat_history.db';

export type DbSession = {
  id: string;
  title: string;
  model_path: string | null;
  repo_id: string | null;
  created_at: number;
  updated_at: number;
};

export type DbMessage = {
  id: number;
  session_id: string;
  role: string;
  content: string;
  thinking: string;
  sources_json: string;
  attachments_json: string;
  created_at: number;
};

type AttachmentPathRow = {
  attachments_json: string;
};

let Database: typeof import('@tauri-apps/plugin-sql').default | null = null;
let dbInstance: Awaited<ReturnType<typeof import('@tauri-apps/plugin-sql').default.load>> | null = null;

async function getDb() {
  if (dbInstance) return dbInstance;

  if (!Database) {
    const mod = await import('@tauri-apps/plugin-sql');
    Database = mod.default;
  }

  dbInstance = await Database.load(DB_NAME);
  return dbInstance;
}

function parseAttachmentPaths(rows: AttachmentPathRow[]): string[] {
  const paths: string[] = [];
  for (const row of rows) {
    try {
      const parsed = JSON.parse(row.attachments_json || '[]');
      if (!Array.isArray(parsed)) continue;
      for (const item of parsed) {
        if (item && typeof item.path === 'string' && item.path.trim()) {
          paths.push(item.path);
        }
      }
    } catch {
      // Ignore malformed rows.
    }
  }
  return Array.from(new Set(paths));
}

export function dbMessageToChatMessage(msg: DbMessage): ChatMessage {
  let sources: RetrievalSource[] = [];
  try {
    const parsed = JSON.parse(msg.sources_json || '[]');
    if (Array.isArray(parsed)) {
      sources = parsed as RetrievalSource[];
    }
  } catch {
    sources = [];
  }

  let attachments: ChatMessage['attachments'] = [];
  try {
    const parsed = JSON.parse(msg.attachments_json || '[]');
    if (Array.isArray(parsed)) {
      attachments = parsed as ChatMessage['attachments'];
    }
  } catch {
    attachments = [];
  }

  return {
    role: msg.role as 'user' | 'assistant',
    content: msg.content,
    thinking: msg.thinking || undefined,
    sources,
    attachments,
  };
}

export class ChatHistoryRepository {
  async loadSessions(): Promise<DbSession[]> {
    const db = await getDb();
    return await db.select<DbSession[]>(
      'SELECT id, title, model_path, repo_id, created_at, updated_at FROM sessions ORDER BY updated_at DESC',
    );
  }

  async createSession(
    id: string,
    title: string,
    modelPath: string | undefined,
    repoId: string | undefined,
    now: number,
  ): Promise<void> {
    const db = await getDb();
    await db.execute(
      'INSERT INTO sessions (id, title, model_path, repo_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)',
      [id, title, modelPath ?? null, repoId ?? null, now, now],
    );
  }

  async loadSessionMessages(sessionId: string): Promise<DbMessage[]> {
    const db = await getDb();
    return await db.select<DbMessage[]>(
      'SELECT id, session_id, role, content, thinking, sources_json, attachments_json, created_at FROM messages WHERE session_id = ? ORDER BY id ASC',
      [sessionId],
    );
  }

  async insertMessage(targetSessionId: string, message: ChatMessage, now: number): Promise<void> {
    const db = await getDb();
    await db.execute(
      'INSERT INTO messages (session_id, role, content, thinking, sources_json, attachments_json, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)',
      [
        targetSessionId,
        message.role,
        message.content,
        message.thinking ?? '',
        JSON.stringify(message.sources ?? []),
        JSON.stringify(message.attachments ?? []),
        now,
      ],
    );
    await db.execute('UPDATE sessions SET updated_at = ? WHERE id = ?', [now, targetSessionId]);
  }

  async updateSessionTitle(sessionId: string, title: string, now: number): Promise<void> {
    const db = await getDb();
    await db.execute('UPDATE sessions SET title = ?, updated_at = ? WHERE id = ?', [title, now, sessionId]);
  }

  async touchSession(sessionId: string, now: number): Promise<void> {
    const db = await getDb();
    await db.execute('UPDATE sessions SET updated_at = ? WHERE id = ?', [now, sessionId]);
  }

  async updateLastMessage(
    sessionId: string,
    content: string,
    thinking: string,
    sources: RetrievalSource[],
  ): Promise<void> {
    const db = await getDb();
    await db.execute(
      `UPDATE messages SET content = ?, thinking = ?, sources_json = ?
       WHERE id = (SELECT MAX(id) FROM messages WHERE session_id = ?)`,
      [content, thinking, JSON.stringify(sources), sessionId],
    );
  }

  async truncateMessages(sessionId: string, keepCount: number): Promise<void> {
    const db = await getDb();
    await db.execute(
      `DELETE FROM messages
       WHERE session_id = ?
       AND id NOT IN (
           SELECT id FROM messages
           WHERE session_id = ?
           ORDER BY id ASC
           LIMIT ?
       )`,
      [sessionId, sessionId, keepCount],
    );
  }

  async deleteSession(sessionId: string): Promise<string[]> {
    const db = await getDb();
    await db.execute('BEGIN IMMEDIATE');
    try {
      const rows = await db.select<AttachmentPathRow[]>(
        'SELECT attachments_json FROM messages WHERE session_id = ?',
        [sessionId],
      );
      await db.execute('DELETE FROM sessions WHERE id = ?', [sessionId]);
      await db.execute('COMMIT');
      return parseAttachmentPaths(rows);
    } catch (error) {
      try {
        await db.execute('ROLLBACK');
      } catch {
        // Ignore rollback errors; propagate original failure.
      }
      throw error;
    }
  }

  async clearAll(): Promise<string[]> {
    const db = await getDb();
    const rows = await db.select<AttachmentPathRow[]>('SELECT attachments_json FROM messages', []);
    await db.execute('DELETE FROM sessions');
    return parseAttachmentPaths(rows);
  }

  async cleanupAttachmentPaths(paths: string[]): Promise<void> {
    if (!paths.length) return;
    await invoke('delete_chat_attachment_files', { paths });
  }
}

export const chatHistoryRepository = new ChatHistoryRepository();
