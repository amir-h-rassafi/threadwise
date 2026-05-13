use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

pub struct TranscriptFile {
    path: PathBuf,
    pub bytes: u64,
    pub modified: u64,
}

impl TranscriptFile {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn display_path(&self, root: &Path) -> String {
        self.path
            .strip_prefix(root)
            .unwrap_or(&self.path)
            .display()
            .to_string()
    }
}

#[derive(Default)]
pub struct TranscriptSummary {
    pub session_id: Option<String>,
    pub cwd: Option<String>,
    pub cli_version: Option<String>,
    pub first_timestamp: Option<String>,
    pub last_timestamp: Option<String>,
    pub events: usize,
    pub user_messages: usize,
    pub agent_messages: usize,
    pub task_started: usize,
    pub task_complete: usize,
    pub parse_errors: usize,
    pub summary_text: String,
}

const SUMMARY_TEXT_CAP: usize = 16384;

pub fn discover_transcripts(root: &Path) -> Result<Vec<TranscriptFile>, String> {
    let mut transcripts = Vec::new();
    if !root.is_dir() {
        return Ok(transcripts);
    }

    visit_transcript_dir(root, &mut transcripts)?;
    transcripts.sort_by_key(|file| std::cmp::Reverse(file.modified));
    Ok(transcripts)
}

fn visit_transcript_dir(dir: &Path, transcripts: &mut Vec<TranscriptFile>) -> Result<(), String> {
    for entry in
        fs::read_dir(dir).map_err(|err| format!("failed to read {}: {err}", dir.display()))?
    {
        let entry =
            entry.map_err(|err| format!("failed to read entry in {}: {err}", dir.display()))?;
        let path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|err| format!("failed to read metadata for {}: {err}", path.display()))?;

        if metadata.is_dir() {
            visit_transcript_dir(&path, transcripts)?;
        } else if is_transcript_file(&path) {
            transcripts.push(TranscriptFile {
                path,
                bytes: metadata.len(),
                modified: metadata
                    .modified()
                    .ok()
                    .and_then(system_time_secs)
                    .unwrap_or(0),
            });
        }
    }
    Ok(())
}

fn is_transcript_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension, "jsonl" | "json"))
}

fn system_time_secs(time: SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

pub fn parse_transcript(path: &Path, agent_kind: &str) -> Result<TranscriptSummary, String> {
    let file =
        fs::File::open(path).map_err(|err| format!("failed to open {}: {err}", path.display()))?;
    let reader = BufReader::new(file);
    let mut summary = TranscriptSummary::default();

    for line in reader.lines() {
        let line = line.map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        summary.events += 1;
        let value = match serde_json::from_str::<Value>(line) {
            Ok(value) => value,
            Err(_) => {
                summary.parse_errors += 1;
                continue;
            }
        };

        if let Some(timestamp) = value.get("timestamp").and_then(Value::as_str) {
            if summary.first_timestamp.is_none() {
                summary.first_timestamp = Some(timestamp.to_string());
            }
            summary.last_timestamp = Some(timestamp.to_string());
        }

        match agent_kind {
            "codex" => apply_codex_event(&value, &mut summary),
            "claude-code" => apply_claude_code_event(&value, &mut summary),
            _ => {}
        }
    }

    if summary.session_id.is_none() {
        summary.session_id = session_id_from_path(path);
    }

    Ok(summary)
}

fn apply_codex_event(value: &Value, summary: &mut TranscriptSummary) {
    match value.get("type").and_then(Value::as_str) {
        Some("session_meta") => apply_session_meta(value, summary),
        Some("event_msg") => apply_event_msg(value, summary),
        _ => {}
    }
}

fn apply_claude_code_event(value: &Value, summary: &mut TranscriptSummary) {
    if summary.session_id.is_none()
        && let Some(id) = value.get("sessionId").and_then(Value::as_str)
    {
        summary.session_id = Some(id.to_string());
    }
    if summary.cwd.is_none()
        && let Some(cwd) = value.get("cwd").and_then(Value::as_str)
    {
        summary.cwd = Some(cwd.to_string());
    }
    if summary.cli_version.is_none()
        && let Some(version) = value.get("version").and_then(Value::as_str)
    {
        summary.cli_version = Some(version.to_string());
    }

    let Some(event_type) = value.get("type").and_then(Value::as_str) else {
        return;
    };
    let message = value.get("message");

    match event_type {
        "user" => {
            summary.user_messages += 1;
            if let Some(content) = message.and_then(|m| m.get("content")) {
                append_claude_content(content, &mut summary.summary_text);
            }
        }
        "assistant" => {
            summary.agent_messages += 1;
            if let Some(content) = message.and_then(|m| m.get("content")) {
                append_claude_content(content, &mut summary.summary_text);
            }
        }
        _ => {}
    }
}

fn append_claude_content(content: &Value, summary_text: &mut String) {
    if let Some(text) = content.as_str() {
        append_text(text, summary_text);
        return;
    }
    if let Some(items) = content.as_array() {
        for item in items {
            if item.get("type").and_then(Value::as_str) == Some("text")
                && let Some(text) = item.get("text").and_then(Value::as_str)
            {
                append_text(text, summary_text);
            }
        }
    }
}

fn apply_session_meta(value: &Value, summary: &mut TranscriptSummary) {
    let Some(payload) = value.get("payload") else {
        return;
    };

    if let Some(id) = payload.get("id").and_then(Value::as_str) {
        summary.session_id = Some(id.to_string());
    }
    if let Some(cwd) = payload.get("cwd").and_then(Value::as_str) {
        summary.cwd = Some(cwd.to_string());
    }
    if let Some(cli_version) = payload.get("cli_version").and_then(Value::as_str) {
        summary.cli_version = Some(cli_version.to_string());
    }
}

fn apply_event_msg(value: &Value, summary: &mut TranscriptSummary) {
    let Some(payload) = value.get("payload") else {
        return;
    };
    let Some(payload_type) = payload.get("type").and_then(Value::as_str) else {
        return;
    };

    match payload_type {
        "user_message" => {
            summary.user_messages += 1;
            append_message_text(payload, &mut summary.summary_text);
        }
        "agent_message" => {
            summary.agent_messages += 1;
            append_message_text(payload, &mut summary.summary_text);
        }
        "task_started" => summary.task_started += 1,
        "task_complete" => summary.task_complete += 1,
        _ => {}
    }
}

fn append_message_text(payload: &Value, summary_text: &mut String) {
    let text = payload
        .get("message")
        .or_else(|| payload.get("text"))
        .or_else(|| payload.get("content"))
        .and_then(Value::as_str);
    if let Some(text) = text {
        append_text(text, summary_text);
    }
}

fn append_text(text: &str, summary_text: &mut String) {
    if summary_text.len() >= SUMMARY_TEXT_CAP {
        return;
    }
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    if !summary_text.is_empty() {
        summary_text.push(' ');
    }
    let mut budget = SUMMARY_TEXT_CAP.saturating_sub(summary_text.len());
    for ch in text.chars() {
        let width = ch.len_utf8();
        if width > budget {
            break;
        }
        summary_text.push(ch);
        budget -= width;
    }
}

fn session_id_from_path(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    stem.strip_prefix("rollout-")
        .and_then(|value| value.rsplit_once('-').map(|(_, id)| id.to_string()))
        .or_else(|| Some(stem.to_string()))
}
