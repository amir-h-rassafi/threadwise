use std::env;
use std::fs;

use crate::adapters::{adapter_for_kind, available_adapters, print_agent_detection};
use crate::advice::recommend_for_hook;
use crate::hooks::{HookKind, read_hook_event};
use crate::paths::AppPaths;
use crate::registry::{
    normalize_existing_dir, print_file, read_enablements, read_sources, source_key, source_record,
    unix_timestamp, upsert_line,
};
use crate::session_index::build_session_index;

pub fn run(args: &[String]) -> Result<i32, String> {
    match args {
        [] => {
            print_help();
            Ok(0)
        }
        [flag] if flag == "--help" || flag == "-h" || flag == "help" => {
            print_help();
            Ok(0)
        }
        [flag] if flag == "--version" || flag == "-V" => {
            println!("tw {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
        [cmd] if cmd == "doctor" => doctor(),
        [cmd] if cmd == "adapters" => adapters(),
        [cmd, agent] if cmd == "connect" => connect_agent(agent),
        [cmd, agent] if cmd == "init" => init_agent(agent),
        [cmd, scope] if cmd == "enable" => set_enablement(scope, true),
        [cmd, scope] if cmd == "disable" => set_enablement(scope, false),
        [cmd, sub, kind, path, flag, agent]
            if cmd == "source" && sub == "add" && kind == "local" && flag == "--agent" =>
        {
            source_add_local(path, agent)
        }
        [cmd] if cmd == "sessions" => sessions(),
        [cmd] if cmd == "status" => status(),
        [cmd] if cmd == "graph" => graph(10),
        [cmd, flag, n] if cmd == "graph" && flag == "--top" => {
            let limit = n
                .parse::<usize>()
                .map_err(|_| format!("--top expects a number, got {n}"))?;
            graph(limit)
        }
        [cmd] if cmd == "explain" => explain(),
        [cmd] if cmd == "handoff" => {
            planned(cmd);
            Ok(2)
        }
        [cmd, sub] if cmd == "hook" => dispatch_hook(sub),
        _ => {
            print_help();
            Ok(2)
        }
    }
}

fn print_help() {
    println!(
        "\
Threadwise CLI

Usage:
  tw --version
  tw doctor
  tw connect <agent>                 # codex | claude-code
  tw source add local <path> --agent <kind>
  tw enable <agent-or-source>
  tw disable <agent-or-source>
  tw adapters
  tw init <agent>                    # codex | claude-code
  tw status
  tw sessions
  tw graph [--top N]                 # mermaid flowchart of workspace + related sessions
  tw handoff
  tw explain

Hook commands:
  tw hook codex-user-prompt-submit
  tw hook codex-stop
  tw hook claude-code-user-prompt-submit
  tw hook claude-code-stop
"
    );
}

fn planned(command: &str) {
    println!("planned: tw {command}");
}

fn doctor() -> Result<i32, String> {
    let paths = AppPaths::resolve()?;

    println!("Threadwise doctor");
    println!("config_dir: {}", paths.config_dir.display());
    println!("data_dir: {}", paths.data_dir.display());
    println!("sources_file: {}", paths.sources_file.display());
    println!("adapters_file: {}", paths.adapters_file.display());
    println!();
    println!("Detected adapters");

    let mut status = "warn";
    for adapter in available_adapters() {
        let detection = adapter.detect(&paths);
        if detection.has_sessions() {
            status = "ok";
        }
        print_agent_detection(&detection);
        println!();
    }

    println!("status: {status}");

    Ok(0)
}

fn adapters() -> Result<i32, String> {
    let paths = AppPaths::resolve()?;

    println!("Detected adapters");
    for adapter in available_adapters() {
        let detection = adapter.detect(&paths);
        print_agent_detection(&detection);
        println!();
    }

    if paths.adapters_file.is_file() {
        println!();
        println!("Registered adapters");
        print_file(&paths.adapters_file)?;
    }

    if paths.sources_file.is_file() {
        println!();
        println!("Registered sources");
        print_file(&paths.sources_file)?;
    }

    if paths.enablements_file.is_file() {
        println!();
        println!("Enablements");
        print_file(&paths.enablements_file)?;
    }

    Ok(0)
}

fn connect_agent(agent: &str) -> Result<i32, String> {
    let adapter = adapter_for_kind(agent)?;
    let paths = AppPaths::resolve()?;
    let detection = adapter.detect(&paths);

    fs::create_dir_all(&paths.config_dir)
        .map_err(|err| format!("failed to create {}: {err}", paths.config_dir.display()))?;
    fs::create_dir_all(&paths.data_dir)
        .map_err(|err| format!("failed to create {}: {err}", paths.data_dir.display()))?;

    let adapter_record = detection.adapter_record();
    upsert_line(&paths.adapters_file, detection.kind, &adapter_record)?;

    if let Some(sessions_path) = detection
        .sessions_path
        .as_ref()
        .filter(|path| path.is_dir())
    {
        let source_record = source_record(detection.kind, "local", sessions_path);
        upsert_line(
            &paths.sources_file,
            &source_key(detection.kind, "local", sessions_path),
            &source_record,
        )?;
    }

    println!("connected {}", detection.kind);
    print_agent_detection(&detection);
    Ok(0)
}

fn init_agent(agent: &str) -> Result<i32, String> {
    let adapter = adapter_for_kind(agent)?;
    println!("{}", adapter.init_instructions());
    Ok(0)
}

fn source_add_local(path: &str, agent: &str) -> Result<i32, String> {
    let paths = AppPaths::resolve()?;
    let source_path = normalize_existing_dir(path)?;

    fs::create_dir_all(&paths.config_dir)
        .map_err(|err| format!("failed to create {}: {err}", paths.config_dir.display()))?;

    let record = source_record(agent, "local", &source_path);
    upsert_line(
        &paths.sources_file,
        &source_key(agent, "local", &source_path),
        &record,
    )?;

    println!("registered source");
    println!("agent: {agent}");
    println!("kind: local");
    println!("path: {}", source_path.display());
    Ok(0)
}

fn set_enablement(scope: &str, enabled: bool) -> Result<i32, String> {
    let paths = AppPaths::resolve()?;
    fs::create_dir_all(&paths.config_dir)
        .map_err(|err| format!("failed to create {}: {err}", paths.config_dir.display()))?;

    let state = if enabled { "enabled" } else { "disabled" };
    let record = format!("{scope}\t{state}\tmanual\t{}", unix_timestamp());
    upsert_line(&paths.enablements_file, scope, &record)?;

    println!("{state} {scope}");
    Ok(0)
}

fn sessions() -> Result<i32, String> {
    let paths = AppPaths::resolve()?;
    let sources = read_sources(&paths)?;
    let enablements = read_enablements(&paths)?;
    let workspace = env::current_dir().map_err(|err| format!("failed to read cwd: {err}"))?;

    if sources.is_empty() {
        println!("No registered sources.");
        println!("Add one with `tw source add local <path> --agent <kind>`.");
        return Ok(0);
    }

    let index = build_session_index(&sources, &enablements, &workspace)?;
    println!("Registered sources");
    for source in &index.sources {
        println!("agent: {}", source.agent);
        println!("kind: {}", source.kind);
        println!("path: {}", source.path.display());
        println!("source_state: {}", source.state);
        println!(
            "advice: {}",
            if source.advice_enabled {
                "enabled"
            } else {
                "disabled"
            }
        );
        println!("available: {}", if source.available { "yes" } else { "no" });
        println!("transcripts: {}", source.transcripts.len());
        for transcript in source.transcripts.iter().take(10) {
            println!(
                "- {} relation={} score={} size={} modified={} events={} parse_errors={} user={} agent={} tasks={}/{} cwd={} id={} version={}",
                transcript.display_path,
                transcript.relation.as_str(),
                transcript.score,
                transcript.bytes,
                transcript.modified,
                transcript.events,
                transcript.parse_errors,
                transcript.user_messages,
                transcript.agent_messages,
                transcript.task_complete,
                transcript.task_started,
                transcript.cwd.as_deref().unwrap_or("unknown"),
                transcript.session_id.as_deref().unwrap_or("unknown"),
                transcript.cli_version.as_deref().unwrap_or("unknown")
            );
        }
        if source.transcripts.len() > 10 {
            println!("- ... {} more", source.transcripts.len() - 10);
        }
        println!();
    }

    Ok(0)
}

fn status() -> Result<i32, String> {
    let paths = AppPaths::resolve()?;
    let sources = read_sources(&paths)?;
    let enablements = read_enablements(&paths)?;
    let workspace = env::current_dir().map_err(|err| format!("failed to read cwd: {err}"))?;
    let index = build_session_index(&sources, &enablements, &workspace)?;
    let related = index.related_transcripts();

    println!("Threadwise status");
    println!("active_workspace: {}", index.workspace.display());
    println!("registered_sources: {}", index.sources.len());
    println!("enabled_sources: {}", index.enabled_sources);
    println!("discovered_transcripts: {}", index.total_transcripts);
    println!("parsed_transcripts: {}", index.parsed_transcripts);
    println!("related_sessions: {}", related.len());
    println!(
        "advice: {}",
        if index.enabled_sources > 0 {
            "enabled"
        } else {
            "disabled"
        }
    );
    if let Some((source, transcript)) = related.first() {
        println!(
            "top_related: agent={} relation={} score={} modified={} cwd={} id={} path={}",
            source.agent,
            transcript.relation.as_str(),
            transcript.score,
            transcript.modified,
            transcript.cwd.as_deref().unwrap_or("unknown"),
            transcript.session_id.as_deref().unwrap_or("unknown"),
            transcript.display_path
        );
    }

    if sources.is_empty() {
        println!("next: tw source add local ~/.codex/sessions --agent codex");
    } else if index.enabled_sources == 0 {
        println!("next: tw enable codex");
    } else if index.total_transcripts == 0 {
        println!("next: add or wait for agent transcript files");
    } else if related.is_empty() {
        println!("next: start or resume an agent in this workspace");
    } else {
        println!("next: tw sessions");
    }

    Ok(0)
}

fn explain() -> Result<i32, String> {
    let paths = AppPaths::resolve()?;
    let sources = read_sources(&paths)?;
    let enablements = read_enablements(&paths)?;
    let workspace = env::current_dir().map_err(|err| format!("failed to read cwd: {err}"))?;
    let index = build_session_index(&sources, &enablements, &workspace)?;
    let related = index.related_transcripts();

    println!("Threadwise explain");
    println!("active_workspace: {}", index.workspace.display());
    println!(
        "discovered_transcripts: {} parsed: {} related: {}",
        index.total_transcripts,
        index.parsed_transcripts,
        related.len()
    );

    let Some((source, transcript)) = related.first() else {
        println!("(no related sessions; nothing to explain)");
        return Ok(0);
    };

    let base = match transcript.relation {
        crate::session_index::WorkspaceRelation::Same => 100u16,
        crate::session_index::WorkspaceRelation::Nested => 80,
        crate::session_index::WorkspaceRelation::Parent => 70,
        crate::session_index::WorkspaceRelation::Unknown => 10,
        crate::session_index::WorkspaceRelation::Different => 0,
    };
    let message_bonus = ((transcript.user_messages + transcript.agent_messages) / 4).min(20) as u16;
    let task_bonus = if transcript.task_complete > 0 { 5 } else { 0 };

    println!();
    println!("top_related:");
    println!("  agent: {}", source.agent);
    println!(
        "  session_id: {}",
        transcript.session_id.as_deref().unwrap_or("unknown")
    );
    println!("  path: {}", transcript.display_path);
    println!("  relation: {}", transcript.relation.as_str());
    println!("  cwd: {}", transcript.cwd.as_deref().unwrap_or("unknown"));
    println!(
        "  cli_version: {}",
        transcript.cli_version.as_deref().unwrap_or("unknown")
    );
    println!("  modified_unix: {}", transcript.modified);
    println!("  bytes: {}", transcript.bytes);
    println!();
    println!("scoring:");
    println!(
        "  relation_base: {base:>3}   ({})",
        transcript.relation.as_str()
    );
    println!(
        "  message_bonus: {message_bonus:>3}   (u={} a={}, capped at 20)",
        transcript.user_messages, transcript.agent_messages
    );
    println!(
        "  task_bonus:    {task_bonus:>3}   (task_complete={})",
        transcript.task_complete
    );
    println!("  total:         {:>3}", transcript.score);
    println!();
    println!("signals:");
    println!("  events: {}", transcript.events);
    println!("  parse_errors: {}", transcript.parse_errors);
    println!(
        "  summary_text_chars: {}",
        transcript.summary_text.chars().count()
    );
    let preview: String = transcript
        .summary_text
        .chars()
        .take(160)
        .collect::<String>()
        .replace('\n', " ");
    println!("  summary_text_preview: {preview:?}");

    Ok(0)
}

fn graph(top: usize) -> Result<i32, String> {
    let paths = AppPaths::resolve()?;
    let sources = read_sources(&paths)?;
    let enablements = read_enablements(&paths)?;
    let workspace = env::current_dir().map_err(|err| format!("failed to read cwd: {err}"))?;
    let index = build_session_index(&sources, &enablements, &workspace)?;

    println!("%% paste into https://mermaid.live or any markdown viewer");
    println!("flowchart LR");
    println!(
        "    W[\"workspace<br/>{}\"]",
        mermaid_escape(&index.workspace.display().to_string())
    );

    for (s_idx, source) in index.sources.iter().enumerate() {
        let advice = if source.advice_enabled { "on" } else { "off" };
        println!(
            "    S{s_idx}[\"{agent}<br/>{path}<br/>advice={advice} transcripts={count}\"]",
            agent = source.agent,
            path = mermaid_escape(&source.path.display().to_string()),
            count = source.transcripts.len(),
        );
        println!("    W --- S{s_idx}");

        let mut related: Vec<&crate::session_index::IndexedTranscript> = source
            .transcripts
            .iter()
            .filter(|t| t.relation.is_related())
            .collect();
        related.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| right.modified.cmp(&left.modified))
        });

        for (t_idx, transcript) in related.iter().take(top).enumerate() {
            let id = format!("T{s_idx}_{t_idx}");
            let session = transcript.session_id.as_deref().unwrap_or("?");
            let short: String = session.chars().take(8).collect();
            println!(
                "    {id}[\"{short}<br/>{relation}<br/>score={score} u={u} a={a}\"]",
                relation = transcript.relation.as_str(),
                score = transcript.score,
                u = transcript.user_messages,
                a = transcript.agent_messages,
            );
            let edge = if t_idx == 0 { "==>" } else { "-->" };
            println!("    S{s_idx} {edge} {id}");
        }
        if related.len() > top {
            println!(
                "    S{s_idx} -.-> R{s_idx}[\"... {} more related\"]",
                related.len() - top
            );
        }
    }

    Ok(0)
}

fn mermaid_escape(text: &str) -> String {
    text.replace('"', "&quot;")
}

fn dispatch_hook(sub: &str) -> Result<i32, String> {
    for adapter in available_adapters() {
        let kind = adapter.kind();
        let Some(event) = sub.strip_prefix(kind).and_then(|s| s.strip_prefix('-')) else {
            continue;
        };
        let hook_kind = match event {
            "user-prompt-submit" => HookKind::UserPromptSubmit,
            "stop" => HookKind::Stop,
            _ => continue,
        };
        let _ = run_hook(kind, hook_kind);
        return Ok(0);
    }
    print_help();
    Ok(2)
}

fn run_hook(agent: &str, kind: HookKind) -> Result<i32, String> {
    let event = read_hook_event(agent, kind)?;
    let paths = AppPaths::resolve()?;
    let sources = read_sources(&paths)?;
    let enablements = read_enablements(&paths)?;

    let enabled = sources.iter().any(|source| {
        source.agent == event.agent
            && (enablements.is_enabled(&source.agent)
                || enablements.is_enabled(&source.path.display().to_string()))
    });
    if !enabled {
        return Ok(0);
    }

    if kind == HookKind::Stop {
        return Ok(0);
    }

    let index = build_session_index(&sources, &enablements, &event.cwd)?;
    if let Some(recommendation) = recommend_for_hook(&index, &event) {
        println!("{}", recommendation.render());
    }

    Ok(0)
}
