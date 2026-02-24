PREFIX ?= $(HOME)/.local
BINDIR := $(PREFIX)/bin

.PHONY: build build-ghw build-glw install install-ghw install-glw uninstall clean setup

build:
	cargo build --release

build-ghw:
	cargo build --release -p ghw

build-glw:
	cargo build --release -p glw

install: build
	mkdir -p $(BINDIR)
	rm -f $(BINDIR)/ghw $(BINDIR)/glw
	install -m 755 target/release/ghw $(BINDIR)/ghw
	install -m 755 target/release/glw $(BINDIR)/glw

install-ghw: build-ghw
	mkdir -p $(BINDIR)
	rm -f $(BINDIR)/ghw
	install -m 755 target/release/ghw $(BINDIR)/ghw

install-glw: build-glw
	mkdir -p $(BINDIR)
	rm -f $(BINDIR)/glw
	install -m 755 target/release/glw $(BINDIR)/glw

uninstall:
	rm -f $(BINDIR)/ghw $(BINDIR)/glw

clean:
	cargo clean

setup:
	git config core.hooksPath .githooks
