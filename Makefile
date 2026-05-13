PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin
DESTDIR ?=
INSTALL ?= install

.PHONY: build test check fmt fmt-check lint clippy run doctor clean ci \
        install uninstall install-cargo uninstall-cargo

build:
	cargo build

install:
	cargo build --release --locked
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
