# Error Handling

## Result type
- Use `color_eyre::eyre::Result` everywhere — never `anyhow` or `Box<dyn Error>`
- Create errors with `eyre!("context: {detail}")` — always include actionable context

## Error classification
- Each executor has a `classify_*_error(stderr)` function (`classify_gh_error`, `classify_glab_error`)
- Maps raw stderr to user-friendly messages for TUI display
- Pattern: check for known substrings ("not logged", "not a git repository") → user-friendly message → fallback to generic

## Two error channels (when to use which)
- `AppEvent::Error(String)` — global failures (poll, CLI not found, timeout)
- `AppEvent::RunError { run_id, error }` — per-run failures (job fetch, log fetch)

## Size guards
- `check_response_size(json)` before any `serde_json::from_str()` — 10MB limit
- `check_log_size(log)` before returning log content — 10MB limit
- Both return `Err(eyre!("...too large..."))` with human-readable MB values

## Error message format
- Include the failing tool: `"gh command failed: {stderr}"`
- Include actionable hints: `"Not authenticated with gh. Run 'gh auth login' first."`
- Include duration on timeouts: `"timed out after 30s"`
