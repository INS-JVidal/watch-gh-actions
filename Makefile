PREFIX ?= $(HOME)/.local
BINDIR := $(PREFIX)/bin
BINARY := ghw

.PHONY: build install uninstall clean clean-runs

build:
	cargo build --release

install: build
	mkdir -p $(BINDIR)
	cp target/release/$(BINARY) $(BINDIR)/$(BINARY)

uninstall:
	rm -f $(BINDIR)/$(BINARY)

clean:
	cargo clean

clean-runs:
	@echo "Deleting workflow runs older than 7 days..."
	gh run list --repo $(shell gh repo view --json nameWithOwner -q .nameWithOwner) \
		--limit 100 --json databaseId,createdAt \
		--jq '[.[] | select(.createdAt < (now - 7*24*3600 | strftime("%Y-%m-%dT%H:%M:%SZ")))] | .[].databaseId' \
	| xargs -I{} gh run delete {} 2>/dev/null; true
