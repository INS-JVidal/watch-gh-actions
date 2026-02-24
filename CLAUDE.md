# CI Watcher Workspace

Terminal UIs for monitoring CI/CD pipelines. Ships as two binaries sharing a common library:
- **ghw** — GitHub Actions watcher (wraps `gh` CLI)
- **glw** — GitLab CI watcher (wraps `glab` CLI)
- **ciw-core** — shared library: data model, event loop, TUI rendering, polling, change detection

## Workspace Map

```
crates/ciw-core/src/
  traits.rs          CiExecutor + CiParser — the platform abstraction boundary
  app.rs             AppState, WorkflowRun, Job, Step, tree model, overlays (~1670 lines)
  events.rs          AppEvent enum + EventHandler (terminal input thread → mpsc channel)
  input.rs           KeyEvent → Action mapping, context-dependent on active overlay
  diff.rs            Change detection: compare poll snapshots → generate notifications
  poller.rs          Background polling loop with adaptive interval + exponential backoff
  notify.rs          Desktop notifications via notify-rust (feature-gated: desktop-notify)
  platform.rs        PlatformConfig: static display strings and ASCII art per platform
  tui/render.rs      Top-level layout: header + tree + footer + overlay dispatch
  tui/tree.rs        Scrollable run/job/step tree with icons, scroll, truncation
  tui/startup.rs     Animated multi-phase startup screen
  tui/log_overlay.rs       Scrollable failed-log viewer
  tui/detail_overlay.rs    Key-value metadata panel
  tui/confirm_overlay.rs   Yes/no confirmation dialog
  tui/header.rs, footer.rs, spinner.rs — small rendering helpers

crates/ghw/src/
  executor.rs   GhExecutor → calls `gh run list`, `gh run view`, etc.
  parser.rs     GhParser → direct serde (GitHub JSON matches core types)
  cli.rs        --repo, --branch, --interval, --limit, --workflow
  main.rs       Entry point + event loop

crates/glw/src/
  executor.rs   GlabExecutor → calls `glab api /projects/...` (REST passthrough)
  parser.rs     GlabParser → intermediate GlabPipeline/GlabJob types + status mapping
  cli.rs        --project, --branch, --interval, --limit, --source
  main.rs       Entry point + event loop (mirrors ghw structure)
```

## Architecture Decisions

**Single-threaded event loop** — All state mutations happen in `run_app()` on the main thread. No locks, no shared mutable state. Background tasks communicate exclusively through an `mpsc::UnboundedChannel<AppEvent>`. Unbounded because backpressure isn't meaningful for UI events — if we can't keep up, we have bigger problems.

**OS thread for terminal input** — `EventHandler` spawns a dedicated `std::thread`, not a tokio task, because `crossterm::event::poll()` is a blocking syscall that would starve the async runtime.

**Flat tree model** — `AppState::tree_items` is a `Vec<TreeItem>` rebuilt from scratch on every state change. Each item stores indices into the `runs` vector, not cloned data. Simpler than a recursive tree: O(1) index access, trivial scroll math, no borrow-checker gymnastics. Rebuild is fast enough (~20 runs × ~10 jobs = negligible).

**Trait-based platform abstraction** — `CiExecutor` and `CiParser` in `traits.rs`. The library only sees `Arc<dyn CiExecutor>` / `Arc<dyn CiParser>`. Executor returns raw JSON strings to keep parsing in the parser (easier to add size validation, test independently).

**`spawn_monitored` double-spawn** — Outer tokio task catches panics from the inner task and converts them to `AppEvent::Error`. Without this, a panicking background task (poller, log fetch) dies silently.

**Adaptive polling** — Interval adjusts based on activity: 3s active → 10s recent → 30s idle. The poller receives interval changes via `watch::Receiver` so the main loop can adjust speed without restarting the polling task.

## Build & Test

```bash
cargo build --release              # Both binaries
cargo test --workspace             # All tests (unit + integration)
cargo test -p ghw -- --ignored     # Live gh CLI tests (requires auth)
cargo clippy --workspace           # Lint
cargo doc --no-deps --workspace    # Generate docs
make install                       # Release build + install to ~/.local/bin
```

## Key Conventions

- **Overlay exclusivity**: Only one overlay active at a time (`ActiveOverlay` enum, not a stack)
- **Error channels**: `AppEvent::RunError` = per-run ⚠ icon (persists), `AppEvent::Error` = global toast (auto-dismisses after 10s)
- **Log cache**: Failed-log content cached for 120s to avoid re-fetching when reopening
- **`WorkflowRun.jobs: Option<Vec<Job>>`**: `None` = not fetched (show Loading), `Some(vec)` = fetched. This distinction drives the expand UI.
- **Change detection**: Snapshot-based with merge semantics — entries persist after disappearing from API response to prevent false notifications when runs scroll out of `--limit` window
