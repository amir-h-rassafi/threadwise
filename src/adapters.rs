use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::paths::AppPaths;
use crate::registry::unix_timestamp;

const CODEX_ADAPTER_VERSION: &str = concat!("tw-codex-adapter ", env!("CARGO_PKG_VERSION"));

pub trait AgentAdapter {
    fn kind(&self) -> &'static str;
    fn detect(&self, paths: &AppPaths) -> AgentDetection;
    fn init_instructions(&self) -> String;
}

pub fn available_adapters() -> Vec<Box<dyn AgentAdapter>> {
    vec![Box::new(CodexAdapter)]
}

pub fn adapter_for_kind(kind: &str) -> Result<Box<dyn AgentAdapter>, String> {
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

pub struct AgentDetection {
    pub kind: &'static str,
    pub adapter_version: String,
    pub executable_path: Option<PathBuf>,
    pub version: Option<String>,
    pub config_path: Option<PathBuf>,
    pub sessions_path: Option<PathBuf>,
    pub capabilities: Vec<String>,
}

impl AgentDetection {
    pub fn has_sessions(&self) -> bool {
        self.sessions_path
            .as_ref()
            .is_some_and(|path| path.is_dir())
    }

    pub fn adapter_record(&self) -> String {
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

pub fn print_agent_detection(detection: &AgentDetection) {
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
