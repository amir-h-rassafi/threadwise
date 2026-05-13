use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

struct TestEnv {
    root: PathBuf,
    config: PathBuf,
    data: PathBuf,
    workspace: PathBuf,
    sessions: PathBuf,
}

impl TestEnv {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("threadwise-{name}-{}-{nonce}", std::process::id()));
        let config = root.join("config");
        let data = root.join("data");
        let workspace = root.join("workspace");
        let sessions = root.join("sessions");

        fs::create_dir_all(&config).expect("create config dir");
        fs::create_dir_all(&data).expect("create data dir");
        fs::create_dir_all(&workspace).expect("create workspace dir");
        fs::create_dir_all(&sessions).expect("create sessions dir");
        write_codex_transcript(&sessions, &workspace);

        Self {
            root,
            config,
            data,
            workspace,
            sessions,
        }
    }

    fn tw(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_tw"));
        command
            .args(args)
            .current_dir(&self.workspace)
            .env("TW_CONFIG_HOME", &self.config)
            .env("TW_DATA_HOME", &self.data);
        command
    }

    fn source_add(&self) {
        let output = self
            .tw(&[
                "source",
                "add",
                "local",
                self.sessions.to_str().expect("sessions path utf8"),
                "--agent",
                "codex",
            ])
            .output()
            .expect("run source add");
        assert_success(&output, "source add");
    }

    fn enable_codex(&self) {
        let output = self.tw(&["enable", "codex"]).output().expect("run enable");
        assert_success(&output, "enable");
    }

    fn hook(&self, payload: &str) -> std::process::Output {
        let mut child = self
            .tw(&["hook", "codex-user-prompt-submit"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn hook");

        child
            .stdin
            .as_mut()
            .expect("hook stdin")
            .write_all(payload.as_bytes())
            .expect("write hook payload");
        child.wait_with_output().expect("wait hook")
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn disabled_hook_is_silent() {
    let env = TestEnv::new("disabled-hook");
    env.source_add();

    let output = env.hook(&hook_payload(
        &env.workspace,
        "split this into a new agent",
        "new-session",
    ));

    assert_success(&output, "disabled hook");
    assert_eq!(stdout(&output), "");
}

#[test]
fn enabled_hook_can_suggest_new_agent_for_explicit_split() {
    let env = TestEnv::new("split-hook");
    env.source_add();
    env.enable_codex();

    let output = env.hook(&hook_payload(
        &env.workspace,
        "handoff this docs review to a new agent",
        "current-session",
    ));

    assert_success(&output, "split hook");
    let stdout = stdout(&output);
    assert!(stdout.contains("Recommendation: open a new agent"));
    assert!(stdout.contains("Suggested handoff:"));
}

#[test]
fn enabled_hook_is_silent_for_normal_prompt() {
    let env = TestEnv::new("normal-hook");
    env.source_add();
    env.enable_codex();

    let output = env.hook(&hook_payload(
        &env.workspace,
        "update the docs based on the parser changes",
        "current-session",
    ));

    assert_success(&output, "normal hook");
    assert_eq!(stdout(&output), "");
}

#[test]
fn enabled_hook_is_silent_for_invalid_payload() {
    let env = TestEnv::new("invalid-hook");
    env.source_add();
    env.enable_codex();

    let output = env.hook("not-json");

    assert_success(&output, "invalid hook payload");
    assert_eq!(stdout(&output), "");
}

#[test]
fn enabled_hook_can_suggest_related_resume_for_explicit_resume() {
    let env = TestEnv::new("resume-hook");
    env.source_add();
    env.enable_codex();

    let output = env.hook(&hook_payload(
        &env.workspace,
        "resume previous session for this repo",
        "fresh-session",
    ));

    assert_success(&output, "resume hook");
    let stdout = stdout(&output);
    assert!(stdout.contains("Recommendation: resume existing session"));
    assert!(stdout.contains("Session: old-session"));
}

fn write_codex_transcript(sessions: &Path, workspace: &Path) {
    let path = sessions.join("rollout-old-session.jsonl");
    let cwd = workspace.display();
    let transcript = format!(
        "{{\"timestamp\":\"2026-05-13T00:00:00Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"old-session\",\"cwd\":\"{cwd}\",\"cli_version\":\"codex-cli 0.130.0\"}}}}\n\
         {{\"timestamp\":\"2026-05-13T00:00:01Z\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\"}}}}\n\
         {{\"timestamp\":\"2026-05-13T00:00:02Z\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"agent_message\"}}}}\n\
         {{\"timestamp\":\"2026-05-13T00:00:03Z\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"task_complete\"}}}}\n"
    );
    fs::write(path, transcript).expect("write transcript");
}

fn hook_payload(workspace: &Path, prompt: &str, session_id: &str) -> String {
    format!(
        "{{\"cwd\":\"{}\",\"prompt\":\"{}\",\"session_id\":\"{}\"}}",
        workspace.display(),
        prompt,
        session_id
    )
}

fn assert_success(output: &std::process::Output, label: &str) {
    assert!(
        output.status.success(),
        "{label} failed\nstdout:\n{}\nstderr:\n{}",
        stdout(output),
        stderr(output)
    );
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}
