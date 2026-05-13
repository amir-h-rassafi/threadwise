use std::path::{Path, PathBuf};

use crate::registry::{Enablements, SourceRecord};
use crate::transcripts::{TranscriptSummary, discover_transcripts, parse_transcript};

const MAX_PARENT_WORKSPACE_DISTANCE: usize = 1;

pub struct SessionIndex {
    pub workspace: PathBuf,
    pub sources: Vec<IndexedSource>,
    pub total_transcripts: usize,
    pub parsed_transcripts: usize,
    pub enabled_sources: usize,
}

pub struct IndexedSource {
    pub agent: String,
    pub kind: String,
    pub path: PathBuf,
    pub state: String,
    pub advice_enabled: bool,
    pub available: bool,
    pub transcripts: Vec<IndexedTranscript>,
}

pub struct IndexedTranscript {
    pub display_path: String,
    pub bytes: u64,
    pub modified: u64,
    pub session_id: Option<String>,
    pub cwd: Option<String>,
    pub cli_version: Option<String>,
    pub events: usize,
    pub user_messages: usize,
    pub agent_messages: usize,
    pub task_started: usize,
    pub task_complete: usize,
    pub parse_errors: usize,
    pub relation: WorkspaceRelation,
    pub score: u16,
    pub summary_text: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceRelation {
    Same,
    Nested,
    Parent,
    Unknown,
    Different,
}

impl WorkspaceRelation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Same => "same_workspace",
            Self::Nested => "nested_workspace",
            Self::Parent => "parent_workspace",
            Self::Unknown => "unknown_workspace",
            Self::Different => "different_workspace",
        }
    }

    pub fn is_related(self) -> bool {
        matches!(self, Self::Same | Self::Nested | Self::Parent)
    }
}

impl SessionIndex {
    pub fn related_transcripts(&self) -> Vec<(&IndexedSource, &IndexedTranscript)> {
        let mut related = self
            .sources
            .iter()
            .flat_map(|source| {
                source
                    .transcripts
                    .iter()
                    .map(move |transcript| (source, transcript))
            })
            .filter(|(_, transcript)| transcript.relation.is_related())
            .collect::<Vec<_>>();

        related.sort_by(|(_, left), (_, right)| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| right.modified.cmp(&left.modified))
                .then_with(|| right.events.cmp(&left.events))
        });
        related
    }
}

pub fn build_session_index(
    sources: &[SourceRecord],
    enablements: &Enablements,
    workspace: &Path,
) -> Result<SessionIndex, String> {
    let workspace = normalize_path(workspace);
    let mut indexed_sources = Vec::new();
    let mut total_transcripts = 0;
    let mut parsed_transcripts = 0;
    let mut enabled_sources = 0;

    for source in sources {
        let advice_enabled = enablements.is_enabled(&source.agent)
            || enablements.is_enabled(&source.path.display().to_string());
        if advice_enabled {
            enabled_sources += 1;
        }

        let files = discover_transcripts(&source.path)?;
        total_transcripts += files.len();

        let mut transcripts = Vec::new();
        for file in files {
            let summary = parse_transcript(file.path())?;
            if summary.parse_errors == 0 {
                parsed_transcripts += 1;
            }

            let relation = relation_to_workspace(summary.cwd.as_deref(), &workspace);
            let score = score_transcript(relation, &summary);
            transcripts.push(IndexedTranscript {
                display_path: file.display_path(&source.path),
                bytes: file.bytes,
                modified: file.modified,
                session_id: summary.session_id,
                cwd: summary.cwd,
                cli_version: summary.cli_version,
                events: summary.events,
                user_messages: summary.user_messages,
                agent_messages: summary.agent_messages,
                task_started: summary.task_started,
                task_complete: summary.task_complete,
                parse_errors: summary.parse_errors,
                relation,
                score,
                summary_text: summary.summary_text,
            });
        }

        indexed_sources.push(IndexedSource {
            agent: source.agent.clone(),
            kind: source.kind.clone(),
            path: source.path.clone(),
            state: source.state.clone(),
            advice_enabled,
            available: source.path.is_dir(),
            transcripts,
        });
    }

    Ok(SessionIndex {
        workspace,
        sources: indexed_sources,
        total_transcripts,
        parsed_transcripts,
        enabled_sources,
    })
}

fn relation_to_workspace(cwd: Option<&str>, workspace: &Path) -> WorkspaceRelation {
    let Some(cwd) = cwd.filter(|value| !value.trim().is_empty()) else {
        return WorkspaceRelation::Unknown;
    };

    let session_path = normalize_path(Path::new(cwd));
    if session_path == workspace {
        return WorkspaceRelation::Same;
    }
    if session_path.starts_with(workspace) {
        return WorkspaceRelation::Nested;
    }
    if workspace.starts_with(&session_path)
        && component_distance(&session_path, workspace) <= MAX_PARENT_WORKSPACE_DISTANCE
    {
        return WorkspaceRelation::Parent;
    }
    WorkspaceRelation::Different
}

fn score_transcript(relation: WorkspaceRelation, summary: &TranscriptSummary) -> u16 {
    let base = match relation {
        WorkspaceRelation::Same => 100,
        WorkspaceRelation::Nested => 80,
        WorkspaceRelation::Parent => 70,
        WorkspaceRelation::Unknown => 10,
        WorkspaceRelation::Different => 0,
    };

    if !relation.is_related() {
        return base;
    }

    let message_bonus = ((summary.user_messages + summary.agent_messages) / 4).min(20) as u16;
    let task_bonus = if summary.task_complete > 0 { 5 } else { 0 };
    base + message_bonus + task_bonus
}

fn normalize_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn component_distance(parent: &Path, child: &Path) -> usize {
    child
        .components()
        .count()
        .saturating_sub(parent.components().count())
}

#[cfg(test)]
mod tests {
    use super::{WorkspaceRelation, relation_to_workspace, score_transcript};
    use crate::transcripts::TranscriptSummary;
    use std::path::Path;

    #[test]
    fn classifies_workspace_relations() {
        let workspace = Path::new("/work/project");

        assert_eq!(
            relation_to_workspace(Some("/work/project"), workspace),
            WorkspaceRelation::Same
        );
        assert_eq!(
            relation_to_workspace(Some("/work/project/crate"), workspace),
            WorkspaceRelation::Nested
        );
        assert_eq!(
            relation_to_workspace(Some("/work"), workspace),
            WorkspaceRelation::Parent
        );
        assert_eq!(
            relation_to_workspace(Some("/"), workspace),
            WorkspaceRelation::Different
        );
        assert_eq!(
            relation_to_workspace(Some("/other/project"), workspace),
            WorkspaceRelation::Different
        );
        assert_eq!(
            relation_to_workspace(None, workspace),
            WorkspaceRelation::Unknown
        );
    }

    #[test]
    fn scores_related_transcripts_above_unrelated_transcripts() {
        let summary = TranscriptSummary {
            user_messages: 20,
            agent_messages: 20,
            task_complete: 1,
            ..TranscriptSummary::default()
        };

        assert!(score_transcript(WorkspaceRelation::Same, &summary) > 100);
        assert_eq!(score_transcript(WorkspaceRelation::Different, &summary), 0);
        assert_eq!(score_transcript(WorkspaceRelation::Unknown, &summary), 10);
    }
}
