# Async & Concurrency Patterns

## spawn_monitored (mandatory for background tasks)
- All background tasks MUST use `spawn_monitored(tx, label, future)`
- Double-spawn: outer task catches panics from inner → converts to `AppEvent::Error`
- Label is `&'static str` used in error messages: `"{label} crashed: {msg}"`
- Never use bare `tokio::spawn` for app tasks — panics die silently without this wrapper

## Timeout wrapping
- Every external CLI command: `tokio::time::timeout(NAMED_CONST, Command::new(...).output())`
- Timeout durations as named `const` at module top: `GH_TIMEOUT`, `GLAB_TIMEOUT`, `CLIPBOARD_TIMEOUT`
- Timeout errors include duration: `eyre!("timed out after {}s", CONST.as_secs())`
- Never use raw `Duration` literals inline in timeout calls

## OS thread vs tokio task
- Blocking syscalls (`crossterm::event::poll`) → `std::thread::spawn` (OS thread)
- Blocking library calls (`notify-rust`) → `tokio::task::spawn_blocking`
- Async I/O (CLI commands, network) → tokio task via `spawn_monitored`
- Rule: if it would starve the tokio runtime, use an OS thread or spawn_blocking

## Dynamic config via watch channel
- Polling interval changes via `watch::Sender<u64>` / `watch::Receiver<u64>`
- Main event loop sends new interval; poller receives without restart
- Poller uses `tokio::select!` on sleep + `interval_rx.changed()` to wake early
- Never restart a task just to change a parameter — use a watch channel
