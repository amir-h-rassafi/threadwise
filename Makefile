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
	@echo "  install          build (as invoking user even under sudo) and copy to PREFIX/bin"
	@echo "                   (default /usr/local/bin; 'sudo make install' is the typical form)"
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
	@if [ "$$(id -u)" = "0" ] && [ -n "$$SUDO_USER" ]; then \
	  echo "threadwise: dropping privileges to $$SUDO_USER for cargo build"; \
	  sudo -u "$$SUDO_USER" -i $(MAKE) -C "$(CURDIR)" build-release; \
	else \
	  $(MAKE) build-release; \
	fi
	@expected=$$(grep '^version' Cargo.toml | sed -E 's/.*"(.+)".*/\1/'); \
	 actual=$$(target/release/tw --version 2>/dev/null | awk '{print $$2}'); \
	 if [ "$$expected" != "$$actual" ]; then \
	   echo "error: target/release/tw is stale (built $$actual, Cargo.toml at $$expected)"; \
	   echo "run: cargo clean && make install"; \
	   exit 1; \
	 fi
	$(INSTALL) -d "$(DESTDIR)$(BINDIR)"
	$(INSTALL) -m 0755 target/release/tw "$(DESTDIR)$(BINDIR)/tw"
	@echo "installed $(DESTDIR)$(BINDIR)/tw (version $$(target/release/tw --version | awk '{print $$2}'))"
	@if [ -x "$$HOME/.cargo/bin/tw" ]; then \
	  echo ""; \
	  echo "note: $$HOME/.cargo/bin/tw still exists from a previous 'make install-cargo'."; \
	  echo "      if it appears first on PATH, 'tw --version' will resolve to it instead."; \
	  echo "      remove with: cargo uninstall threadwise"; \
	fi

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
