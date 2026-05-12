# Threadwise

A lightweight local session advisor for CLI coding agents.

The project is Threadwise; the developer-facing CLI is `tw`.

Threadwise keeps normal agent usage intact while adding a second pair of eyes
on session hygiene. It watches recent local agent transcripts and project state,
then suggests whether to continue in the current session, resume a related
session, or open a new agent with a focused handoff.

See [docs/architecture.md](docs/architecture.md) for architecture and
[docs/mvp-plan.md](docs/mvp-plan.md) for the implementation plan.

## Development

Run the local CLI through Cargo:

```text
cargo run -- doctor
cargo run -- connect codex
cargo run -- source add local ~/.codex/sessions --agent codex
```

Release builds are produced by the GitHub Actions release workflow when a
`v*` tag is pushed.
