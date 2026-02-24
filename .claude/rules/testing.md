# Testing Conventions

## Structure
- Unit tests: `#[cfg(test)] mod tests { ... }` at the bottom of each source file
- Integration tests: `crates/{crate}/tests/` directory
- Live CLI tests: `#[tokio::test] #[ignore]` — run with `cargo test -p ghw -- --ignored`

## Fixtures
- Inline const JSON: `const SINGLE_RUN_JSON: &str = r#"[{...}]"#;`
- Prefer inline over file-based — keeps tests self-contained
- Compact JSON for single-field tests, full JSON for integration tests

## Helper constructors
- `make_run(id, status, conclusion) -> WorkflowRun` — minimal valid run
- `make_state() -> AppState` — default empty state (does NOT call rebuild_tree)
- `make_state_with_runs(runs) -> AppState` — sets runs + calls rebuild_tree()
- Keep helpers in local `mod tests` (unit) or `tests/fixtures.rs` (integration)

## Assertions
- `pretty_assertions` in dev-deps for readable struct diffs
- `assert_eq!` with context: `assert_eq!(status, expected, "for status string: {s}")`
- Expected failures: `assert!(result.is_err())` then `.to_string().contains(...)`

## Test naming
- `snake_case`, pattern: `{action}_{condition}_{outcome}`
- Examples: `backoff_capped_at_max`, `classify_not_logged_in`, `first_poll_no_notifications`
- No `test_` prefix — `#[test]` already marks them

## Verification
- See `git-workflow.md` for build verification commands
