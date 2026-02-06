//! Adapted from Jan's tauri-plugin-llamacpp process lifecycle patterns (Apache-2.0).
//! Source references:
//! - example/clients/jan/src-tauri/plugins/tauri-plugin-llamacpp/src/commands.rs
//! - example/clients/jan/src-tauri/plugins/tauri-plugin-llamacpp/src/process.rs

use super::args::LlamaArgumentBuilder;
use super::state::{LlamaCppState, LlamaSession, SessionInfo, SessionKey};
use crate::core::types::{LlamaRuntimeConfig, LlamaSessionKind};
use rand::Rng;
use std::collections::HashSet;
use std::env;
use std::io::{BufRead, BufReader, Read};
use std::net::TcpListener;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
#[cfg(windows)]
use windows_sys::Win32::System::Threading::{CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW};

const START_RETRIES: usize = 5;
const HEALTH_POLL_INTERVAL_MS: u64 = 150;
const GRACEFUL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
const PROCESS_WAIT_POLL: Duration = Duration::from_millis(50);

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn key(model_id: &str, kind: LlamaSessionKind) -> SessionKey {
    SessionKey {
        model_id: model_id.to_string(),
        kind,
    }
}

fn random_port() -> Result<u16, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| e.to_string())?;
    let addr = listener.local_addr().map_err(|e| e.to_string())?;
    Ok(addr.port())
}

fn generate_api_key(model_id: &str) -> String {
    let mut rng = rand::rng();
    let salt: u64 = rng.random();
    format!("oxide-{}-{:x}", model_id.replace(['/', '\\'], "_"), salt)
}

fn resolve_from_path(path: &str) -> Option<PathBuf> {
    let p = PathBuf::from(path);
    if p.exists() && p.is_file() {
        return Some(p);
    }
    None
}

fn bin_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Ok(root_var) = env::var("OXIDE_LLAMA_BIN_ROOT") {
        for p in env::split_paths(&root_var) {
            if p.exists() && p.is_dir() {
                roots.push(p);
            }
        }
    }

    if let Ok(cwd) = env::current_dir() {
        let p = cwd.join("example").join("bin");
        if p.exists() && p.is_dir() {
            roots.push(p);
        }
    }

    let repo_bin = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("example")
        .join("bin");
    if repo_bin.exists() && repo_bin.is_dir() {
        roots.push(repo_bin);
    }

    let mut uniq = Vec::new();
    for r in roots {
        if !uniq.contains(&r) {
            uniq.push(r);
        }
    }
    uniq
}

fn bundle_candidate_score(cfg: &LlamaRuntimeConfig, dir_name: &str) -> Option<i32> {
    let n = dir_name.to_ascii_lowercase();

    let os_match = if cfg!(target_os = "windows") {
        n.contains("win")
    } else if cfg!(target_os = "macos") {
        n.contains("macos")
    } else if cfg!(target_os = "linux") {
        n.contains("ubuntu") || n.contains("linux")
    } else {
        false
    };
    if !os_match {
        return None;
    }

    let prefers_gpu = cfg.n_gpu_layers > 0;
    let flavor_score = if n.contains("cuda") {
        if prefers_gpu { 300 } else { 100 }
    } else if n.contains("vulkan") {
        if prefers_gpu { 250 } else { 150 }
    } else if n.contains("cpu") {
        if prefers_gpu { 200 } else { 300 }
    } else {
        120
    };

    // Avoid duplicate dump dirs like "... (1)" when original is present.
    let duplicate_penalty = if n.contains(" (") { -50 } else { 0 };
    Some(1000 + flavor_score + duplicate_penalty)
}

fn find_binaries_in_bundle(cfg: &LlamaRuntimeConfig) -> Vec<PathBuf> {
    let names: &[&str] = if cfg!(windows) {
        &["llama-server.exe", "llama-server"]
    } else {
        &["llama-server"]
    };

    let mut ranked: Vec<(i32, PathBuf)> = Vec::new();
    for root in bin_roots() {
        for name in names {
            let direct = root.join(name);
            if direct.exists() && direct.is_file() {
                ranked.push((900, direct));
            }
        }

        let entries = match std::fs::read_dir(&root) {
            Ok(v) => v,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let dir_name = match path.file_name().and_then(|s| s.to_str()) {
                Some(s) => s,
                None => continue,
            };
            let score = match bundle_candidate_score(cfg, dir_name) {
                Some(s) => s,
                None => continue,
            };
            for name in names {
                let candidate = path.join(name);
                if candidate.exists() && candidate.is_file() {
                    ranked.push((score, candidate));
                }
            }
        }
    }

    ranked.sort_by(|(sa, pa), (sb, pb)| sb.cmp(sa).then_with(|| pa.cmp(pb)));

    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for (_, p) in ranked {
        let key = p.to_string_lossy().to_ascii_lowercase();
        if seen.insert(key) {
            out.push(p);
        }
    }
    out
}

fn resolve_binary_candidates(cfg: &LlamaRuntimeConfig) -> Result<Vec<PathBuf>, String> {
    if let Some(p) = cfg.server_path.as_ref().and_then(|p| resolve_from_path(p)) {
        return Ok(vec![p]);
    }

    if let Ok(p) = env::var("OXIDE_LLAMA_SERVER_PATH")
        && let Some(pb) = resolve_from_path(&p)
    {
        return Ok(vec![pb]);
    }

    let bundled = find_binaries_in_bundle(cfg);
    if !bundled.is_empty() {
        return Ok(bundled);
    }

    let path_var = env::var_os("PATH").ok_or_else(|| "PATH is not set".to_string())?;
    let candidates: &[&str] = if cfg!(windows) {
        &["llama-server.exe", "llama-server"]
    } else {
        &["llama-server"]
    };

    let mut found = Vec::new();
    let mut seen = HashSet::new();
    for dir in env::split_paths(&path_var) {
        for bin in candidates {
            let candidate = dir.join(bin);
            if candidate.exists() && candidate.is_file() {
                let key = candidate.to_string_lossy().to_ascii_lowercase();
                if seen.insert(key) {
                    found.push(candidate);
                }
            }
        }
    }
    if !found.is_empty() {
        return Ok(found);
    }

    Err("llama-server binary not found (config/env/example/bin/PATH)".to_string())
}

fn spawn_log_reader<R: Read + Send + 'static>(
    reader: R,
    sink: Arc<Mutex<String>>,
    prefix: &'static str,
) {
    thread::spawn(move || {
        let mut buf = BufReader::new(reader);
        let mut line = String::new();
        while let Ok(n) = buf.read_line(&mut line) {
            if n == 0 {
                break;
            }
            let l = line.trim_end();
            if !l.is_empty() {
                log::info!("[{}] {}", prefix, l);
                if let Ok(mut s) = sink.lock() {
                    s.push_str(l);
                    s.push('\n');
                    trim_log_buffer(&mut s, 8_000);
                }
            }
            line.clear();
        }
    });
}

fn trim_log_buffer(s: &mut String, max_bytes: usize) {
    if s.len() <= max_bytes {
        return;
    }
    let mut keep_from = s.len().saturating_sub(max_bytes);
    while keep_from < s.len() && !s.is_char_boundary(keep_from) {
        keep_from += 1;
    }
    s.drain(..keep_from);
}

fn health_check(port: u16, api_key: &str) -> bool {
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    let url = format!("http://127.0.0.1:{}/health", port);
    let req = client.get(url).bearer_auth(api_key);
    match req.send() {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

fn prepend_path_entries(binary: &Path) -> Vec<PathBuf> {
    let mut entries = Vec::new();

    if let Some(parent) = binary.parent() {
        entries.push(parent.to_path_buf());
    }

    // If running CUDA build from example/bin, include cudart sibling folders.
    for root in bin_roots() {
        let read = match std::fs::read_dir(root) {
            Ok(v) => v,
            Err(_) => continue,
        };
        for entry in read.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if name.to_ascii_lowercase().starts_with("cudart-") {
                entries.push(path);
            }
        }
    }

    let mut uniq = Vec::new();
    for p in entries {
        if !uniq.contains(&p) {
            uniq.push(p);
        }
    }
    uniq
}

fn merge_path(prepend: &[PathBuf], base: Option<String>) -> Option<String> {
    let mut merged = Vec::new();
    let mut seen = HashSet::new();

    for p in prepend {
        let key = p.to_string_lossy().to_ascii_lowercase();
        if seen.insert(key) {
            merged.push(p.clone());
        }
    }

    if let Some(base_path) = base {
        for p in env::split_paths(&base_path) {
            let key = p.to_string_lossy().to_ascii_lowercase();
            if seen.insert(key) {
                merged.push(p);
            }
        }
    }

    match env::join_paths(merged) {
        Ok(os) => Some(os.to_string_lossy().to_string()),
        Err(_) => None,
    }
}

fn set_env_var(out: &mut Vec<(String, String)>, key: &str, value: String) {
    if let Some((_, v)) = out.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(key)) {
        *v = value;
        return;
    }
    out.push((key.to_string(), value));
}

fn build_env(cfg: &LlamaRuntimeConfig, api_key: &str, binary: &Path) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = cfg
        .extra_env
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let prepend = prepend_path_entries(binary);
    let base_path = cfg
        .extra_env
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("PATH"))
        .map(|(_, v)| v.clone())
        .or_else(|| env::var("PATH").ok());
    if let Some(path_value) = merge_path(&prepend, base_path) {
        set_env_var(&mut out, "PATH", path_value);
    }

    out.push(("LLAMA_API_KEY".to_string(), api_key.to_string()));
    out
}

fn request_graceful_shutdown(info: &SessionInfo) {
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(700))
        .build()
    {
        Ok(c) => c,
        Err(_) => return,
    };

    // Best-effort graceful shutdown hooks. Different llama-server builds expose different paths.
    for path in ["/shutdown", "/v1/shutdown"] {
        let url = format!("http://127.0.0.1:{}{}", info.port, path);
        let _ = client.post(&url).bearer_auth(&info.api_key).send();
        let _ = client.get(&url).bearer_auth(&info.api_key).send();
    }
}

fn wait_for_exit(child: &mut Child, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Ok(None) => thread::sleep(PROCESS_WAIT_POLL),
            Err(_) => return false,
        }
    }
    matches!(child.try_wait(), Ok(Some(_)))
}

#[cfg(windows)]
fn windows_best_effort_stop(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T"])
        .status();
}

fn stop_session_process(session: &mut LlamaSession) {
    if wait_for_exit(&mut session.child, Duration::from_millis(10)) {
        return;
    }

    request_graceful_shutdown(&session.info);
    if wait_for_exit(&mut session.child, GRACEFUL_SHUTDOWN_TIMEOUT) {
        return;
    }

    #[cfg(windows)]
    {
        windows_best_effort_stop(session.child.id());
        if wait_for_exit(&mut session.child, Duration::from_secs(1)) {
            return;
        }
    }

    let _ = session.child.kill();
    let _ = session.child.wait();
}

fn start_once(
    binary: &Path,
    model_id: &str,
    model_path: &str,
    kind: LlamaSessionKind,
    cfg: &LlamaRuntimeConfig,
    timeout: Duration,
) -> Result<LlamaSession, String> {
    let port = random_port()?;
    let api_key = generate_api_key(model_id);
    let args = LlamaArgumentBuilder::new(cfg.clone(), kind).build(model_id, model_path, port);
    let mut cmd = Command::new(binary);
    cmd.args(args);
    cmd.envs(build_env(cfg, &api_key, binary));
    if let Some(parent) = binary.parent() {
        cmd.current_dir(parent);
    }
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    #[cfg(windows)]
    {
        // Keep llama-server as background child process without a visible console window.
        cmd.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn llama-server: {}", e))?;

    let logs = Arc::new(Mutex::new(String::new()));
    if let Some(stdout) = child.stdout.take() {
        spawn_log_reader(stdout, logs.clone(), "llama stdout");
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_log_reader(stderr, logs.clone(), "llama stderr");
    }

    let started_at = now_unix();
    let start = Instant::now();

    while start.elapsed() < timeout {
        match child.try_wait() {
            Ok(Some(status)) => {
                let captured = logs.lock().map(|s| s.clone()).unwrap_or_default();
                return Err(format!(
                    "llama-server exited early ({:?}). logs: {}",
                    status.code(),
                    captured
                ));
            }
            Ok(None) => {}
            Err(e) => return Err(format!("Failed checking llama-server process: {}", e)),
        }

        if health_check(port, &api_key) {
            let pid = child.id() as i32;
            let info = SessionInfo {
                pid,
                port,
                model_id: model_id.to_string(),
                model_path: model_path.to_string(),
                api_key,
                kind,
                created_at: started_at,
                last_health_ok_at: Some(now_unix()),
            };
            return Ok(LlamaSession { child, info });
        }

        thread::sleep(Duration::from_millis(HEALTH_POLL_INTERVAL_MS));
    }

    let _ = child.kill();
    let _ = child.wait();
    Err("llama-server readiness timeout".to_string())
}

pub fn get_or_start_session(
    state: &LlamaCppState,
    model_id: &str,
    model_path: &str,
    kind: LlamaSessionKind,
    cfg: &LlamaRuntimeConfig,
    timeout: Duration,
) -> Result<SessionInfo, String> {
    cleanup_dead_sessions(state)?;

    if let Some(existing) = find_session_info(state, model_id, kind)? {
        return Ok(existing);
    }

    let binaries = resolve_binary_candidates(cfg)?;
    let retries_per_binary = if binaries.len() > 1 { 2 } else { START_RETRIES };
    let mut errors = Vec::new();

    for binary in binaries {
        log::info!("llama.cpp candidate binary: {}", binary.display());
        let mut last_err = String::new();
        for _ in 0..retries_per_binary {
            match start_once(&binary, model_id, model_path, kind, cfg, timeout) {
                Ok(session) => {
                    let info = session.info.clone();
                    let mut map = state.sessions.lock().map_err(|e| e.to_string())?;
                    map.insert(key(model_id, kind), session);
                    return Ok(info);
                }
                Err(e) => {
                    last_err = e;
                }
            }
        }
        errors.push(format!("{} => {}", binary.display(), last_err));
    }

    Err(format!(
        "Failed to start llama-server session after trying binaries: {}",
        errors.join(" | ")
    ))
}

pub fn find_session_info(
    state: &LlamaCppState,
    model_id: &str,
    kind: LlamaSessionKind,
) -> Result<Option<SessionInfo>, String> {
    let mut map = state.sessions.lock().map_err(|e| e.to_string())?;
    if let Some(sess) = map.get_mut(&key(model_id, kind)) {
        match sess.child.try_wait() {
            Ok(Some(_)) => {
                map.remove(&key(model_id, kind));
                Ok(None)
            }
            Ok(None) => Ok(Some(sess.info.clone())),
            Err(e) => Err(e.to_string()),
        }
    } else {
        Ok(None)
    }
}

pub fn mark_session_health_ok(
    state: &LlamaCppState,
    model_id: &str,
    kind: LlamaSessionKind,
) -> Result<(), String> {
    let mut map = state.sessions.lock().map_err(|e| e.to_string())?;
    if let Some(sess) = map.get_mut(&key(model_id, kind)) {
        sess.info.last_health_ok_at = Some(now_unix());
    }
    Ok(())
}

pub fn unload_session(
    state: &LlamaCppState,
    model_id: &str,
    kind: LlamaSessionKind,
) -> Result<(), String> {
    let mut map = state.sessions.lock().map_err(|e| e.to_string())?;
    if let Some(mut sess) = map.remove(&key(model_id, kind)) {
        stop_session_process(&mut sess);
    }
    Ok(())
}

pub fn unload_model_sessions(state: &LlamaCppState, model_id: &str) -> Result<(), String> {
    unload_session(state, model_id, LlamaSessionKind::Chat)?;
    unload_session(state, model_id, LlamaSessionKind::Embedding)?;
    Ok(())
}

pub fn unload_all_sessions(state: &LlamaCppState) -> Result<(), String> {
    let mut map = state.sessions.lock().map_err(|e| e.to_string())?;
    for (_, mut sess) in map.drain() {
        stop_session_process(&mut sess);
    }
    Ok(())
}

pub fn cleanup_dead_sessions(state: &LlamaCppState) -> Result<(), String> {
    let mut map = state.sessions.lock().map_err(|e| e.to_string())?;
    let mut dead = Vec::new();
    for (k, sess) in map.iter_mut() {
        if let Ok(Some(_)) = sess.child.try_wait() {
            dead.push(k.clone());
        }
    }
    for k in dead {
        map.remove(&k);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::trim_log_buffer;

    #[test]
    fn trim_log_buffer_preserves_utf8_boundaries() {
        let mut s = format!("🙂{}", "a".repeat(7999));
        trim_log_buffer(&mut s, 8_000);
        assert!(s.is_char_boundary(0));
        assert_eq!(s.len(), 7_999);
    }

    #[test]
    fn trim_log_buffer_keeps_short_strings() {
        let mut s = "hello".to_string();
        trim_log_buffer(&mut s, 8_000);
        assert_eq!(s, "hello");
    }
}
