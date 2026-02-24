# Code Organization

## Where new code goes
- New platform trait methods → `ciw-core/src/traits.rs`
- New data model types/enums → `ciw-core/src/app.rs`
- New TUI widgets → `ciw-core/src/tui/{name}.rs` + register in `tui/mod.rs`
- New GitHub-specific logic → `crates/ghw/src/executor.rs` or `parser.rs`
- New GitLab-specific logic → `crates/glw/src/executor.rs` or `parser.rs`
- New event types → `ciw-core/src/events.rs` (AppEvent enum)
- New key mappings → `ciw-core/src/input.rs`

## Binary crate structure (ghw and glw mirror each other)
- `main.rs` — entry point, event loop, spawn_monitored
- `executor.rs` — platform CLI wrapper (implements CiExecutor)
- `parser.rs` — JSON deserialization (implements CiParser)
- `cli.rs` — clap CLI argument definitions

## Naming
- Files: `snake_case` (e.g., `log_overlay.rs`, `confirm_overlay.rs`)
- TUI components: `tui/{widget_name}.rs`
- One module per file; `mod.rs` only for `tui/` directory

## Clippy configuration
- `#![warn(clippy::pedantic)]` + shared allow list in `lib.rs` (ciw-core, ghw have one; glw does not)
- Canonical allow list: see `ciw-core/src/lib.rs`
- New crates with a `lib.rs` should copy the same allow block

## Dependencies
- Shared deps in ciw-core Cargo.toml; binary-only deps in binary Cargo.toml
- Both binaries depend on ciw-core via `path = "../ciw-core"`
