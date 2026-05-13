# Threadwise Benchmark & Evaluation Plan

Status: **scoping**. No harness exists yet. This document fixes what we measure
and the shape of the pipeline before we build any of it. Implementation lands
incrementally after the MVP plan's M3 (summaries + vector search) is wired
end-to-end.

## Why a separate plan

The advisor's value is judged on two axes the unit tests cannot capture:

1. Did it pick the right action on a realistic session graph?
2. Did it stay silent when it should have?

False positives are worse than misses — noisy advice gets turned off. We need
a measurable definition of "right" and "quiet enough" before tuning thresholds
or swapping the embedding backend.

## Metrics

### Match quality

| Metric                  | Definition                                                    | Target  |
| ----------------------- | ------------------------------------------------------------- | ------- |
| Recall@1 (resume)       | Ground truth says resume X → top recommendation matches X.    | ≥ 0.80  |
| Precision (resume)      | Fraction of `ResumeExisting` outputs a human would accept.    | ≥ 0.85  |
| Precision (new agent)   | Same, for `OpenNewAgent` handoffs.                            | ≥ 0.90  |
| Silence rate (normal)   | Fraction of unrelated prompts that produce no output.         | ≥ 0.99  |
| Workspace specificity   | Fraction of recommendations whose session is in this repo.    | ≥ 0.95  |

### Latency (hook path)

| Metric              | Target                |
| ------------------- | --------------------- |
| p50 hook wall time  | < 100 ms              |
| p95 hook wall time  | < 500 ms (MVP budget) |
| p99 hook wall time  | < 2000 ms (hard cap)  |

Measured on a populated workspace: 50–200 transcripts, 10–50 MB each.

### Robustness

- No crash, exit code 0, no stderr on: malformed JSON, partial JSONL writes,
  missing transcript directory, unreadable file, transcripts written by a
  newer codex version.
- Disabled scopes stay silent across all of the above.

## Pipeline shape

```
fixtures/                         harness                         report
─────────                         ───────                         ──────
transcript bundles  ──┐
ground-truth labels ──┼─► load → drive recommend_for_hook ──► metrics.json
synthetic prompts   ──┘                                      ──► latency.json
                                                             ──► silence.json
```

1. **Fixtures** in `tests/fixtures/<scenario>/` — each bundle is a directory of
   `*.jsonl` transcripts plus a `ground_truth.toml` describing per-prompt
   expectations.
2. **Harness** — a `cargo test --test eval` (or `benches/`) entrypoint that
   loads each bundle, synthesizes `HookEvent`s, drives `recommend_for_hook`,
   and writes structured results.
3. **Reports** — JSON files committed under `target/eval/` (gitignored), with
   a Markdown summary printed by the harness.
4. **CI gate** — PR job runs the quality and silence sets; nightly runs the
   latency set on the populated workspace fixture.

## Fixture catalogue (initial 10–20 bundles)

- `same_repo_resume` — clear continuation of a prior in-repo session.
- `nested_workspace_resume` — session was in a child crate.
- `parent_workspace_resume` — session was at the monorepo root.
- `unrelated_repo` — top related session is in a different repo; should not
  surface.
- `stale_session` — top related session is from months ago; should be
  ranked below recent ones.
- `partial_write` — last JSONL line truncated mid-record.
- `malformed_record` — one record fails JSON parse, others fine.
- `explicit_split` — prompt asks for a new agent; expect `OpenNewAgent`.
- `explicit_resume` — prompt asks to resume; expect `ResumeExisting` with the
  matching session.
- `normal_prompt` — neither split nor resume phrasing; expect silence.
- `multi_match` — several plausible resume candidates; the harness verifies
  ranking by summary similarity.
- `disabled_scope` — source registered but disabled; expect silence regardless
  of prompt.
- `large_workspace` — 200 transcripts × 20 MB each; latency bundle.

## Phases

### Phase 0 — Scoping

This document. Lock metrics, fixture format, and pipeline shape.

### Phase 1 — Fixtures

Add `tests/fixtures/` with the catalogue above. Transcripts are synthesized,
not copied from real sessions; treat anything plausibly private as poison.

### Phase 2 — Quality harness

`tests/eval.rs` (separate test target) loads fixtures, drives the advisor,
emits `target/eval/quality.json`, prints a table. Wired into `make check` once
stable.

### Phase 3 — Latency harness

`benches/hook_latency.rs` via [criterion](https://github.com/bheisler/criterion.rs).
Runs against the `large_workspace` fixture. Tracks p50/p95/p99 over time.

### Phase 4 — CI gate

`make eval` runs quality + silence. PR fails if any metric regresses outside a
tolerance band. Nightly workflow tracks latency trends.

## Open questions

- **Embedding swap.** When we replace the FNV bag-of-words `embed_text` with
  `fastembed-rs`, what's the right tolerance band on quality regressions
  before the new model is rejected?
- **Fixture provenance.** Synthesize, or anonymize real transcripts? Lean
  synthesize for the first pass.
- **Latency fixture realism.** 200 × 20 MB is a stretch; real users likely
  have fewer, smaller transcripts. Calibrate after first telemetry.
- **Hook output diffing.** Compare stdout textually or parse a structured
  advisor record? Textual is simpler; structured is sturdier under wording
  changes.
