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
make install          # cargo install --path . --locked
make uninstall        # cargo uninstall threadwise
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
