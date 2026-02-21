PREFIX ?= $(HOME)/.local
BINDIR := $(PREFIX)/bin
BINARY := ghw

.PHONY: build install uninstall clean setup

build:
	cargo build --release

install: build
	mkdir -p $(BINDIR)
	cp target/release/$(BINARY) $(BINDIR)/$(BINARY)

uninstall:
	rm -f $(BINDIR)/$(BINARY)

clean:
	cargo clean

setup:
	git config core.hooksPath .githooks
