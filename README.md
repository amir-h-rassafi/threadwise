# Threadwise

A lightweight local session advisor for CLI coding agents.

The goal is to keep normal agent usage intact while giving the user a second
pair of eyes on session hygiene. Threadwise watches recent local agent
transcripts and project state, then suggests whether to continue in the current
session, resume a related session, or open a new agent with a focused handoff.

Initial scope:

- Works locally first.
- Starts with Codex CLI for the MVP.
- Reads recent Codex session transcripts.
- Uses Codex pre-prompt hooks where available, with transcript watching as a
  fallback.
- Installs quickly through Homebrew and apt once packaged.
- Ships multi-arch builds for Linux and macOS.
- Keeps a short configurable memory window, such as 1-14 days.
- Starts with fast local search plus metadata filters to find related sessions.
- Starts as a read-only advisor, not a wrapper around any single agent.
- Produces actionable handoff prompts when a new agent is the right move.
- Adds other agents, editors, and MCP after the Codex MVP is reliable.

See [docs/architecture.md](docs/architecture.md) for architecture and
[docs/mvp-plan.md](docs/mvp-plan.md) for the implementation plan.
