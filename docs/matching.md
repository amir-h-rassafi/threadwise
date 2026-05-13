# How Threadwise Matches Sessions

Status: this is the **current** (M2/M3 hybrid) matcher. Honest, no
marketing — it's a literal bag-of-words pipeline with a planned swap to a
real ONNX embedding via `fastembed-rs`.

## TL;DR

- The matcher does **vector search**, but the vector is a 256-dim
  **FNV-hashed bag-of-words**, L2-normalized, compared by **cosine
  similarity**.
- It is **not** semantic. The word "weather" matches the word "weather".
  It does **not** match "climate", "rain", "forecast", or any paraphrase.
- Two filters are layered on top: an explicit-phrase intent classifier
  and a workspace-relation pre-filter.
- All matching is local, in-process, single-pass over the registered
  transcript directories. No model download, no network.

## Pipeline (decision flow for one hook event)

```
hook stdin                                 │
JSON { cwd, prompt, session_id }           │
                                           ▼
                              ┌────────────────────────┐
                              │ 1. enablement gate     │  silent if disabled
                              └────────────────────────┘
                                           │
                                           ▼
                              ┌────────────────────────┐
                              │ 2. parse transcripts   │  per-agent JSONL
                              └────────────────────────┘
                                           │
                                           ▼
                              ┌────────────────────────┐
                              │ 3. workspace relation  │  Same/Nested/Parent/…
                              └────────────────────────┘
                                           │
                                           ▼
                              ┌────────────────────────┐
                              │ 4. activity score      │  base + msg/task bonuses
                              └────────────────────────┘
                                           │
                                           ▼
                              ┌────────────────────────┐
                              │ 5. intent classifier   │  Split / Resume / None
                              └────────────────────────┘
                                  │           │           │
                       Split      Resume      None (soft-resume path)
                          │          │            │
                          ▼          ▼            ▼
                              ┌────────────────────────┐
                              │ 6. vector similarity   │  FNV bag-of-words, cosine
                              └────────────────────────┘
                                           │
                                           ▼
                              ┌────────────────────────┐
                              │ 7. decision + render   │  short recommendation
                              └────────────────────────┘
```

## What gets captured for matching

Only `summary_text` is embedded. Everything else (cwd, message counts,
session id, cli version) is structured metadata used by the workspace
filter and the activity score — never tokenized.

Per-agent capture rules:

| Agent       | Event type      | Source field                                                       |
| ----------- | --------------- | ------------------------------------------------------------------ |
| codex       | `event_msg`     | `payload.message` / `payload.text` / `payload.content` (string)    |
| claude-code | `user`          | `message.content` (string **or** array of `type:text` items)       |
| claude-code | `assistant`     | `message.content` array, `type:text` items only                    |

**Skipped on purpose today** (these often hold real intent but the
matcher ignores them):

- Codex `task_started`, `task_complete` (counters only)
- Claude Code `tool_use`, `tool_result`, `thinking`, `attachment`,
  `system`, `permission-mode`, `file-history-snapshot`, `summary`, etc.
- Attachments and image parts in either format

Cap: **16,384 chars per transcript**. Tokens past the cap are invisible
to the matcher. Long sessions where the topic only comes up late will
lose that signal.

## Workspace relation

Each transcript records its `cwd` at session start. We compare to the
current shell's working directory:

| Relation              | Definition                                        | Base score |
| --------------------- | ------------------------------------------------- | ---------- |
| `same_workspace`      | Identical paths                                   | 100        |
| `nested_workspace`    | Session was inside the current workspace          | 80         |
| `parent_workspace`    | Session was 1 dir up from the current workspace   | 70         |
| `unknown_workspace`   | Session has no recorded cwd                       | 10         |
| `different_workspace` | Different tree                                    | 0          |

Only `Same`, `Nested`, and `Parent` enter the candidate pool. The rest
are filtered out before similarity is computed.

Activity score = `base + min(20, (u_msg + a_msg)/4) + (5 if task_complete else 0)`.

## Intent classifier

Case-insensitive substring match against fixed phrase lists:

| Intent  | Trigger phrases (any one matches)                                                              |
| ------- | ---------------------------------------------------------------------------------------------- |
| Split   | new agent, another agent, separate agent, parallel agent, open an agent, split this, split out, handoff |
| Resume  | resume previous, resume existing, continue previous, previous session, old session, related session    |
| None    | anything else                                                                                  |

`None` is the default and **does not mean silent** — the soft-resume
path still runs and can fire on content overlap alone.

## Vector similarity

Pseudocode for `embed_text`:

```rust
let mut vec = [0f32; 256];
for token in text.split(|c| !c.is_alphanumeric()) {
    if token.is_empty() { continue; }
    let bucket = fnv1a_64(token.to_ascii_lowercase()) % 256;
    vec[bucket] += 1.0;
}
l2_normalize(&mut vec);
```

Cosine similarity is then `dot(prompt_vec, transcript_vec)` (both
already L2-normalized).

Properties of this matcher:

- **Tokenization**: alphanumeric runs only. `don't` → `don` + `t`.
- **Stemming / lemmatization**: none. `weather` ≠ `weathers`.
- **Stopword removal**: none. `the`, `a`, `to` all consume bucket mass.
- **Collisions**: 256 buckets means many distinct words share a bucket.
- **Semantics**: zero. Synonyms and paraphrases do not match.

A realistic cosine you'll see on this matcher:

| Prompt vs. transcript                                                 | Cosine     |
| --------------------------------------------------------------------- | ---------- |
| Unrelated topic (`weather` vs. a parser session)                      | ~0.00–0.02 |
| Loose overlap (one shared word in many)                               | ~0.05      |
| Real continuation (`continue parser audit` vs. a parser session)      | ~0.30–0.50 |
| Same session text repeated                                            | 1.00       |

## Decision rules

| Intent       | Behavior                                                                                                                       |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------ |
| Split        | Return `OpenNewAgent` with confidence 90, handoff = the prompt verbatim.                                                       |
| Resume       | Among workspace-related candidates, pick the highest cosine. Return `ResumeExisting` with confidence 85.                       |
| None         | Same picker only after the prompt has at least 3 informative words. If cosine ≥ `SOFT_RESUME_THRESHOLD` (0.10), return `ResumeExisting` with confidence 70. Otherwise silent. |

Constants live in `src/advice.rs` and `src/transcripts.rs` — see the
table below. Tuning these is the easy way to make the matcher more or
less talkative.

## Tunable constants

| Constant                        | File                  | Value | Effect                                       |
| ------------------------------- | --------------------- | ----- | -------------------------------------------- |
| `SUMMARY_TEXT_CAP`              | src/transcripts.rs    | 16384 | Max chars captured per session into summary  |
| `SUMMARY_EMBEDDING_DIM`         | src/advice.rs         | 256   | FNV bucket count                             |
| `SOFT_RESUME_THRESHOLD`         | src/advice.rs         | 0.10  | Min cosine to fire without explicit phrasing |
| `SOFT_RESUME_MIN_INFORMATIVE_WORDS` | src/advice.rs     | 3     | Min non-stopword tokens for implicit resume  |
| `MAX_PARENT_WORKSPACE_DISTANCE` | src/session_index.rs  | 1     | How many dirs up still counts as Parent      |

## Why your "weather" session may not be matching

Check in order:

1. **Word never captured.** Did `weather` literally appear in a
   `user.message.content` or `assistant.message.content[*].text` of that
   session, in the first 16 KB? Tool use / attachments don't count.
   Verify with `tw probe <agent> "weather"` and look at
   `candidate[*].similarity`. A score of `0.00–0.02` means the token
   simply isn't in the captured text.
2. **Capped out.** The mention may live past the 16 KB summary cap.
3. **Wrong workspace.** That session's recorded cwd may be outside the
   current workspace (Different/Unknown). It will not reach the
   candidate list.
4. **Synonyms.** `forecast`, `climate`, `rain` will not match
   `weather`. This is the fundamental limit of bag-of-words.

## How to debug, end to end

```sh
tw status              # workspace, source count, top related summary
tw sessions            # every per-transcript metric, flat text
tw monitor             # last hook payload, response, and top related session
tw top                 # live shell monitor; stop with Ctrl-C
tw explain             # scoring breakdown for the top related session
tw graph --top 10      # mermaid flowchart, paste into mermaid.live
tw probe codex "your exact prompt"
tw probe claude-code "your exact prompt"
tw hook codex-user-prompt-submit --prompt "your exact prompt"
```

`tw probe` is the single most useful debug surface: it shows enablement
state, intent classification, per-candidate similarity, the threshold,
and the final decision with reason.

`tw monitor` is the single most useful runtime surface: it reads
`~/.local/share/threadwise/monitor/hook-events.jsonl` by default and shows
the hook's observed session id, prompt preview, decision, confidence, selected
session, and the current top related transcript.

For a quick manual shell test, use `tw hook ... --prompt "..."`. Running
`tw hook ...` without `--prompt` is the real agent hook path: it waits for the
agent to send JSON on stdin and finishes only after stdin closes.

## What's planned

- **Real semantic embedding** via `fastembed-rs` ONNX (`bge-small` /
  `all-MiniLM-L6-v2`). Swap behind the existing `embed_text` signature;
  no caller changes.
- **Persistent index** in SQLite + LanceDB. Per-session vectors
  precomputed after the session stops; the hook only embeds the new
  prompt.
- **FTS/BM25** for filename/symbol/command matches. Hybrid scoring on
  top of vectors.
- **Chunked summaries** so long sessions don't lose their tail.
- **Tool-use / thinking capture** for richer signal where it carries
  user goal.
