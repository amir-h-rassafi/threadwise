use std::env;
use std::path::{Path, PathBuf};

pub struct AppPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub sources_file: PathBuf,
    pub adapters_file: PathBuf,
    pub enablements_file: PathBuf,
    pub default_codex_config: PathBuf,
    pub default_codex_sessions: PathBuf,
}

impl AppPaths {
    pub fn resolve() -> Result<Self, String> {
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

pub fn home_dir() -> Result<PathBuf, String> {
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set".to_string())
}

fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn xdg_path(name: &str, home: &Path, fallback: &str) -> Option<PathBuf> {
    env_path(name).or_else(|| Some(home.join(fallback)))
}
