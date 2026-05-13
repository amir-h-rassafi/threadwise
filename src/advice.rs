use crate::hooks::{HookEvent, HookKind};
use crate::session_index::{IndexedSource, IndexedTranscript, SessionIndex};
use crate::vector::{InMemoryVectorIndex, VectorIndex, embed_text};

const SUMMARY_EMBEDDING_DIM: usize = 256;

pub enum RecommendationAction {
    ResumeExisting,
    OpenNewAgent,
}

pub struct Recommendation {
    pub action: RecommendationAction,
    pub confidence: u8,
    pub reason: String,
    pub session_id: Option<String>,
    pub handoff: Option<String>,
}

pub fn recommend_for_hook(index: &SessionIndex, event: &HookEvent) -> Option<Recommendation> {
    if event.kind != HookKind::UserPromptSubmit {
        return None;
    }

    let prompt = event.prompt.as_deref()?.trim();
    if prompt.is_empty() {
        return None;
    }

    let intent = PromptIntent::from_prompt(prompt);
    match intent {
        PromptIntent::Split => Some(open_new_agent(prompt)),
        PromptIntent::Resume => resume_existing(index, event),
        PromptIntent::None => None,
    }
}

impl Recommendation {
    pub fn render(&self) -> String {
        let action = match self.action {
            RecommendationAction::ResumeExisting => "resume existing session",
            RecommendationAction::OpenNewAgent => "open a new agent",
        };

        let mut output = format!(
            "Recommendation: {action}\nConfidence: {}\nReason: {}",
            self.confidence, self.reason
        );

        if let Some(session_id) = &self.session_id {
            output.push_str(&format!("\nSession: {session_id}"));
        }
        if let Some(handoff) = &self.handoff {
            output.push_str("\n\nSuggested handoff:\n");
            output.push_str(handoff);
        }
        output
    }
}

enum PromptIntent {
    Split,
    Resume,
    None,
}

impl PromptIntent {
    fn from_prompt(prompt: &str) -> Self {
        let prompt = prompt.to_ascii_lowercase();
        if contains_any(
            &prompt,
            &[
                "new agent",
                "another agent",
                "separate agent",
                "parallel agent",
                "open an agent",
                "split this",
                "split out",
                "handoff",
            ],
        ) {
            return Self::Split;
        }
        if contains_any(
            &prompt,
            &[
                "resume previous",
                "resume existing",
                "continue previous",
                "previous session",
                "old session",
                "related session",
            ],
        ) {
            return Self::Resume;
        }
        Self::None
    }
}

fn open_new_agent(prompt: &str) -> Recommendation {
    Recommendation {
        action: RecommendationAction::OpenNewAgent,
        confidence: 90,
        reason: "prompt explicitly asks for a separate focused agent".to_string(),
        session_id: None,
        handoff: Some(prompt.to_string()),
    }
}

fn resume_existing(index: &SessionIndex, event: &HookEvent) -> Option<Recommendation> {
    let prompt = event.prompt.as_deref()?.trim();
    let related = index.related_transcripts();
    if related.is_empty() {
        return None;
    }

    let (_, transcript) = pick_best_match(&related, prompt).unwrap_or(related[0]);
    let session_id = transcript.session_id.as_deref()?;

    if event.session_id.as_deref() == Some(session_id) {
        return None;
    }

    Some(Recommendation {
        action: RecommendationAction::ResumeExisting,
        confidence: 85,
        reason: format!(
            "prompt asks for prior context and the best related session is {}",
            transcript.relation.as_str()
        ),
        session_id: Some(session_id.to_string()),
        handoff: None,
    })
}

fn pick_best_match<'a>(
    related: &[(&'a IndexedSource, &'a IndexedTranscript)],
    prompt: &str,
) -> Option<(&'a IndexedSource, &'a IndexedTranscript)> {
    if related
        .iter()
        .all(|(_, transcript)| transcript.summary_text.is_empty())
    {
        return None;
    }

    let query = embed_text(prompt, SUMMARY_EMBEDDING_DIM);
    let mut vectors = InMemoryVectorIndex::new();
    for (position, (_, transcript)) in related.iter().enumerate() {
        if transcript.summary_text.is_empty() {
            continue;
        }
        vectors.add(
            position.to_string(),
            embed_text(&transcript.summary_text, SUMMARY_EMBEDDING_DIM),
        );
    }

    let best = vectors.search(&query, 1).into_iter().next()?;
    if best.score <= 0.0 {
        return None;
    }
    let position = best.id.parse::<usize>().ok()?;
    related.get(position).copied()
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::{PromptIntent, RecommendationAction, pick_best_match};
    use crate::session_index::{IndexedSource, IndexedTranscript, WorkspaceRelation};
    use std::path::PathBuf;

    #[test]
    fn detects_explicit_split_intent() {
        assert!(matches!(
            PromptIntent::from_prompt("handoff this to a new agent"),
            PromptIntent::Split
        ));
    }

    #[test]
    fn detects_explicit_resume_intent() {
        assert!(matches!(
            PromptIntent::from_prompt("resume previous session"),
            PromptIntent::Resume
        ));
    }

    #[test]
    fn stays_silent_for_normal_prompt() {
        assert!(matches!(
            PromptIntent::from_prompt("please update the tests"),
            PromptIntent::None
        ));
    }

    #[test]
    fn renders_open_new_agent_advice() {
        let advice = super::open_new_agent("review tests");
        assert!(matches!(advice.action, RecommendationAction::OpenNewAgent));
        assert!(advice.render().contains("Recommendation: open a new agent"));
    }

    #[test]
    fn pick_best_match_uses_summary_text_similarity() {
        let parser = transcript("parser", "parser bison grammar tokens");
        let deploy = transcript("deploy", "deployment kubernetes helm rollout");
        let source = IndexedSource {
            agent: "codex".to_string(),
            kind: "local".to_string(),
            path: PathBuf::from("/sessions"),
            state: "enabled".to_string(),
            advice_enabled: true,
            available: true,
            transcripts: Vec::new(),
        };
        let related = vec![(&source, &parser), (&source, &deploy)];

        let (_, picked) = pick_best_match(&related, "resume parser changes please").expect("hit");
        assert_eq!(picked.session_id.as_deref(), Some("parser"));

        let (_, picked) = pick_best_match(&related, "resume kubernetes rollout").expect("hit");
        assert_eq!(picked.session_id.as_deref(), Some("deploy"));
    }

    #[test]
    fn pick_best_match_returns_none_when_no_summary_text() {
        let empty = transcript("only-id", "");
        let source = IndexedSource {
            agent: "codex".to_string(),
            kind: "local".to_string(),
            path: PathBuf::from("/sessions"),
            state: "enabled".to_string(),
            advice_enabled: true,
            available: true,
            transcripts: Vec::new(),
        };
        let related = vec![(&source, &empty)];
        assert!(pick_best_match(&related, "anything").is_none());
    }

    fn transcript(session_id: &str, summary: &str) -> IndexedTranscript {
        IndexedTranscript {
            display_path: format!("{session_id}.jsonl"),
            bytes: 0,
            modified: 0,
            session_id: Some(session_id.to_string()),
            cwd: None,
            cli_version: None,
            events: 0,
            user_messages: 0,
            agent_messages: 0,
            task_started: 0,
            task_complete: 0,
            parse_errors: 0,
            relation: WorkspaceRelation::Same,
            score: 100,
            summary_text: summary.to_string(),
        }
    }
}
