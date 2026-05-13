use std::env;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const CODEX_ADAPTER_VERSION: &str = concat!("tw-codex-adapter ", env!("CARGO_PKG_VERSION"));

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let code = match run(&args) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err}");
            1
        }
    };
    std::process::exit(code);
}

fn run(args: &[String]) -> Result<i32, String> {
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

    if sources.is_empty() {
        println!("No registered sources.");
        println!("Add one with `tw source add local <path> --agent <kind>`.");
        return Ok(0);
    }

    println!("Registered sources");
    for source in sources {
        println!("agent: {}", source.agent);
        println!("kind: {}", source.kind);
        println!("path: {}", source.path.display());
        println!("source_state: {}", source.state);
        println!(
            "advice: {}",
            if enablements.is_enabled(&source.agent)
                || enablements.is_enabled(&source.path.display().to_string())
            {
                "enabled"
            } else {
                "disabled"
            }
        );
        println!(
            "available: {}",
            if source.path.is_dir() { "yes" } else { "no" }
        );
        println!();
    }

    Ok(0)
}

fn status() -> Result<i32, String> {
    let paths = AppPaths::resolve()?;
    let sources = read_sources(&paths)?;
    let enablements = read_enablements(&paths)?;
    let enabled_count = sources
        .iter()
        .filter(|source| {
            enablements.is_enabled(&source.agent)
                || enablements.is_enabled(&source.path.display().to_string())
        })
        .count();

    println!("Threadwise status");
    println!("registered_sources: {}", sources.len());
    println!("enabled_sources: {enabled_count}");
    println!(
        "advice: {}",
        if enabled_count > 0 {
            "enabled"
        } else {
            "disabled"
        }
    );

    if sources.is_empty() {
        println!("next: tw source add local ~/.codex/sessions --agent codex");
    } else if enabled_count == 0 {
        println!("next: tw enable codex");
    } else {
        println!("next: transcript ingestion");
    }

    Ok(0)
}

fn print_agent_detection(detection: &AgentDetection) {
    println!("agent: {}", detection.kind);
    println!("adapter: {}", detection.adapter_version);
    println!(
        "binary: {}",
        detection
            .executable_path
            .as_ref()
            .map_or("missing".to_string(), |path| path.display().to_string())
    );
    println!(
        "version: {}",
        detection.version.as_deref().unwrap_or("unknown")
    );
    println!(
        "config: {}",
        detection
            .config_path
            .as_ref()
            .map_or("unknown".to_string(), |path| path.display().to_string())
    );
    println!(
        "sessions: {}",
        detection
            .sessions_path
            .as_ref()
            .map_or("unknown".to_string(), |path| path.display().to_string())
    );
    println!("capabilities: {}", detection.capabilities.join(","));
}

fn print_file(path: &Path) -> Result<(), String> {
    let mut content = String::new();
    fs::File::open(path)
        .and_then(|mut file| file.read_to_string(&mut content))
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    print!("{content}");
    io::stdout()
        .flush()
        .map_err(|err| format!("failed to flush stdout: {err}"))?;
    Ok(())
}

fn read_sources(paths: &AppPaths) -> Result<Vec<SourceRecord>, String> {
    if !paths.sources_file.is_file() {
        return Ok(Vec::new());
    }

    let content = read_to_string(&paths.sources_file)?;
    let mut sources = Vec::new();
    for line in content.lines().filter(|line| !line.trim().is_empty()) {
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() < 5 {
            continue;
        }
        sources.push(SourceRecord {
            agent: fields[0].to_string(),
            kind: fields[1].to_string(),
            path: PathBuf::from(fields[2]),
            state: fields[3].to_string(),
        });
    }
    Ok(sources)
}

fn read_enablements(paths: &AppPaths) -> Result<Enablements, String> {
    if !paths.enablements_file.is_file() {
        return Ok(Enablements {
            records: Vec::new(),
        });
    }

    let content = read_to_string(&paths.enablements_file)?;
    let mut records = Vec::new();
    for line in content.lines().filter(|line| !line.trim().is_empty()) {
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() < 2 {
            continue;
        }
        records.push(EnablementRecord {
            scope: fields[0].to_string(),
            enabled: fields[1] == "enabled",
        });
    }
    Ok(Enablements { records })
}

fn read_to_string(path: &Path) -> Result<String, String> {
    let mut content = String::new();
    fs::File::open(path)
        .and_then(|mut file| file.read_to_string(&mut content))
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    Ok(content)
}

fn upsert_line(path: &Path, key: &str, line: &str) -> Result<(), String> {
    let mut lines = Vec::new();
    if path.is_file() {
        let mut content = String::new();
        fs::File::open(path)
            .and_then(|mut file| file.read_to_string(&mut content))
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;

        for existing in content.lines() {
            if !existing.starts_with(key) {
                lines.push(existing.to_string());
            }
        }
    }

    lines.push(line.to_string());

    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .map_err(|err| format!("failed to write {}: {err}", path.display()))?;

    for existing in lines {
        writeln!(file, "{existing}")
            .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    }

    Ok(())
}

fn source_record(agent: &str, kind: &str, path: &Path) -> String {
    format!(
        "{agent}\t{kind}\t{}\tenabled\t{}",
        path.display(),
        unix_timestamp()
    )
}

fn source_key(agent: &str, kind: &str, path: &Path) -> String {
    format!("{agent}\t{kind}\t{}", path.display())
}

fn normalize_existing_dir(path: &str) -> Result<PathBuf, String> {
    let path = expand_tilde(path)?;
    if !path.is_dir() {
        return Err(format!(
            "source path is not a directory: {}",
            path.display()
        ));
    }
    fs::canonicalize(&path).map_err(|err| format!("failed to resolve {}: {err}", path.display()))
}

fn expand_tilde(path: &str) -> Result<PathBuf, String> {
    if path == "~" {
        return home_dir();
    }
    if let Some(rest) = path.strip_prefix("~/") {
        return Ok(home_dir()?.join(rest));
    }
    Ok(PathBuf::from(path))
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    for dir in env::split_paths(&path_var) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }

    #[cfg(not(unix))]
    {
        true
    }
}

fn command_version(path: &Path) -> Option<String> {
    let output = Command::new(path).arg("--version").output().ok()?;
    let text = if output.stdout.is_empty() {
        String::from_utf8_lossy(&output.stderr).to_string()
    } else {
        String::from_utf8_lossy(&output.stdout).to_string()
    };
    let version = text.trim();
    if version.is_empty() {
        None
    } else {
        Some(version.to_string())
    }
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

struct AppPaths {
    config_dir: PathBuf,
    data_dir: PathBuf,
    sources_file: PathBuf,
    adapters_file: PathBuf,
    enablements_file: PathBuf,
    default_codex_config: PathBuf,
    default_codex_sessions: PathBuf,
}

impl AppPaths {
    fn resolve() -> Result<Self, String> {
        let home = home_dir()?;
        let config_dir = env_path("TW_CONFIG_HOME")
            .or_else(|| {
                xdg_path("XDG_CONFIG_HOME", &home, ".config").map(|path| path.join("threadwise"))
            })
            .unwrap_or_else(|| home.join(".config").join("threadwise"));
        let data_dir = env_path("TW_DATA_HOME")
            .or_else(|| {
                xdg_path("XDG_DATA_HOME", &home, ".local/share").map(|path| path.join("threadwise"))
            })
            .unwrap_or_else(|| home.join(".local").join("share").join("threadwise"));

        Ok(Self {
            sources_file: config_dir.join("sources.tsv"),
            adapters_file: config_dir.join("adapters.tsv"),
            enablements_file: config_dir.join("enablements.tsv"),
            default_codex_config: home.join(".codex").join("config.toml"),
            default_codex_sessions: home.join(".codex").join("sessions"),
            config_dir,
            data_dir,
        })
    }
}

fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn xdg_path(name: &str, home: &Path, fallback: &str) -> Option<PathBuf> {
    env_path(name).or_else(|| Some(home.join(fallback)))
}

fn home_dir() -> Result<PathBuf, String> {
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set".to_string())
}

struct SourceRecord {
    agent: String,
    kind: String,
    path: PathBuf,
    state: String,
}

struct Enablements {
    records: Vec<EnablementRecord>,
}

impl Enablements {
    fn is_enabled(&self, scope: &str) -> bool {
        self.records
            .iter()
            .rev()
            .find(|record| record.scope == scope)
            .is_some_and(|record| record.enabled)
    }
}

struct EnablementRecord {
    scope: String,
    enabled: bool,
}

trait AgentAdapter {
    fn kind(&self) -> &'static str;
    fn detect(&self, paths: &AppPaths) -> AgentDetection;
    fn init_instructions(&self) -> String;
}

fn available_adapters() -> Vec<Box<dyn AgentAdapter>> {
    vec![Box::new(CodexAdapter)]
}

fn adapter_for_kind(kind: &str) -> Result<Box<dyn AgentAdapter>, String> {
    available_adapters()
        .into_iter()
        .find(|adapter| adapter.kind() == kind)
        .ok_or_else(|| format!("unsupported agent adapter: {kind}"))
}

struct CodexAdapter;

impl AgentAdapter for CodexAdapter {
    fn kind(&self) -> &'static str {
        "codex"
    }

    fn detect(&self, paths: &AppPaths) -> AgentDetection {
        let executable_path = find_in_path("codex");
        let version = executable_path
            .as_ref()
            .and_then(|path| command_version(path));
        let mut capabilities = Vec::new();

        if executable_path.is_some() {
            capabilities.push("cli".to_string());
            capabilities.push("hooks".to_string());
        }
        if paths.default_codex_sessions.is_dir() {
            capabilities.push("transcripts".to_string());
        }

        AgentDetection {
            kind: self.kind(),
            adapter_version: CODEX_ADAPTER_VERSION.to_string(),
            executable_path,
            version,
            config_path: Some(paths.default_codex_config.clone()),
            sessions_path: Some(paths.default_codex_sessions.clone()),
            capabilities,
        }
    }

    fn init_instructions(&self) -> String {
        "\
Add Threadwise to the Codex hooks configuration:

[hooks.UserPromptSubmit]
command = \"tw hook codex-user-prompt-submit\"

[hooks.Stop]
command = \"tw hook codex-stop\"

Run `tw connect codex` after updating hook configuration.
"
        .to_string()
    }
}

struct AgentDetection {
    kind: &'static str,
    adapter_version: String,
    executable_path: Option<PathBuf>,
    version: Option<String>,
    config_path: Option<PathBuf>,
    sessions_path: Option<PathBuf>,
    capabilities: Vec<String>,
}

impl AgentDetection {
    fn has_sessions(&self) -> bool {
        self.sessions_path
            .as_ref()
            .is_some_and(|path| path.is_dir())
    }

    fn adapter_record(&self) -> String {
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            self.kind,
            self.version.as_deref().unwrap_or("unknown"),
            self.adapter_version,
            self.executable_path
                .as_ref()
                .map_or("missing".to_string(), |path| path.display().to_string()),
            self.capabilities.join(","),
            unix_timestamp()
        )
    }
}

#[allow(dead_code)]
fn _os_string_debug(value: Option<OsString>) -> String {
    value
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "unset".to_string())
}
