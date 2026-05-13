# Threadwise

[![Release](https://github.com/amir-h-rassafi/threadwise/actions/workflows/release.yml/badge.svg)](https://github.com/amir-h-rassafi/threadwise/actions/workflows/release.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org)

**Local session advisor for CLI coding agents.** Threadwise (CLI: `tw`) reads
your local agent transcripts and tells you — before you hit enter on the next
prompt — whether to stay in the current session, resume a related one, or fork
a focused new agent with a handoff. Runs offline, opt-in, silent unless it has
something concrete to say.

## Why

Long agent sessions accumulate unrelated context, drift across repos, and end
up reloading the same problem in fresh windows. Nothing watches that drift.
You only notice when the agent has lost the plot or you've burned tokens
rebuilding context you already had two sessions ago.

Threadwise is a second pair of eyes on session hygiene. It does not wrap or
replace the agent — it sits next to it, watches transcripts on disk, and emits
one short suggestion via a hook when, and only when, it's confident.

## What it does

Three answers, picked from local evidence:

- **Continue current session** — quiet, no output.
- **Resume related session** — points at the prior session whose summary
  best matches what you're about to ask, ranked by workspace relation +
  cosine similarity.
- **Open new agent** — emits a copy-ready handoff prompt for a fresh focused
  agent when the prompt explicitly asks to split.

Codex first. The adapter layer is built to accept more agents once Codex is
solid.

## Installation

### From source (works today)

```sh
git clone https://github.com/amir-h-rassafi/threadwise
cd threadwise
make install        # installs to $CARGO_HOME/bin (usually ~/.cargo/bin)
tw --version
```

`make install` is a thin wrapper around `cargo install --path . --locked`.
Make sure `~/.cargo/bin` is on your `PATH`.

### Prebuilt binaries

Tagged releases publish tarballs for `darwin-arm64`, `darwin-amd64`,
`linux-amd64`, `linux-arm64` alongside a `SHA256SUMS` file. Grab from the
[releases page](https://github.com/amir-h-rassafi/threadwise/releases), verify
the checksum, and drop `tw` somewhere on your `PATH`.

```sh
curl -L -o tw.tar.gz https://github.com/amir-h-rassafi/threadwise/releases/latest/download/threadwise-darwin-arm64.tar.gz
tar -xzf tw.tar.gz
sudo mv threadwise-darwin-arm64/tw /usr/local/bin/
```

### Homebrew

A formula template lives at
[`packaging/homebrew/threadwise.rb`](packaging/homebrew/threadwise.rb) and is
published to a tap with each release.

## Quickstart

```sh
tw doctor                                              # check setup
tw connect codex                                       # detect Codex install
tw source add local ~/.codex/sessions --agent codex    # register transcripts
tw enable codex                                        # opt in to advice
tw init codex                                          # print hook config
tw status                                              # current + related sessions
```

Paste the hook lines from `tw init codex` into your Codex config. The hook
runs on each prompt submit, stays silent unless confidence is high, and hard-
times-out under 2 s.

## Status

MVP in progress. Roadmap and milestones:
[`docs/mvp-plan.md`](docs/mvp-plan.md). Module layout and data flow:
[`docs/architecture.md`](docs/architecture.md). Evaluation scoping:
[`docs/benchmark-plan.md`](docs/benchmark-plan.md).

## Development

See [`docs/development.md`](docs/development.md) for toolchain, make targets,
and release flow.

## License

Dual-licensed under MIT or Apache-2.0, at your option.
