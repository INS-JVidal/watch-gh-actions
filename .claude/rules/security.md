# Security & Safety

## URL validation
- Before `open_in_browser()`: must start with `https://` or `http://`
- Reject non-HTTP schemes: `eyre!("Refusing to open non-HTTP URL: {url}")`
- Windows `cmd /C start`: always pass empty title `""` as first arg (prevents shell injection)

## External command safety
- Use `Command::new(tool).args([...])` array form — never string concatenation
- Fire-and-forget child processes: set stdin/stdout/stderr to `Stdio::null()`
- Never interpolate user input into shell strings

## Size limits (check BEFORE parsing/displaying)
- See `error-handling.md` for size guard functions and limits
- `LOG_MAX_LINES = 500` — tail truncation in TUI prevents OOM on render

## Supply chain (deny.toml)
- Only crates.io registry allowed (`unknown-registry = "deny"`)
- No git dependencies (`unknown-git = "deny"`)
- License allowlist: MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-*, Zlib
- CI runs `cargo deny check` on every push

## Secrets
- Never commit: `.env`, API tokens, credentials
- Auth handled by user's `gh`/`glab` CLI tool — this app stores no credentials
