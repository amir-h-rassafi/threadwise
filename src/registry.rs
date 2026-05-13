use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::paths::{AppPaths, home_dir};

pub struct SourceRecord {
    pub agent: String,
    pub kind: String,
    pub path: PathBuf,
    pub state: String,
}

pub struct Enablements {
    records: Vec<EnablementRecord>,
}

impl Enablements {
    pub fn is_enabled(&self, scope: &str) -> bool {
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

pub fn read_sources(paths: &AppPaths) -> Result<Vec<SourceRecord>, String> {
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

pub fn read_enablements(paths: &AppPaths) -> Result<Enablements, String> {
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

pub fn print_file(path: &Path) -> Result<(), String> {
    let content = read_to_string(path)?;
    print!("{content}");
    io::stdout()
        .flush()
        .map_err(|err| format!("failed to flush stdout: {err}"))?;
    Ok(())
}

pub fn read_to_string(path: &Path) -> Result<String, String> {
    let mut content = String::new();
    fs::File::open(path)
        .and_then(|mut file| file.read_to_string(&mut content))
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    Ok(content)
}

pub fn upsert_line(path: &Path, key: &str, line: &str) -> Result<(), String> {
    let mut lines = Vec::new();
    if path.is_file() {
        let content = read_to_string(path)?;
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

pub fn source_record(agent: &str, kind: &str, path: &Path) -> String {
    format!(
        "{agent}\t{kind}\t{}\tenabled\t{}",
        path.display(),
        unix_timestamp()
    )
}

pub fn source_key(agent: &str, kind: &str, path: &Path) -> String {
    format!("{agent}\t{kind}\t{}", path.display())
}

pub fn normalize_existing_dir(path: &str) -> Result<PathBuf, String> {
    let path = expand_tilde(path)?;
    if !path.is_dir() {
        return Err(format!(
            "source path is not a directory: {}",
            path.display()
        ));
    }
    fs::canonicalize(&path).map_err(|err| format!("failed to resolve {}: {err}", path.display()))
}

pub fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
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
