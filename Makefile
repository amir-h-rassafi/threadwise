PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin
DESTDIR ?=
INSTALL ?= install

.DEFAULT_GOAL := help

.PHONY: help build build-release test check fmt fmt-check lint clippy run doctor clean ci \
        install uninstall install-cargo uninstall-cargo

help:
	@echo "Threadwise — make targets"
	@echo ""
	@echo "  build            cargo build (debug)"
	@echo "  build-release    cargo build --release --locked"
	@echo "  test             cargo test"
	@echo "  fmt              cargo fmt"
	@echo "  fmt-check        cargo fmt --check"
	@echo "  lint / clippy    cargo clippy --all-targets -- -D warnings"
	@echo "  check            fmt-check + lint + test"
	@echo "  ci               build + check (mirrors GitHub Actions)"
	@echo ""
	@echo "  install          copy target/release/tw to PREFIX/bin (default /usr/local/bin)"
	@echo "                   run 'make build-release' first; use 'sudo make install' for /usr/local"
	@echo "  uninstall        rm PREFIX/bin/tw"
	@echo "  install-cargo    cargo install --path . --locked -> CARGO_HOME/bin"
	@echo "  uninstall-cargo  cargo uninstall threadwise"
	@echo ""
	@echo "  run -- <args>    cargo run -- <args>"
	@echo "  doctor           tw doctor"
	@echo "  clean            cargo clean"
	@echo "  help             show this message"
	@echo ""
	@echo "Overrides: PREFIX=/usr/local  DESTDIR=  INSTALL=install"
	@echo ""
	@echo "Prebuilt binaries (skip the build entirely):"
	@echo "  curl -sSfL https://raw.githubusercontent.com/amir-h-rassafi/threadwise/main/scripts/install.sh | sh"

build:
	cargo build

build-release:
	cargo build --release --locked

install:
	@if [ ! -x target/release/tw ]; then \
	  echo "error: target/release/tw not found."; \
	  echo "build first as your user: make build-release"; \
	  exit 1; \
	fi
	$(INSTALL) -d "$(DESTDIR)$(BINDIR)"
	$(INSTALL) -m 0755 target/release/tw "$(DESTDIR)$(BINDIR)/tw"
	@echo "installed $(DESTDIR)$(BINDIR)/tw"

uninstall:
	rm -f "$(DESTDIR)$(BINDIR)/tw"
	@echo "removed $(DESTDIR)$(BINDIR)/tw"

install-cargo:
	cargo install --path . --locked

uninstall-cargo:
	cargo uninstall threadwise

test:
	cargo test

fmt:
	cargo fmt

fmt-check:
	cargo fmt --check

lint clippy:
	cargo clippy --all-targets -- -D warnings

check: fmt-check lint test

run:
	cargo run --

doctor:
	cargo run -q -- doctor

clean:
	cargo clean

ci: build check
