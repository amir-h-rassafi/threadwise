# Development

## Toolchain

Rust stable (1.95+). Install via [rustup](https://rustup.rs).

## Make targets

```sh
make build            # cargo build
make test             # cargo test
make fmt              # cargo fmt
make fmt-check        # cargo fmt --check
make lint             # cargo clippy --all-targets -- -D warnings
make check            # fmt-check + lint + test
make ci               # build + check (mirrors GitHub Actions)
make run -- <args>    # cargo run -- <args>
make doctor           # cargo run -q -- doctor
make build-release    # cargo build --release --locked
make install          # install target/release/tw to $(PREFIX)/bin/tw (default /usr/local)
make uninstall        # rm $(PREFIX)/bin/tw
make install-cargo    # alternative: cargo install --path . --locked -> $CARGO_HOME/bin
make uninstall-cargo  # cargo uninstall threadwise
```

`install` is intentionally a **copy-only** step — it does not call cargo. This
keeps `sudo make install` working even though cargo lives in the invoking
user's rustup toolchain, not root's. Standard flow:

```sh
make build-release        # as your user (cargo present)
sudo make install         # as root (copy only)
```

`install` honors standard `PREFIX` (default `/usr/local`) and `DESTDIR`
(default empty) overrides:

```sh
PREFIX=$HOME/.local make build-release install   # user install, no sudo
DESTDIR=/tmp/stage make install                  # stage for packaging
```

## Driving the CLI from source

```sh
cargo run -- doctor
cargo run -- connect codex
cargo run -- source add local ~/.codex/sessions --agent codex
cargo run -- enable codex
cargo run -- status
```

## Release flow

`.github/workflows/release.yml` is the source of truth.

- `workflow_dispatch` runs the test + build matrix without publishing.
- Pushing a `v*` tag additionally runs the `publish` job, which computes
  `SHA256SUMS` over the four tarballs and uploads them to a GitHub release.
- darwin-amd64 is cross-compiled from arm64 macOS; the per-leg smoke test
  (`tw --version`) is skipped for that target only.

After a release: bump `version` in
[`packaging/homebrew/threadwise.rb`](../packaging/homebrew/threadwise.rb) and
replace the four `REPLACE_WITH_*_SHA256` placeholders from `SHA256SUMS`.

## Pointers

- [architecture.md](architecture.md) — module boundaries and data flow.
- [mvp-plan.md](mvp-plan.md) — milestones M0–M6.
- [benchmark-plan.md](benchmark-plan.md) — evaluation harness scoping.
