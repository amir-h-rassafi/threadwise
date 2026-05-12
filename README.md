# Threadwise

A lightweight local session advisor for CLI coding agents.

The goal is to keep normal agent usage intact while giving the user a second
pair of eyes on session hygiene. Threadwise watches recent local agent
transcripts and project state, then suggests whether to continue in the current
session, resume a related session, or open a new agent with a focused handoff.

Initial scope:

- Works locally first.
- Reads recent session transcripts from supported agents.
- Uses agent pre-prompt hooks where available, with transcript watching as a
  fallback.
- Targets CLI agents and editor-hosted agents, including VS Code/Copilot and
  Cursor where plugin or hook APIs are available.
- Keeps a short configurable memory window, such as 1-14 days.
- Uses vector search plus metadata filters to find related sessions.
- Starts as a read-only advisor, not a wrapper around any single agent.
- Produces actionable handoff prompts when a new agent is the right move.

See [docs/architecture.md](docs/architecture.md) for the initial architecture and
build plan.
