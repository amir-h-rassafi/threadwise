use std::io::{self, Read};
use std::path::PathBuf;

use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HookKind {
    CodexUserPromptSubmit,
    CodexStop,
}

pub struct HookEvent {
    pub agent: String,
    pub kind: HookKind,
    pub cwd: PathBuf,
    pub prompt: Option<String>,
    pub session_id: Option<String>,
}

pub fn read_hook_event(agent: &str, kind: HookKind) -> Result<HookEvent, String> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|err| format!("failed to read hook stdin: {err}"))?;

    let value = parse_optional_json(&input)?;
    let cwd = find_string(
        value.as_ref(),
        &[
            &["cwd"],
            &["current_working_directory"],
            &["workspace"],
            &["workspace_root"],
            &["payload", "cwd"],
            &["payload", "workspace"],
            &["metadata", "cwd"],
        ],
    )
    .map(PathBuf::from)
    .map(Ok)
    .unwrap_or_else(|| {
        std::env::current_dir().map_err(|err| format!("failed to read cwd: {err}"))
    })?;

    Ok(HookEvent {
        agent: agent.to_string(),
        kind,
        cwd,
        prompt: find_string(
            value.as_ref(),
            &[
                &["prompt"],
                &["user_prompt"],
                &["input"],
                &["message"],
                &["payload", "prompt"],
                &["payload", "user_prompt"],
                &["payload", "input"],
                &["payload", "message"],
            ],
        ),
        session_id: find_string(
            value.as_ref(),
            &[
                &["session_id"],
                &["sessionId"],
                &["conversation_id"],
                &["payload", "session_id"],
                &["payload", "sessionId"],
                &["payload", "id"],
                &["metadata", "session_id"],
            ],
        ),
    })
}

fn parse_optional_json(input: &str) -> Result<Option<Value>, String> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(None);
    }
    Ok(serde_json::from_str(input).ok())
}

fn find_string(value: Option<&Value>, paths: &[&[&str]]) -> Option<String> {
    paths
        .iter()
        .filter_map(|path| value_at(value?, path))
        .find_map(|value| {
            value
                .as_str()
                .filter(|text| !text.trim().is_empty())
                .map(ToString::to_string)
        })
}

fn value_at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use super::{find_string, parse_optional_json};
    use serde_json::json;

    #[test]
    fn reads_nested_hook_fields() {
        let value = json!({
            "payload": {
                "cwd": "/repo",
                "prompt": "split this out"
            }
        });

        assert_eq!(
            find_string(Some(&value), &[&["payload", "cwd"]]).as_deref(),
            Some("/repo")
        );
        assert_eq!(
            find_string(Some(&value), &[&["payload", "prompt"]]).as_deref(),
            Some("split this out")
        );
    }

    #[test]
    fn invalid_json_is_treated_as_missing_hook_payload() {
        assert!(
            parse_optional_json("not-json")
                .expect("parse fallback")
                .is_none()
        );
    }
}
