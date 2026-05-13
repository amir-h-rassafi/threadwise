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
  cosine similarity over captured message text.
- **Open new agent** — emits a copy-ready handoff prompt for a fresh focused
  agent when the prompt explicitly asks to split.

Adapters today: **Codex** (`~/.codex/sessions/`) and **Claude Code**
(`~/.claude/projects/`). The adapter boundary is the agent-specific wiring
layer; the core ranking is agent-neutral.

## Installation

### One-liner (recommended)

```sh
curl -sSfL https://raw.githubusercontent.com/amir-h-rassafi/threadwise/main/scripts/install.sh | sh
```

Detects your OS/arch, downloads the matching prebuilt tarball from the latest
release, and installs `tw` to `/usr/local/bin`. No toolchain required.

User install (no sudo):

```sh
curl -sSfL https://raw.githubusercontent.com/amir-h-rassafi/threadwise/main/scripts/install.sh | PREFIX=$HOME/.local sh
```

Pin a version:

```sh
curl -sSfL https://raw.githubusercontent.com/amir-h-rassafi/threadwise/main/scripts/install.sh | TW_VERSION=v0.1.1 sh
```

### From source

```sh
git clone https://github.com/amir-h-rassafi/threadwise
cd threadwise
make build-release     # cargo build, run as your user
sudo make install      # copies target/release/tw to /usr/local/bin/tw
tw --version
```

The build and install steps are split because `cargo` lives in your user's
rustup toolchain but `/usr/local/bin` needs root — `sudo make install` is
copy-only. **After `git pull`, re-run `make build-release` before
`sudo make install`** or you'll re-install the old binary.

Overrides:

```sh
PREFIX=$HOME/.local make build-release install   # user install, no sudo
DESTDIR=/tmp/stage make install                  # stage for packaging
make install-cargo                               # cargo install --path . --locked
make uninstall                                   # rm $(PREFIX)/bin/tw
make help                                        # list all targets
```

### Homebrew

A formula template lives at
[`packaging/homebrew/threadwise.rb`](packaging/homebrew/threadwise.rb); a tap
will follow once we settle on one.

## Quickstart

```sh
tw doctor                                              # check setup
tw connect codex                                       # detect Codex install
tw connect claude-code                                 # detect Claude Code install
tw enable codex                                        # opt in to advice for codex
tw enable claude-code                                  # opt in to advice for claude-code
tw init codex                                          # print Codex hook config (config.toml)
tw init claude-code                                    # print Claude Code hook config (settings.json)
tw status                                              # current + related sessions
tw monitor                                             # last hook + response + top related session
tw top                                                 # live terminal monitor, like top
tw hook codex-user-prompt-submit --prompt "manual test" # shell test for monitor
```

`tw connect <agent>` registers the default source path for you
(`~/.codex/sessions` or `~/.claude/projects`). For a non-default location use
`tw source add local <path> --agent <kind>`. Paste the hook snippet from
`tw init <agent>` into the agent's config. Hooks stay silent unless confidence
is high and hard-time-out under 2 s. Hook decisions are recorded locally under
`~/.local/share/threadwise/monitor/` by default so `tw monitor` can show what
the running session actually sent and how Threadwise answered.

## Status

Working today: Codex + Claude Code adapters, opt-in enablement, transcript
discovery + parsing, workspace-relation + cosine-similarity ranking,
`UserPromptSubmit` hooks that suggest *resume* or *new agent* on explicit
phrasing and stay silent otherwise, and `tw monitor` / `tw top` for hook
observability. 16 unit + 7 e2e tests.

Not yet: persistent SQLite store, `fastembed-rs` embeddings, LanceDB,
`tw handoff` command body.

Roadmap: [`docs/mvp-plan.md`](docs/mvp-plan.md). Module layout and data flow:
[`docs/architecture.md`](docs/architecture.md). How matching actually
works (vector shape, thresholds, why a topic might miss):
[`docs/matching.md`](docs/matching.md). Evaluation scoping:
[`docs/benchmark-plan.md`](docs/benchmark-plan.md).

## Development

See [`docs/development.md`](docs/development.md) for toolchain, make targets,
and release flow.

## License

Dual-licensed under MIT or Apache-2.0, at your option.
