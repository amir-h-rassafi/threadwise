use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct TranscriptFile {
    path: PathBuf,
    pub bytes: u64,
    pub modified: u64,
}

impl TranscriptFile {
    pub fn display_path(&self, root: &Path) -> String {
        self.path
            .strip_prefix(root)
            .unwrap_or(&self.path)
            .display()
            .to_string()
    }
}

pub fn discover_transcripts(root: &Path) -> Result<Vec<TranscriptFile>, String> {
    let mut transcripts = Vec::new();
    if !root.is_dir() {
        return Ok(transcripts);
    }

    visit_transcript_dir(root, &mut transcripts)?;
    transcripts.sort_by(|left, right| right.modified.cmp(&left.modified));
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
