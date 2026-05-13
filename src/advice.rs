use crate::hooks::{HookEvent, HookKind};
use crate::session_index::SessionIndex;

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
    if event.kind != HookKind::CodexUserPromptSubmit {
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
    let related = index.related_transcripts();
    let (_, transcript) = related.first()?;
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

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::{PromptIntent, RecommendationAction};

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
}
