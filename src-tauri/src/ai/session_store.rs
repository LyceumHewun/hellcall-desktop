use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSessionEvent {
    pub id: String,
    pub kind: String,
    pub text: Option<String>,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSessionSummary {
    pub id: String,
    pub title: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub message_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSessionRecord {
    #[serde(flatten)]
    pub summary: AiSessionSummary,
    pub events: Vec<AiSessionEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AiSessionMeta {
    pub id: String,
    pub title: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

pub fn list_sessions(app_handle: &AppHandle) -> Result<Vec<AiSessionSummary>, String> {
    let sessions_dir = resolve_sessions_dir(app_handle)?;
    let entries = fs::read_dir(&sessions_dir).map_err(|e| e.to_string())?;
    let mut sessions = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.path().is_dir() {
            continue;
        }

        let meta_path = entry.path().join("meta.json");
        if !meta_path.is_file() {
            continue;
        }

        let meta = read_meta(&meta_path)?;
        let message_count = count_events(&entry.path().join("events.jsonl"))?;
        sessions.push(AiSessionSummary {
            id: meta.id,
            title: meta.title,
            created_at_ms: meta.created_at_ms,
            updated_at_ms: meta.updated_at_ms,
            message_count,
        });
    }

    sessions.sort_by(|a, b| b.updated_at_ms.cmp(&a.updated_at_ms));
    Ok(sessions)
}

pub fn create_session(app_handle: &AppHandle, title: Option<String>) -> Result<AiSessionSummary, String> {
    let sessions_dir = resolve_sessions_dir(app_handle)?;
    let id = format!("session-{}", now_ms());
    let session_dir = sessions_dir.join(&id);
    fs::create_dir_all(&session_dir).map_err(|e| e.to_string())?;

    let now = now_ms();
    let meta = AiSessionMeta {
        id: id.clone(),
        title: title
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "New Session".to_string()),
        created_at_ms: now,
        updated_at_ms: now,
    };

    write_meta(&session_dir.join("meta.json"), &meta)?;
    fs::File::create(session_dir.join("events.jsonl")).map_err(|e| e.to_string())?;

    Ok(AiSessionSummary {
        id: meta.id,
        title: meta.title,
        created_at_ms: meta.created_at_ms,
        updated_at_ms: meta.updated_at_ms,
        message_count: 0,
    })
}

pub fn get_session(app_handle: &AppHandle, session_id: &str) -> Result<AiSessionRecord, String> {
    let session_dir = resolve_session_dir(app_handle, session_id)?;
    let meta = read_meta(&session_dir.join("meta.json"))?;
    let events = read_events(&session_dir.join("events.jsonl"))?;
    Ok(AiSessionRecord {
        summary: AiSessionSummary {
            id: meta.id,
            title: meta.title,
            created_at_ms: meta.created_at_ms,
            updated_at_ms: meta.updated_at_ms,
            message_count: events.len(),
        },
        events,
    })
}

pub fn delete_session(app_handle: &AppHandle, session_id: &str) -> Result<bool, String> {
    let session_dir = resolve_session_dir(app_handle, session_id)?;
    fs::remove_dir_all(session_dir).map_err(|e| e.to_string())?;
    Ok(true)
}

#[allow(dead_code)]
pub fn append_event(app_handle: &AppHandle, session_id: &str, event: AiSessionEvent) -> Result<(), String> {
    let session_dir = resolve_session_dir(app_handle, session_id)?;
    let events_path = session_dir.join("events.jsonl");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&events_path)
        .map_err(|e| e.to_string())?;
    let serialized = serde_json::to_string(&event).map_err(|e| e.to_string())?;
    writeln!(file, "{}", serialized).map_err(|e| e.to_string())?;

    let mut meta = read_meta(&session_dir.join("meta.json"))?;
    meta.updated_at_ms = now_ms();
    write_meta(&session_dir.join("meta.json"), &meta)?;
    Ok(())
}

pub fn maybe_promote_title_from_text(
    app_handle: &AppHandle,
    session_id: &str,
    text: &str,
) -> Result<(), String> {
    let session_dir = resolve_session_dir(app_handle, session_id)?;
    let mut meta = read_meta(&session_dir.join("meta.json"))?;
    if meta.title != "New Session" {
        return Ok(());
    }

    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    let mut chars = trimmed.chars();
    let promoted: String = chars.by_ref().take(28).collect();
    meta.title = if chars.next().is_some() {
        format!("{}...", promoted)
    } else {
        promoted
    };
    meta.updated_at_ms = now_ms();
    write_meta(&session_dir.join("meta.json"), &meta)
}

fn resolve_sessions_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let base_dir = app_handle.path().app_data_dir().map_err(|e| e.to_string())?;
    let sessions_dir = base_dir.join("chat_sessions");
    fs::create_dir_all(&sessions_dir).map_err(|e| e.to_string())?;
    Ok(sessions_dir)
}

fn resolve_session_dir(app_handle: &AppHandle, session_id: &str) -> Result<PathBuf, String> {
    let clean_id = session_id.trim();
    if clean_id.is_empty() || clean_id.contains("..") || clean_id.contains(['/', '\\']) {
        return Err("Invalid AI session ID.".to_string());
    }

    let session_dir = resolve_sessions_dir(app_handle)?.join(clean_id);
    if !session_dir.is_dir() {
        return Err(format!("AI session '{}' was not found.", clean_id));
    }

    Ok(session_dir)
}

fn read_meta(path: &Path) -> Result<AiSessionMeta, String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

fn write_meta(path: &Path, meta: &AiSessionMeta) -> Result<(), String> {
    let content = serde_json::to_string_pretty(meta).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())
}

fn read_events(path: &Path) -> Result<Vec<AiSessionEvent>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();

    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let event = serde_json::from_str::<AiSessionEvent>(&line).map_err(|e| e.to_string())?;
        events.push(event);
    }

    Ok(events)
}

fn count_events(path: &Path) -> Result<usize, String> {
    if !path.exists() {
        return Ok(0);
    }

    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let reader = BufReader::new(file);
    let mut count = 0;

    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if !line.trim().is_empty() {
            count += 1;
        }
    }

    Ok(count)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
