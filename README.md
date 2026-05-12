# Agentic Session Manager

A lightweight local session router for CLI coding agents.

The goal is to keep agent sessions clean by checking each prompt before it is sent
to a CLI agent. If the prompt looks unrelated to the active session, the manager
suggests continuing in the current session, resuming a better matching existing
session, or starting a new one.

Initial scope:

- Works locally first.
- Reads recent session transcripts from supported agents.
- Keeps a short configurable memory window, such as 1-14 days.
- Uses vector search plus metadata filters to find related sessions.
- Starts as a wrapper around existing tools instead of replacing them.

See [docs/architecture.md](docs/architecture.md) for the initial architecture and
build plan.
