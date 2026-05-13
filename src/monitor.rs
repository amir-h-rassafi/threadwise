use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, IsTerminal, Write};
use std::thread;
use std::time::Duration;

use serde_json::Value;

use crate::advice::{Recommendation, RecommendationAction};
use crate::hooks::{HookEvent, HookKind};
use crate::paths::AppPaths;
use crate::registry::{read_enablements, read_sources, unix_timestamp};
use crate::session_index::build_session_index;

const RECENT_EVENT_LIMIT: usize = 12;
const PROMPT_PREVIEW_CHARS: usize = 160;
const REASON_PREVIEW_CHARS: usize = 180;

pub fn record_hook_result(
    paths: &AppPaths,
    event: &HookEvent,
    enabled: bool,
    recommendation: Option<&Recommendation>,
) -> Result<(), String> {
    fs::create_dir_all(&paths.monitor_dir)
        .map_err(|err| format!("failed to create {}: {err}", paths.monitor_dir.display()))?;

    let action = recommendation
        .map(recommendation_action)
        .unwrap_or_else(|| {
            if event.kind == HookKind::Stop {
                "stop"
            } else if enabled {
                "silent"
            } else {
                "silent_disabled"
            }
        });
    let confidence = recommendation.map(|rec| rec.confidence);
    let selected_session_id = recommendation.and_then(|rec| rec.session_id.as_deref());
    let reason = recommendation
        .map(|rec| rec.reason.as_str())
        .unwrap_or_else(|| silent_reason(event.kind, enabled));

    let record = serde_json::json!({
        "timestamp": unix_timestamp(),
        "agent": event.agent,
        "kind": hook_kind(event.kind),
        "cwd": event.cwd.display().to_string(),
        "session_id": event.session_id,
        "prompt": preview(event.prompt.as_deref().unwrap_or(""), PROMPT_PREVIEW_CHARS),
        "enabled": enabled,
        "decision": action,
        "confidence": confidence,
        "selected_session_id": selected_session_id,
        "reason": preview(reason, REASON_PREVIEW_CHARS),
    });

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.hook_events_file)
        .map_err(|err| {
            format!(
                "failed to write {}: {err}",
                paths.hook_events_file.display()
            )
        })?;
    writeln!(file, "{record}").map_err(|err| {
        format!(
            "failed to write {}: {err}",
            paths.hook_events_file.display()
        )
    })?;
    Ok(())
}

pub fn monitor_once() -> Result<i32, String> {
    let paths = AppPaths::resolve()?;
    print_monitor_snapshot(&paths)?;
    Ok(0)
}

pub fn monitor_watch() -> Result<i32, String> {
    let paths = AppPaths::resolve()?;
    loop {
        print!("\x1b[2J\x1b[H");
        print_monitor_snapshot(&paths)?;
        std::io::stdout()
            .flush()
            .map_err(|err| format!("failed to flush stdout: {err}"))?;
        thread::sleep(Duration::from_secs(2));
    }
}

fn print_monitor_snapshot(paths: &AppPaths) -> Result<(), String> {
    let colors = Colors::detect();
    let events = read_recent_events(paths, RECENT_EVENT_LIMIT)?;
    let latest = events.last();
    let sources = read_sources(paths)?;
    let enablements = read_enablements(paths)?;
    let workspace = std::env::current_dir().map_err(|err| format!("failed to read cwd: {err}"))?;
    let index = build_session_index(&sources, &enablements, &workspace)?;
    let related = index.related_transcripts();

    println!("{}", colors.header("Threadwise monitor"));
    println!("version: {}", colors.value(env!("CARGO_PKG_VERSION")));
    println!("binary: {}", colors.path(&current_exe()));
    println!(
        "monitor_dir: {}",
        colors.path(&paths.monitor_dir.display().to_string())
    );
    println!(
        "hook_log: {}",
        colors.path(&paths.hook_events_file.display().to_string())
    );
    println!("refresh: {}", colors.command("tw top"));
    println!(
        "manual_test: {}",
        colors.command("tw hook codex-user-prompt-submit --prompt 'weather'")
    );
    println!();

    println!("{}", colors.section("Runtime"));
    println!(
        "current_workspace: {}",
        colors.path(&index.workspace.display().to_string())
    );
    println!(
        "sources: {} enabled={} transcripts={} parsed={} related={}",
        colors.count(index.sources.len()),
        colors.count(index.enabled_sources),
        colors.count(index.total_transcripts),
        colors.count(index.parsed_transcripts),
        colors.count(related.len())
    );
    print_codex_hook_config(paths, &colors)?;
    if let Some((source, transcript)) = related.first() {
        println!("top_related:");
        println!("  title: {}", colors.value(&transcript_title(transcript)));
        println!(
            "  agent={} id={} relation={} score={}",
            colors.value(&source.agent),
            colors.session(transcript.session_id.as_deref().unwrap_or("unknown")),
            colors.value(transcript.relation.as_str()),
            colors.count(transcript.score)
        );
        println!(
            "  cwd: {}",
            colors.path(transcript.cwd.as_deref().unwrap_or("unknown"))
        );
        println!("  path: {}", colors.path(&transcript.display_path));
    } else {
        println!("top_related: {}", colors.warn("none"));
    }
    println!();

    println!("{}", colors.section("Last Hook"));
    if let Some(event) = latest {
        let decision = string_field(event, "decision");
        println!(
            "  ts={} agent={} kind={} enabled={} session={} cwd={}",
            colors.value(&string_field(event, "timestamp")),
            colors.value(&string_field(event, "agent")),
            colors.value(&string_field(event, "kind")),
            colors.enabled(&string_field(event, "enabled")),
            colors.session(&optional_string_field(event, "session_id")),
            colors.path(&string_field(event, "cwd"))
        );
        println!(
            "  prompt: {}",
            colors.prompt(&string_field(event, "prompt"))
        );
        println!(
            "  response: decision={} confidence={} selected_session={} reason={}",
            colors.decision(&decision),
            colors.count_text(&optional_string_field(event, "confidence")),
            colors.session(&optional_string_field(event, "selected_session_id")),
            colors.value(&string_field(event, "reason"))
        );
    } else {
        println!("  {}", colors.warn("none yet"));
        println!(
            "  {}",
            colors.value("hook events appear here after Codex invokes `tw hook ...`")
        );
    }
    println!();

    println!("{}", colors.section("Recent Hooks"));
    if events.is_empty() {
        println!("  {}", colors.warn("(empty)"));
    } else {
        for event in events.iter().rev() {
            let decision = string_field(event, "decision");
            println!(
                "  {} {} {} session={} decision={} target={}",
                colors.value(&string_field(event, "timestamp")),
                colors.value(&string_field(event, "agent")),
                colors.value(&string_field(event, "kind")),
                colors.session(&optional_string_field(event, "session_id")),
                colors.decision(&decision),
                colors.session(&optional_string_field(event, "selected_session_id")),
            );
        }
    }

    Ok(())
}

fn print_codex_hook_config(paths: &AppPaths, colors: &Colors) -> Result<(), String> {
    if !paths.default_codex_config.is_file() && !paths.default_codex_hooks.is_file() {
        println!(
            "codex_hook_config: {} config={} hooks_file={}",
            colors.warn("missing"),
            colors.path(&paths.default_codex_config.display().to_string()),
            colors.path(&paths.default_codex_hooks.display().to_string())
        );
        return Ok(());
    }

    let content = if paths.default_codex_config.is_file() {
        fs::read_to_string(&paths.default_codex_config).map_err(|err| {
            format!(
                "failed to read {}: {err}",
                paths.default_codex_config.display()
            )
        })?
    } else {
        String::new()
    };
    let hooks_content = if paths.default_codex_hooks.is_file() {
        fs::read_to_string(&paths.default_codex_hooks).map_err(|err| {
            format!(
                "failed to read {}: {err}",
                paths.default_codex_hooks.display()
            )
        })?
    } else {
        String::new()
    };
    let has_feature = content.contains("hooks = true");
    let has_deprecated_feature = content.contains("codex_hooks = true");
    let has_prompt_command = content.contains("tw hook codex-user-prompt-submit")
        || hooks_content.contains("tw hook codex-user-prompt-submit");
    let has_stop_command =
        content.contains("tw hook codex-stop") || hooks_content.contains("tw hook codex-stop");
    let has_prompt_table = content.contains("[[hooks.UserPromptSubmit]]");
    let has_stop_table = content.contains("[[hooks.Stop]]");
    let has_prompt_handler = content.contains("[[hooks.UserPromptSubmit.hooks]]");
    let has_stop_handler = content.contains("[[hooks.Stop.hooks]]");
    let has_command_type =
        content.contains("type = \"command\"") || hooks_content.contains("\"type\": \"command\"");
    let has_matcher = content.contains("matcher =");
    let has_hooks_file_prompt = hooks_content.contains("\"UserPromptSubmit\"");
    let has_hooks_file_stop = hooks_content.contains("\"Stop\"");
    let has_hooks_file_matcher = hooks_content.contains("\"matcher\"");
    let usable_feature = has_feature || has_deprecated_feature;
    let inline_prompt_ok = has_prompt_table && has_prompt_handler && has_matcher;
    let inline_stop_ok = has_stop_table && has_stop_handler && has_matcher;
    let hooks_file_prompt_ok = has_hooks_file_prompt && has_hooks_file_matcher;
    let hooks_file_stop_ok = has_hooks_file_stop && has_hooks_file_matcher;
    let prompt_ok = usable_feature
        && (inline_prompt_ok || hooks_file_prompt_ok)
        && has_prompt_command
        && has_command_type;
    let stop_ok = usable_feature
        && (inline_stop_ok || hooks_file_stop_ok)
        && has_stop_command
        && has_command_type;
    let status = if prompt_ok {
        if has_deprecated_feature && !has_feature {
            colors.warn("deprecated flag")
        } else {
            colors.ok("ok")
        }
    } else if has_prompt_command {
        colors.warn("old/incomplete")
    } else {
        colors.warn("missing prompt hook")
    };
    println!(
        "codex_hook_config: {} hooks={} deprecated_codex_hooks={} prompt_hook={} stop_hook={} config={} hooks_file={}",
        status,
        colors.enabled_bool(has_feature),
        colors.enabled_bool(has_deprecated_feature),
        colors.enabled_bool(prompt_ok),
        colors.enabled_bool(stop_ok),
        colors.path(&paths.default_codex_config.display().to_string()),
        colors.path(&paths.default_codex_hooks.display().to_string())
    );
    if prompt_ok {
        println!(
            "codex_reload_note: {}",
            colors.value("open a new Codex session after changing hook config or reinstalling tw")
        );
    } else if has_prompt_command {
        println!(
            "codex_next: {}",
            colors.command(
                "update ~/.codex/config.toml with nested [[hooks.Event.hooks]] command handlers and [features].hooks"
            )
        );
    } else {
        println!("codex_next: {}", colors.command("tw init codex"));
    }
    Ok(())
}

fn read_recent_events(paths: &AppPaths, limit: usize) -> Result<Vec<Value>, String> {
    if !paths.hook_events_file.is_file() {
        return Ok(Vec::new());
    }

    let file = fs::File::open(&paths.hook_events_file)
        .map_err(|err| format!("failed to read {}: {err}", paths.hook_events_file.display()))?;
    let mut events = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line
            .map_err(|err| format!("failed to read {}: {err}", paths.hook_events_file.display()))?;
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(&line) {
            events.push(value);
            if events.len() > limit {
                events.remove(0);
            }
        }
    }
    Ok(events)
}

fn recommendation_action(recommendation: &Recommendation) -> &'static str {
    match recommendation.action {
        RecommendationAction::ResumeExisting => "resume_existing",
        RecommendationAction::OpenNewAgent => "open_new_agent",
    }
}

fn hook_kind(kind: HookKind) -> &'static str {
    match kind {
        HookKind::UserPromptSubmit => "user-prompt-submit",
        HookKind::Stop => "stop",
    }
}

fn silent_reason(kind: HookKind, enabled: bool) -> &'static str {
    if kind == HookKind::Stop {
        "stop hook observed"
    } else if enabled {
        "no recommendation emitted"
    } else {
        "no enabled source matched this hook"
    }
}

fn preview(text: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for ch in text.chars().take(max_chars) {
        if ch.is_control() {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    if text.chars().count() > max_chars {
        out.push_str("...");
    }
    out
}

fn transcript_title(transcript: &crate::session_index::IndexedTranscript) -> String {
    transcript
        .title
        .as_deref()
        .filter(|title| !title.trim().is_empty())
        .or(transcript.session_id.as_deref())
        .unwrap_or(&transcript.display_path)
        .to_string()
}

fn string_field(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        Some(other) => other.to_string(),
        None => "unknown".to_string(),
    }
}

fn optional_string_field(value: &Value, key: &str) -> String {
    match value.get(key) {
        Some(Value::Null) | None => "none".to_string(),
        _ => string_field(value, key),
    }
}

fn current_exe() -> String {
    std::env::current_exe()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

struct Colors {
    enabled: bool,
}

impl Colors {
    fn detect() -> Self {
        Self {
            enabled: std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none(),
        }
    }

    fn header(&self, text: &str) -> String {
        self.paint("1;36", text)
    }

    fn section(&self, text: &str) -> String {
        self.paint("1;34", text)
    }

    fn ok(&self, text: &str) -> String {
        self.paint("1;32", text)
    }

    fn warn(&self, text: &str) -> String {
        self.paint("1;33", text)
    }

    fn value(&self, text: &str) -> String {
        self.paint("37", text)
    }

    fn path(&self, text: &str) -> String {
        self.paint("36", text)
    }

    fn command(&self, text: &str) -> String {
        self.paint("1;35", text)
    }

    fn prompt(&self, text: &str) -> String {
        self.paint("1;37", text)
    }

    fn session(&self, text: &str) -> String {
        if text == "none" || text == "unknown" {
            self.warn(text)
        } else {
            self.paint("35", text)
        }
    }

    fn decision(&self, text: &str) -> String {
        match text {
            "resume_existing" | "open_new_agent" => self.ok(text),
            "silent" | "stop" => self.paint("2;37", text),
            "silent_disabled" => self.warn(text),
            _ => self.value(text),
        }
    }

    fn enabled(&self, text: &str) -> String {
        match text {
            "true" => self.ok(text),
            "false" => self.warn(text),
            _ => self.value(text),
        }
    }

    fn enabled_bool(&self, value: bool) -> String {
        self.enabled(if value { "true" } else { "false" })
    }

    fn count<T: std::fmt::Display>(&self, value: T) -> String {
        self.paint("1;37", &value.to_string())
    }

    fn count_text(&self, text: &str) -> String {
        if text == "none" {
            self.warn(text)
        } else {
            self.count(text)
        }
    }

    fn paint(&self, code: &str, text: &str) -> String {
        if self.enabled {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }
}
