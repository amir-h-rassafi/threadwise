# Threadwise MVP Plan

## Goal

Ship a local Codex-first advisor that can be installed as a single CLI, wired
into an existing Codex setup, index recent sessions, and suggest one of three
actions:

- Continue in the current session.
- Resume a related session.
- Open a focused new agent with a handoff prompt.

The MVP is useful when it can run quietly in the Codex loop without changing
normal Codex usage.

## Non-Goals

- No wrapper around Codex prompt entry.
- No automatic agent spawning.
- No cloud service.
- No cross-agent support before Codex is reliable.
- No long-term personal memory system.
- No editor extension in the first release.

## MVP Success Criteria

- `threadwise init codex` can install or print Codex hook config.
- `threadwise connect codex` can detect Codex binary, version, config, hooks,
  transcript path, and supported capabilities.
- `threadwise source add local <path> --agent codex` can register an explicit
  session source.
- `threadwise status` can show the current session and related sessions for
  the active repo.
- `threadwise hook codex-user-prompt-submit` usually returns no output, but
  returns short advice when confidence is high.
- `threadwise handoff` can print a copy-ready prompt for a new focused agent.
- Hook execution targets under 500 ms and hard-times-out under 2 seconds.
- Everything works offline after the local embedding model is present.

## Technology Choice

Use Rust for the MVP.

Reasons:

- LanceDB has official Rust support.
- `fastembed-rs` gives a local embedding path without Python.
- Rust fits Homebrew, apt, and multi-arch binary releases.
- Go is deferred because LanceDB's Go SDK is community-driven and uses
  CGO/native artifacts.

Storage:

- SQLite for metadata, agent registry, sessions, turns, summaries,
  recommendations, and handoffs.
- LanceDB for vector lookup.
- `sqlite-vec` or SQLite BLOB cosine search as fallback behind the same
  `VectorIndex` trait.

## CLI Shape

Setup commands:

```text
threadwise init codex
threadwise connect codex
threadwise source add local ~/.codex/sessions --agent codex
threadwise doctor
```

Daily commands:

```text
threadwise status
threadwise sessions
threadwise handoff
threadwise explain
threadwise adapters
```

Hook commands:

```text
threadwise hook codex-user-prompt-submit
threadwise hook codex-stop
```

## Data Model

SQLite tables:

- `agents`: agent kind, agent version, adapter version, executable path,
  capabilities, first seen, last seen.
- `sources`: agent id, source kind, path, enabled, last scan.
- `sessions`: agent id, source id, session id, repo root, title, status,
  timestamps.
- `turns`: session id, role, content, files mentioned, commands mentioned,
  timestamp.
- `summaries`: session id, objective, state, touched files, open questions,
  vector id, timestamp.
- `recommendations`: action, confidence, reason, selected session, timestamp.
- `handoffs`: recommendation id, generated prompt, timestamp.

LanceDB table:

- `session_vectors`: vector id, session id, agent id, repo root, summary kind,
  embedding, text hash, created_at.

Agent versions are compatibility and debug metadata. Lookup should rank by
content similarity, repo, recency, files, commands, and task state.

## Milestones

### M0: Rust CLI Skeleton

Deliver:

- Cargo project.
- `threadwise --version`.
- `threadwise doctor`.
- Config directory resolution.
- Basic logging and error formatting.

Done when:

- CLI runs on macOS and Linux dev machines.
- `doctor` reports config path, data path, and unsupported checks clearly.

### M1: Codex Adapter Discovery

Deliver:

- `threadwise connect codex`.
- Detect Codex executable path and version.
- Detect config path and likely transcript path.
- Persist agent and source metadata.
- `threadwise adapters`.

Done when:

- A real local Codex install is detected without manual config.
- Manual `source add local` works when detection fails.

### M2: Transcript Ingestion

Deliver:

- Read recent Codex transcripts from registered sources.
- Normalize turns into the internal event schema.
- Extract repo root, files mentioned, commands mentioned, and timestamps.
- Persist sessions and turns in SQLite.

Done when:

- `threadwise sessions` lists recent sessions for the current repo.
- Re-running ingestion is idempotent.

### M3: Summaries And Search

Deliver:

- Generate deterministic local session summaries first.
- Add SQLite FTS/BM25 over summary and turn text.
- Add `fastembed-rs` embeddings.
- Add LanceDB-backed `VectorIndex`.
- Hybrid scoring with content, repo, recency, files, and commands.

Done when:

- `threadwise status` shows current session plus top related sessions.
- Similar task wording matches even when exact words differ.

### M4: Advice Engine

Deliver:

- Score `continue_current`, `resume_existing`, and `open_new_agent`.
- Produce short reason text.
- Generate handoff prompt for `open_new_agent`.
- Store recommendation and handoff history.
- `threadwise explain`.

Done when:

- Advice is silent when confidence is low.
- Advice is concise and copy-ready when confidence is high.

### M5: Codex Hooks

Deliver:

- `threadwise init codex`.
- `threadwise hook codex-user-prompt-submit`.
- `threadwise hook codex-stop`.
- Timeout enforcement.
- No-op fallback on internal errors.

Done when:

- Hook can be wired into Codex without changing normal Codex usage.
- Pre-prompt hook returns within the timeout budget.

### M6: Packaging

Deliver:

- Release builds for `darwin-arm64`, `darwin-amd64`, `linux-amd64`,
  `linux-arm64`.
- Homebrew formula.
- Debian package.
- Release smoke tests.

Done when:

- Fresh install can run `threadwise doctor`.
- Fresh install can connect to Codex and index a local source.

## First Implementation Slice

Build the smallest useful vertical path:

1. Rust CLI skeleton.
2. SQLite metadata store.
3. `source add local`.
4. Codex transcript scanner.
5. `sessions` and `status` with simple metadata/FTS search.
6. Add embeddings and LanceDB.
7. Add hooks after status is useful from the CLI.

This avoids debugging hooks before the core session lookup works.

## Risks

- Codex transcript format may change.
- LanceDB Rust packaging may increase binary size or complicate release builds.
- Local model download can make first run feel slow.
- Hook output must be conservative; noisy advice will make users disable it.

Mitigations:

- Keep adapters versioned and isolated.
- Keep `VectorIndex` swappable.
- Add `threadwise doctor` checks for model, DB, source, and hook state.
- Default to no output unless confidence is high.
