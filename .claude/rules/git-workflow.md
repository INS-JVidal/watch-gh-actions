# Git & Workflow

## Commit messages
- Conventional commits: `feat:`, `fix:`, `docs:`, `refactor:`, `chore:`, `test:`
- Lowercase after colon, no period at end
- Body optional — use for multi-line explanation when needed
- `Co-Authored-By:` trailer when pair-programming with AI

## Never commit
- `.env`, credentials, API tokens
- `target/` directory (in .gitignore)
- Large binary files

## Git hooks
- Pre-commit: auto-increments `BUILD_NUMBER` file and stages it
- Hooks path: `.githooks/` — set up via `make setup`
- After clone, run `make setup` to configure `git config core.hooksPath .githooks`

## Manual verification (before push)
- `cargo fmt --all --check`
- `cargo test --workspace`
- `cargo clippy --workspace`
- `cargo deny check`
- Note: pre-push hook is a no-op; these must be run manually
