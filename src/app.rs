use std::env;
use std::fs;

use crate::adapters::{adapter_for_kind, available_adapters, print_agent_detection};
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
        [cmd] if matches!(cmd.as_str(), "handoff" | "explain") => {
            planned(cmd);
            Ok(2)
        }
        [cmd, sub] if cmd == "hook" && sub == "codex-user-prompt-submit" => {
            planned("hook codex-user-prompt-submit");
            Ok(2)
        }
        [cmd, sub] if cmd == "hook" && sub == "codex-stop" => {
            planned("hook codex-stop");
            Ok(2)
        }
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
  tw connect codex
  tw source add local <path> --agent <kind>
  tw enable <agent-or-source>
  tw disable <agent-or-source>
  tw adapters
  tw init codex
  tw status
  tw sessions
  tw handoff
  tw explain

Hook commands:
  tw hook codex-user-prompt-submit
  tw hook codex-stop
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
