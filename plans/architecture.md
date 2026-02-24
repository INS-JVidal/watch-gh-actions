# Architecture Overview

> watch-gh-actions workspace — `ghw` (GitHub Actions) and `glw` (GitLab CI)

```
watch-gh-actions/
├── Cargo.toml              ← workspace root (resolver v2)
├── crates/
│   ├── ciw-core/           ← shared library (no binary)
│   ├── ghw/                ← GitHub Actions watcher binary
│   └── glw/                ← GitLab CI watcher binary
├── Makefile
├── plans/
└── .github/workflows/
    ├── ci.yml
    └── release.yml
```

All three crates are versioned together at `0.8.0`.

---

## 1. Crate Dependency Graph

```
┌─────────┐      ┌─────────┐
│   ghw   │      │   glw   │        ← binary crates
└────┬────┘      └────┬────┘
     │                │
     └───────┬────────┘
             ▼
       ┌──────────┐
       │ ciw-core  │                 ← shared library
       └──────────┘
```

Both `ghw` and `glw` depend on `ciw-core` via path. They share the
same set of external dependencies (`ratatui`, `crossterm`, `tokio`,
`clap`, `serde`, `chrono`, `color-eyre`, `async-trait`, etc.).

---

## 2. ciw-core — The Shared Library

`ciw-core` contains **all platform-independent logic**. Neither binary
duplicates any of this code.

### 2.1 Trait Abstractions (`traits.rs`)

Two traits define the contract between the shared library and each
platform-specific binary:

```
┌──────────────────────────────────────────────────┐
│  CiExecutor  (async, Send + Sync, async_trait)   │
├──────────────────────────────────────────────────┤
│  check_available()       → Result<()>            │
│  detect_repo()           → Result<String>        │
│  detect_branch()         → Result<String>        │
│  fetch_runs(limit, filter) → Result<String>      │
│  fetch_jobs(run_id)      → Result<String>        │
│  cancel_run(run_id)      → Result<()>            │
│  delete_run(run_id)      → Result<()>            │
│  rerun_failed(run_id)    → Result<()>            │
│  fetch_failed_logs(...)  → Result<String>        │
│  open_in_browser(url)    → Result<()>            │
│  copy_to_clipboard(text) → Result<()>            │
└──────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────┐
│  CiParser  (sync, Send + Sync)                   │
├──────────────────────────────────────────────────┤
│  parse_runs(json)        → Result<Vec<Run>>      │
│  parse_jobs(json)        → Result<Vec<Job>>      │
│  process_log_output(raw) → (String, bool)        │
└──────────────────────────────────────────────────┘
```

The library only ever sees `Arc<dyn CiExecutor>` and `Arc<dyn CiParser>`.
It never imports `GhExecutor`, `GlabExecutor`, or any concrete type.

### 2.2 Data Models (`app.rs`)

All domain types live in `ciw-core`, shared by both binaries:

| Type | Purpose |
|------|---------|
| `WorkflowRun` | A CI run/pipeline: id, title, branch, status, conclusion, timestamps, URL, optional jobs |
| `Job` | A job within a run: id, name, status, timestamps, steps |
| `Step` | A step within a job: name, number, status, timestamps |
| `RunStatus` | `Completed \| InProgress \| Queued \| Requested \| Waiting \| Pending \| Unknown` |
| `Conclusion` | `Success \| Failure \| Cancelled \| Skipped \| TimedOut \| ActionRequired \| StartupFailure \| Stale \| Neutral \| Unknown` |
| `FilterMode` | `All \| ActiveOnly \| CurrentBranch` |
| `TreeItem` | One visible row in the TUI tree (run, job, step, or loading indicator) |
| `AppConfig` | Repo, branch, limit, workflow filter, version string |
| `AppState` | The entire mutable application state (see below) |

### 2.3 Application State (`AppState`)

`AppState` is the single source of truth for the application. Key groups:

- **Run data**: `runs`, `previous_snapshot` (for diff detection), `poll_count`
- **Tree navigation**: `tree_items`, `cursor`, `expanded_runs`, `expanded_jobs`, `filter`
- **Polling**: `last_poll`, `next_poll_in`, `poll_interval`
- **UI state**: `notifications`, `error`, `spinner_frame`, `loading_count`, `overlay`
- **Caches**: `log_cache`, `run_errors`
- **Config**: `desktop_notify`, `should_quit`

Methods cover all user interactions: cursor movement, expand/collapse,
filtering, overlay management, run deletion, cache pruning, etc.

### 2.4 Module Map

```
ciw-core/src/
├── lib.rs          Re-exports all modules
├── traits.rs       CiExecutor + CiParser trait definitions
├── app.rs          Data models, AppState, all state mutations (~1670 lines)
├── platform.rs     PlatformConfig struct (name, ascii art, install hints)
├── poller.rs       Background polling loop + exponential backoff
├── diff.rs         Change detection: old snapshot vs new poll → notifications
├── events.rs       AppEvent enum + EventHandler (crossterm → channel bridge)
├── input.rs        Stateless key → Action mapping
├── notify.rs       Desktop notifications (feature-gated on notify-rust)
└── tui/
    ├── mod.rs              Re-exports
    ├── render.rs           Top-level layout: header + tree + footer + overlays
    ├── header.rs           Version, repo, branch, filter, spinner/countdown
    ├── footer.rs           Context-sensitive keybinding hints
    ├── tree.rs             Scrollable run/job/step tree with icons and colors
    ├── spinner.rs          10-frame braille spinner
    ├── startup.rs          Animated startup sequence with phase tracking
    ├── log_overlay.rs      Scrollable log viewer modal
    ├── detail_overlay.rs   Key-value detail modal
    └── confirm_overlay.rs  Yes/no confirmation dialog
```

### 2.5 Event Flow

```
┌──────────────┐     ┌──────────────┐
│  crossterm   │     │   Poller     │
│  (OS thread) │     │ (tokio task) │
└──────┬───────┘     └──────┬───────┘
       │ Key/Tick           │ PollResult/JobsResult
       └──────────┬─────────┘
                  ▼
        mpsc::UnboundedChannel
                  │
                  ▼
          ┌───────────────┐
          │   run_app()   │  ← main event loop
          │   match event │
          └───────┬───────┘
                  │
          ┌───────┴───────┐
          ▼               ▼
     AppState          terminal
     mutations         .draw()
```

- `EventHandler` runs on a dedicated OS thread, polling `crossterm` events
  at 100ms intervals. Key presses become `AppEvent::Key`, timeouts become
  `AppEvent::Tick`.
- `Poller` runs as a tokio task, fetching runs at adaptive intervals
  (3s active / 10s recent / 30s idle). It receives interval updates via
  a `tokio::sync::watch` channel.
- The event loop in `run_app()` is the single consumer. It reads events,
  mutates `AppState`, and redraws the terminal.

### 2.6 Adaptive Polling

The poll interval adjusts based on run activity:

| Condition | Interval | Constant |
|-----------|----------|----------|
| Any run is `InProgress`, `Queued`, `Waiting`, or `Pending` | 3s | `POLL_INTERVAL_ACTIVE` |
| Most recent run updated within last 60s | 10s | `POLL_INTERVAL_RECENT` |
| All runs idle | 30s | `POLL_INTERVAL_IDLE` |

On consecutive failures, `backoff_delay(base, failures)` applies
exponential backoff: `min(base × 2^failures, 300s)`.

### 2.7 Change Detection (`diff.rs`)

Each poll, `detect_changes()` compares the new run list against a
`HashMap<u64, SnapshotEntry>` of previous `(status, conclusion)` pairs:

- First appearance → no notification (initial load)
- Status/conclusion changed → generate human-readable `Notification`
- Entries not seen in last 10 polls → evicted from snapshot

### 2.8 Platform Config (`platform.rs`)

A small struct that lets each binary inject platform identity strings
without any conditional logic in the library:

```rust
pub struct PlatformConfig {
    pub name: &'static str,         // "workflow" / "pipeline"
    pub full_name: &'static str,    // "GitHub Actions" / "GitLab CI"
    pub cli_tool: &'static str,     // "GitHub" / "GitLab"
    pub install_hint: &'static str,
    pub ascii_art: &'static [&'static str],
}
```

Used by the startup screen for the ASCII art banner and phase messages.

---

## 3. ghw — GitHub Actions Watcher

### 3.1 Structure

```
ghw/src/
├── main.rs       Entry point, event loop, terminal setup
├── lib.rs        Module declarations + re-exports for tests
├── cli.rs        Clap CLI: --repo, --branch, --interval, --limit, --workflow
├── executor.rs   GhExecutor: wraps `gh` CLI commands
├── parser.rs     GhParser: direct serde deserialization
└── build.rs      Reads BUILD_NUMBER for version string
```

### 3.2 GhExecutor

Wraps the GitHub CLI (`gh`). Each `CiExecutor` method maps to a
`gh` subcommand:

| Trait method | `gh` command |
|-------------|-------------|
| `check_available` | `gh auth status` |
| `detect_repo` | `gh repo view --json nameWithOwner` |
| `detect_branch` | `git rev-parse --abbrev-ref HEAD` |
| `fetch_runs` | `gh run list --json <fields> [--workflow w]` |
| `fetch_jobs` | `gh run view {id} --json jobs` |
| `cancel_run` | `gh run cancel {id}` |
| `delete_run` | `gh run delete {id}` |
| `rerun_failed` | `gh run rerun --failed {id}` |
| `fetch_failed_logs` | `gh run view {id} --log-failed` |

All commands go through `run_gh()`, which applies a 30-second timeout
and classifies stderr into user-friendly error messages via
`classify_gh_error()`.

### 3.3 GhParser

Parsing is straightforward because `gh --json` output uses the same
camelCase field names as the `WorkflowRun` and `Job` serde attributes:

```rust
parse_runs(json)  →  serde_json::from_str::<Vec<WorkflowRun>>(json)
parse_jobs(json)  →  serde_json::from_str::<JobsResponse>(json).jobs
```

No intermediate types or status mapping needed.

### 3.4 CLI Arguments

```
ghw [OPTIONS]
  --repo, -r       owner/repo (auto-detected if omitted)
  --branch, -b     filter by branch
  --interval, -i   poll interval in seconds (default: 10, min: 1)
  --limit, -l      max runs to fetch (default: 20)
  --workflow, -w   filter by workflow name
  --no-notify      disable desktop notifications
  --verbose        enable debug logging
```

`validate_repo_format()` enforces exactly one `/` with no empty segments.

---

## 4. glw — GitLab CI Watcher

### 4.1 Structure

```
glw/src/
├── main.rs       Entry point, event loop, terminal setup
├── cli.rs        Clap CLI: --project, --branch, --interval, --limit, --source
├── executor.rs   GlabExecutor: wraps `glab api` REST calls
├── parser.rs     GlabParser: intermediate types + status mapping
└── build.rs      Reads BUILD_NUMBER for version string
```

### 4.2 GlabExecutor

Wraps the GitLab CLI (`glab`), using its `api` subcommand for REST
passthrough. Stores both the raw `project` path and an `encoded_project`
with slashes URL-encoded as `%2F` (e.g. `group/sub/proj` → `group%2Fsub%2Fproj`).

| Trait method | `glab` command / API endpoint |
|-------------|------------------------------|
| `check_available` | `glab auth status` |
| `detect_repo` | `glab repo view --output json` → `.path_with_namespace` |
| `detect_branch` | `git rev-parse --abbrev-ref HEAD` |
| `fetch_runs` | `GET /projects/{enc}/pipelines?per_page={n}[&source={s}]` |
| `fetch_jobs` | `GET /projects/{enc}/pipelines/{id}/jobs?per_page=100` |
| `cancel_run` | `POST /projects/{enc}/pipelines/{id}/cancel` |
| `delete_run` | `DELETE /projects/{enc}/pipelines/{id}` |
| `rerun_failed` | `POST /projects/{enc}/pipelines/{id}/retry` |
| `fetch_failed_logs` | Fetches jobs → filters failed → concatenates traces |

Notable difference: `fetch_failed_logs` is multi-step because GitLab
has no `--log-failed` equivalent. It fetches all jobs for the pipeline,
filters for `status=failed`, then fetches each job's trace endpoint
(`/jobs/{id}/trace`) and concatenates them.

### 4.3 GlabParser

Unlike `GhParser`, GitLab's API uses different field names and status
strings. `GlabParser` uses intermediate types with `From` conversions:

```
GitLab API JSON                          ciw-core types
─────────────                            ──────────────
GlabPipeline ──From──→ WorkflowRun
  .id             →     .database_id
  .iid            →     .number
  (generated)     →     .display_title = "Pipeline #{iid}"
  .source         →     .name, .event
  .status         →     map_status() + map_conclusion()

GlabJob     ──From──→ Job
  .id             →     .database_id
  .finished_at    →     .completed_at
  (empty)         →     .steps = []  (no step-level API)
```

**Status mapping** (`map_status`):

| GitLab status | → RunStatus |
|---------------|------------|
| `success`, `failed`, `canceled`, `skipped` | `Completed` |
| `running` | `InProgress` |
| `pending`, `created`, `preparing`, `manual`, `scheduled` | `Pending` |
| `waiting_for_resource` | `Waiting` |

**Conclusion mapping** (`map_conclusion`):

| GitLab status | → Conclusion |
|---------------|-------------|
| `success` | `Success` |
| `failed` | `Failure` |
| `canceled` | `Cancelled` |
| `skipped` | `Skipped` |
| in-progress states | `None` |

**Timestamp parsing**: Handles both UTC ISO 8601 (`2024-01-15T10:00:00.000Z`)
and RFC 3339 with offset (`2024-01-15T10:00:00.000+02:00`).

### 4.4 CLI Arguments

```
glw [OPTIONS]
  --project, -p    group/project or group/subgroup/project (auto-detected)
  --branch, -b     filter by branch
  --interval, -i   poll interval in seconds (default: 10, min: 1)
  --limit, -l      max pipelines to fetch (default: 20)
  --source, -s     filter by source (push/schedule/merge_request_event/etc.)
  --no-notify      disable desktop notifications
  --verbose        enable debug logging
```

`validate_project_format()` requires at least 2 `/`-separated segments
(supports nested paths like `group/subgroup/project`).

---

## 5. Shared vs Platform-Specific

### 5.1 What's in ciw-core (100% shared)

| Area | Details |
|------|---------|
| Data models | `WorkflowRun`, `Job`, `Step`, `RunStatus`, `Conclusion`, all enums |
| State management | `AppState` with all navigation, filtering, overlay, notification logic |
| Polling | `Poller`, `backoff_delay`, `fetch_jobs_for_run` |
| Change detection | `detect_changes()` with snapshot tracking |
| Event system | `AppEvent` enum, `EventHandler` (crossterm bridge) |
| Input mapping | `map_key()`, `Action` enum, `InputContext` |
| Full TUI | Header, footer, tree, all 3 overlays, spinner, startup sequence |
| Notifications | `send_desktop()` (feature-gated) |
| Platform identity | `PlatformConfig` struct |

### 5.2 What's unique to each binary

| Aspect | ghw (GitHub) | glw (GitLab) |
|--------|-------------|-------------|
| CLI tool | `gh` (direct subcommands) | `glab api` (REST passthrough) |
| Parsing | Direct serde → core types | Intermediate types + `From` conversion + status mapping |
| Repo identifier | `owner/repo` (1 slash) | `group[/sub]/project` (1+ slashes) |
| CLI filter flag | `--workflow` | `--source` |
| Failed logs | Single `gh run view --log-failed` call | Multi-step: fetch jobs → filter failed → fetch each trace |
| Step-level data | Full step info from API | Always empty (GitLab has no step API) |
| Display title | From API (`displayTitle` field) | Generated: `"Pipeline #{iid}"` |
| UI labels | "Run", "Workflow", "Event" | "Pipeline", "Source" |
| Notification text | "Run cancelled", "Rerun triggered" | "Pipeline cancelled", "Retry triggered" |

### 5.3 Code duplicated across both binaries (not yet in ciw-core)

The `main.rs` files in both binaries are structurally identical (~975 lines
each). The following functions are duplicated with only cosmetic differences
(string literals, type names):

| Function | Difference |
|----------|-----------|
| `main()` | Type names (`GhExecutor` vs `GlabExecutor`), platform constant |
| `run_app()` | Label strings ("Run" vs "Pipeline") |
| `spawn_monitored()` | Identical |
| `build_log_title()` | Identical |
| `build_detail_lines()` | Label names differ |
| `fetch_logs_async()` | Identical |
| `setup_verbose_logging()` | Binary name in path |
| `open_in_browser_impl()` | Identical (in executor.rs) |
| `copy_to_clipboard_impl()` | Identical (in executor.rs) |

---

## 6. Build & Release

### 6.1 Build System

Both binaries share an identical `build.rs` that reads a `BUILD_NUMBER`
file from the workspace root and embeds it as a compile-time env var.
Version strings follow the format: `ghw v0.8.0+42` / `glw v0.8.0+42`.

The `Makefile` provides:
- `make build` — both binaries (release)
- `make install` — builds + installs to `~/.local/bin`
- `make build-ghw` / `make build-glw` — individual builds
- Uses `rm -f` before `install` to avoid "Text file busy" errors

### 6.2 CI (`ci.yml`)

Runs on push to `master` and all PRs:
1. `cargo fmt --all --check`
2. `cargo test --workspace`
3. `cargo clippy --workspace -- -D warnings`
4. `cargo deny check` (license/advisory audit)
5. Cleanup job: deletes workflow runs older than 7 days

### 6.3 Release (`release.yml`)

Triggered by `v*` tags. Matrix build (currently Linux x86_64 only)
that produces `ghw-{tag}-{target}.tar.gz` and `glw-{tag}-{target}.tar.gz`
artifacts, then creates a GitHub release with auto-generated notes.

---

## 7. Key Architectural Patterns

### 7.1 Trait-based Platform Abstraction

The "Strategy pattern via traits" keeps `ciw-core` completely decoupled
from any specific CI platform. The binaries act as **composition roots**
that wire concrete implementations into the shared infrastructure:

```rust
// In ghw/src/main.rs:
let executor: Arc<dyn CiExecutor> = Arc::new(GhExecutor::new(repo));
let parser:   Arc<dyn CiParser>   = Arc::new(GhParser);

// In glw/src/main.rs:
let executor: Arc<dyn CiExecutor> = Arc::new(GlabExecutor::new(project));
let parser:   Arc<dyn CiParser>   = Arc::new(GlabParser);
```

### 7.2 Single-threaded UI + Async Background Work

The TUI runs on the main thread with a simple `loop { draw; match event }`
pattern. All I/O (polling, log fetching, clipboard) is dispatched to
tokio tasks via `spawn_monitored()`, which catches panics and reports
them as `AppEvent::Error`.

### 7.3 Centralized State

All mutable state lives in `AppState`. There is no shared mutable state
between tasks — background tasks communicate exclusively through the
`mpsc::UnboundedChannel<AppEvent>`. This avoids any need for locks or
`Arc<Mutex<_>>`.

### 7.4 Feature-gated Desktop Notifications

The `desktop-notify` feature (default on) gates the `notify-rust`
dependency. When disabled, `send_desktop()` is a no-op. This allows
building on systems without `libdbus` (the dependency for Linux desktop
notifications).
