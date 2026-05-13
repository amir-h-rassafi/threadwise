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
            if !line_matches_key(existing, key) {
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

fn line_matches_key(line: &str, key: &str) -> bool {
    if !line.starts_with(key) {
        return false;
    }
    line.len() == key.len() || line.as_bytes()[key.len()] == b'\t'
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

#[cfg(test)]
mod tests {
    use super::{line_matches_key, upsert_line};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn key_match_requires_tab_boundary() {
        assert!(line_matches_key("codex\tenabled\t...", "codex"));
        assert!(line_matches_key("codex", "codex"));
        assert!(!line_matches_key("codex-extra\tenabled", "codex"));
        assert!(!line_matches_key("codexx", "codex"));
    }

    #[test]
    fn upsert_does_not_drop_keys_that_share_a_prefix() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("tw-upsert-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("table.tsv");

        upsert_line(&path, "codex", "codex\tenabled\tmanual\t1").expect("write codex");
        upsert_line(&path, "codex-extra", "codex-extra\tenabled\tmanual\t1")
            .expect("write codex-extra");
        upsert_line(&path, "codex", "codex\tdisabled\tmanual\t2").expect("rewrite codex");

        let content = fs::read_to_string(&path).expect("read back");
        assert!(content.contains("codex-extra\tenabled"));
        assert!(content.contains("codex\tdisabled"));
        assert!(!content.contains("codex\tenabled"));

        let _ = fs::remove_dir_all(&dir);
    }
}
