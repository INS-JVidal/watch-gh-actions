# Implementation Plan: glw (GitLab CI Watcher TUI)

## Context

Build `glw`, a GitLab CI/CD pipeline watcher that replicates all core features of the existing `ghw` (GitHub Actions Watcher). Both binaries share a `ciw-core` library for ~80% code reuse. The refactor must be done incrementally with verification at each phase to ensure zero regressions in `ghw`.

**Scope**: Core feature parity only. No GitLab-specific extras in v1.

---

## Architecture

```
watch-gh-actions/                  (existing root, unchanged name)
├── Cargo.toml                     (MODIFIED → workspace manifest)
├── BUILD_NUMBER                   (unchanged)
├── Makefile                       (MODIFIED → workspace targets)
├── deny.toml, rustfmt.toml        (unchanged)
│
├── crates/
│   ├── ciw-core/                  (NEW — shared library)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── app.rs             (FROM src/app.rs)
│   │       ├── events.rs          (FROM src/events.rs)
│   │       ├── input.rs           (FROM src/input.rs)
│   │       ├── diff.rs            (FROM src/diff.rs)
│   │       ├── notify.rs          (FROM src/notify.rs — parameterized)
│   │       ├── traits.rs          (NEW — CiExecutor + CiParser traits)
│   │       ├── platform.rs        (NEW — PlatformConfig struct)
│   │       ├── poller.rs          (FROM src/gh/poller.rs — generic)
│   │       └── tui/               (FROM src/tui/ — header/startup parameterized)
│   │
│   ├── ghw/                       (MOVED — existing GitHub binary)
│   │   ├── Cargo.toml
│   │   ├── build.rs
│   │   └── src/
│   │       ├── main.rs            (FROM src/main.rs — uses trait objects)
│   │       ├── cli.rs             (FROM src/cli.rs)
│   │       ├── executor.rs        (FROM src/gh/executor.rs — impl CiExecutor)
│   │       └── parser.rs          (FROM src/gh/parser.rs — impl CiParser)
│   │
│   └── glw/                       (NEW — GitLab binary)
│       ├── Cargo.toml
│       ├── build.rs
│       └── src/
│           ├── main.rs            (adapted from ghw main.rs)
│           ├── cli.rs             (NEW)
│           ├── executor.rs        (NEW — GlabExecutor)
│           └── parser.rs          (NEW — GlabParser)
│
└── tests/                         (workspace-level integration tests)
```

---

## Phase 1: Create Workspace Structure

**Goal**: Directory setup and workspace Cargo.toml. Zero code changes, zero behavior change.

### Step 1.1: Create directories

```bash
mkdir -p crates/ciw-core/src/tui
mkdir -p crates/ghw/src
mkdir -p crates/glw/src
```

### Step 1.2: Create workspace root `Cargo.toml`

Replace the current root `Cargo.toml` with a workspace manifest:

```toml
[workspace]
resolver = "2"
members = ["crates/ciw-core", "crates/ghw", "crates/glw"]
```

### Step 1.3: Create `crates/ciw-core/Cargo.toml`

```toml
[package]
name = "ciw-core"
version = "0.7.0"
edition = "2021"
license = "MIT"
description = "Shared core library for CI/CD watcher TUIs"

[dependencies]
ratatui = "0.30"
crossterm = "0.29"
tokio = { version = "1", features = ["rt", "sync", "time", "process"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
color-eyre = "0.6"
tracing = "0.1"
unicode-width = "0.2"
async-trait = "0.1"
notify-rust = { version = "4", optional = true }

[features]
default = ["desktop-notify"]
desktop-notify = ["dep:notify-rust"]
```

### Step 1.4: Create `crates/ghw/Cargo.toml`

```toml
[package]
name = "ghw"
version = "0.7.0"
edition = "2021"
description = "GitHub Actions Watcher TUI"
license = "MIT"

[dependencies]
ciw-core = { path = "../ciw-core" }
clap = { version = "4", features = ["derive"] }
color-eyre = "0.6"
tokio = { version = "1", features = ["rt", "rt-multi-thread", "macros", "sync", "time", "process", "io-util"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["fmt"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
crossterm = "0.29"
ratatui = "0.30"
async-trait = "0.1"

[dev-dependencies]
pretty_assertions = "1"
```

### Step 1.5: Create `crates/glw/Cargo.toml`

Same as ghw but `name = "glw"`, `description = "GitLab CI Watcher TUI"`.

### Step 1.6: Move build.rs

Copy `build.rs` to `crates/ghw/build.rs` and `crates/glw/build.rs`. Update the `BUILD_NUMBER` path to `../../BUILD_NUMBER` (relative to crate root).

### Verification

```bash
# Structure exists, no compilation yet (sources not moved)
ls crates/ciw-core/src/ crates/ghw/src/ crates/glw/src/
```

---

## Phase 2: Extract ciw-core (Shared Library)

**Goal**: Move platform-agnostic code to `crates/ciw-core/src/`. Create trait definitions.

### Step 2.1: Define traits — `crates/ciw-core/src/traits.rs` (NEW)

```rust
use crate::app::{Job, WorkflowRun};
use async_trait::async_trait;
use color_eyre::eyre::Result;

/// Platform-specific CI/CD command executor.
/// Implementations: GhExecutor (GitHub), GlabExecutor (GitLab).
#[async_trait]
pub trait CiExecutor: Send + Sync {
    async fn check_available(&self) -> Result<()>;
    async fn detect_repo(&self) -> Result<String>;
    async fn detect_branch(&self) -> Result<String>;
    async fn fetch_runs(&self, limit: usize, filter: Option<&str>) -> Result<String>;
    async fn fetch_jobs(&self, run_id: u64) -> Result<String>;
    async fn cancel_run(&self, run_id: u64) -> Result<()>;
    async fn delete_run(&self, run_id: u64) -> Result<()>;
    async fn rerun_failed(&self, run_id: u64) -> Result<()>;
    async fn fetch_failed_logs(&self, run_id: u64) -> Result<String>;
    async fn fetch_failed_logs_for_job(&self, run_id: u64, job_id: u64) -> Result<String>;
    fn open_in_browser(&self, url: &str) -> Result<()>;
    async fn copy_to_clipboard(&self, text: &str) -> Result<()>;
}

/// Platform-specific JSON parser.
/// Implementations: GhParser (GitHub), GlabParser (GitLab).
pub trait CiParser: Send + Sync {
    fn parse_runs(&self, json: &str) -> Result<Vec<WorkflowRun>>;
    fn parse_jobs(&self, json: &str) -> Result<Vec<Job>>;
    fn process_log_output(&self, raw: &str, max_lines: usize) -> (String, bool);
}
```

### Step 2.2: Define PlatformConfig — `crates/ciw-core/src/platform.rs` (NEW)

```rust
/// Branding and display configuration for each CI platform.
pub struct PlatformConfig {
    pub name: &'static str,            // "ghw" or "glw"
    pub full_name: &'static str,       // "GitHub Actions" or "GitLab CI"
    pub cli_tool: &'static str,        // "gh" or "glab"
    pub install_hint: &'static str,    // URL to install the CLI tool
    pub ascii_art: &'static [&'static str],
}
```

### Step 2.3: Move modules to ciw-core

**Verbatim moves (no changes needed)**:

| Source → Destination |
|---------------------|
| `src/app.rs` → `crates/ciw-core/src/app.rs` |
| `src/events.rs` → `crates/ciw-core/src/events.rs` |
| `src/input.rs` → `crates/ciw-core/src/input.rs` |
| `src/diff.rs` → `crates/ciw-core/src/diff.rs` |
| `src/tui/mod.rs` → `crates/ciw-core/src/tui/mod.rs` |
| `src/tui/render.rs` → `crates/ciw-core/src/tui/render.rs` |
| `src/tui/tree.rs` → `crates/ciw-core/src/tui/tree.rs` |
| `src/tui/footer.rs` → `crates/ciw-core/src/tui/footer.rs` |
| `src/tui/spinner.rs` → `crates/ciw-core/src/tui/spinner.rs` |
| `src/tui/log_overlay.rs` → `crates/ciw-core/src/tui/log_overlay.rs` |
| `src/tui/detail_overlay.rs` → `crates/ciw-core/src/tui/detail_overlay.rs` |
| `src/tui/confirm_overlay.rs` → `crates/ciw-core/src/tui/confirm_overlay.rs` |

**Moves with modifications**:

| Source → Destination | Changes |
|---------------------|---------|
| `src/notify.rs` → `crates/ciw-core/src/notify.rs` | Accept notification title/body as parameters instead of hardcoding "CI Passed"/"CI Failed". Or add platform labels to `AppState`. |
| `src/tui/header.rs` → `crates/ciw-core/src/tui/header.rs` | Replace `format!(" ghw v{}+{} ", ...)` with a `version_string` field from `AppState.config` |
| `src/tui/startup.rs` → `crates/ciw-core/src/tui/startup.rs` | Replace hardcoded `GHW_ART` with `PlatformConfig.ascii_art`. Change `run_startup` to accept `&dyn CiExecutor`, `&dyn CiParser`, `&PlatformConfig`. Phase labels from config. |
| `src/gh/poller.rs` → `crates/ciw-core/src/poller.rs` | See Step 2.4 |

### Step 2.4: Generalize Poller

**Current struct** (in `src/gh/poller.rs`):
```rust
pub struct Poller {
    repo: String,
    limit: usize,
    workflow: Option<String>,
    tx: mpsc::UnboundedSender<AppEvent>,
    interval_rx: watch::Receiver<u64>,
}
```

**New struct** (in `crates/ciw-core/src/poller.rs`):
```rust
use std::sync::Arc;
use crate::traits::{CiExecutor, CiParser};

pub struct Poller {
    executor: Arc<dyn CiExecutor>,
    parser: Arc<dyn CiParser>,
    limit: usize,
    filter: Option<String>,
    tx: mpsc::UnboundedSender<AppEvent>,
    interval_rx: watch::Receiver<u64>,
}
```

**`poll_once` change**:
```rust
// Before:
executor::fetch_runs(&self.repo, self.limit, self.workflow.as_deref())
// After:
self.executor.fetch_runs(self.limit, self.filter.as_deref())
```

**`fetch_jobs_for_run` change**:
```rust
// Before:
pub async fn fetch_jobs_for_run(repo: &str, run_id: u64, tx: &mpsc::UnboundedSender<AppEvent>)
// After:
pub async fn fetch_jobs_for_run(
    executor: &dyn CiExecutor,
    parser: &dyn CiParser,
    run_id: u64,
    tx: &mpsc::UnboundedSender<AppEvent>,
)
```

### Step 2.5: Create `crates/ciw-core/src/lib.rs`

```rust
#![warn(clippy::pedantic)]
#![allow(
    clippy::module_name_repetitions,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::struct_excessive_bools,
    clippy::wildcard_imports,
    clippy::too_many_lines,
    clippy::must_use_candidate,
    clippy::return_self_not_must_use,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::fn_params_excessive_bools,
    clippy::too_many_arguments,
    clippy::doc_markdown
)]

pub mod app;
pub mod diff;
pub mod events;
pub mod input;
pub mod notify;
pub mod platform;
pub mod poller;
pub mod traits;
pub mod tui;
```

### Step 2.6: Add version_string to AppConfig

In `crates/ciw-core/src/app.rs`, add to `AppConfig`:
```rust
pub struct AppConfig {
    pub repo: String,
    pub branch: Option<String>,
    pub limit: usize,
    pub workflow_filter: Option<String>,
    pub version_string: String,  // NEW: "ghw v0.7.0+35" or "glw v0.7.0+35"
}
```

### Verification

```bash
cargo build -p ciw-core
```

---

## Phase 3: Adapt ghw to Use ciw-core

**Goal**: The `crates/ghw/` binary imports from `ciw-core` and provides `GhExecutor`/`GhParser`. Zero behavior change for users.

### Step 3.1: Create GhExecutor — `crates/ghw/src/executor.rs`

Move code from `src/gh/executor.rs`. Wrap free functions into a struct:

```rust
use std::sync::Arc;
use ciw_core::traits::CiExecutor;
use async_trait::async_trait;
use color_eyre::eyre::Result;

pub struct GhExecutor {
    pub repo: String,
}

#[async_trait]
impl CiExecutor for GhExecutor {
    async fn check_available(&self) -> Result<()> {
        run_gh(&["auth", "status"]).await.map(|_| ())
    }

    async fn detect_repo(&self) -> Result<String> {
        // existing detect_repo() logic
    }

    async fn detect_branch(&self) -> Result<String> {
        // existing detect_branch() logic
    }

    async fn fetch_runs(&self, limit: usize, workflow: Option<&str>) -> Result<String> {
        // existing fetch_runs() logic using self.repo
    }

    async fn fetch_jobs(&self, run_id: u64) -> Result<String> {
        // existing fetch_jobs() logic using self.repo
    }

    async fn cancel_run(&self, run_id: u64) -> Result<()> {
        // existing cancel_run() using self.repo
    }

    async fn delete_run(&self, run_id: u64) -> Result<()> {
        // existing delete_run() using self.repo
    }

    async fn rerun_failed(&self, run_id: u64) -> Result<()> {
        // existing rerun_workflow() using self.repo
    }

    async fn fetch_failed_logs(&self, run_id: u64) -> Result<String> {
        // existing fetch_failed_logs() using self.repo
    }

    async fn fetch_failed_logs_for_job(&self, run_id: u64, job_id: u64) -> Result<String> {
        // existing fetch_failed_logs_for_job() using self.repo
    }

    fn open_in_browser(&self, url: &str) -> Result<()> {
        // existing open_in_browser() — no self.repo needed
    }

    async fn copy_to_clipboard(&self, text: &str) -> Result<()> {
        // existing copy_to_clipboard()
    }
}

// Keep run_gh(), classify_gh_error(), check_log_size() as private helpers
```

### Step 3.2: Create GhParser — `crates/ghw/src/parser.rs`

Move code from `src/gh/parser.rs`. Wrap functions:

```rust
use ciw_core::traits::CiParser;
use ciw_core::app::{Job, WorkflowRun};
use color_eyre::eyre::Result;

pub struct GhParser;

impl CiParser for GhParser {
    fn parse_runs(&self, json: &str) -> Result<Vec<WorkflowRun>> {
        // existing parse_runs() logic
    }

    fn parse_jobs(&self, json: &str) -> Result<Vec<Job>> {
        // existing parse_jobs() logic
    }

    fn process_log_output(&self, raw: &str, max_lines: usize) -> (String, bool) {
        // existing process_log_output() logic
    }
}
```

### Step 3.3: Move CLI — `crates/ghw/src/cli.rs`

Copy `src/cli.rs` verbatim. No changes needed.

### Step 3.4: Adapt main.rs — `crates/ghw/src/main.rs`

Adapt `src/main.rs`:

1. Replace imports:
   ```rust
   // Before:
   use ghw::{app, cli, diff, events, gh, input, notify, tui};
   // After:
   use ciw_core::{app, diff, events, input, notify, tui, poller, platform};
   use ciw_core::traits::{CiExecutor, CiParser};
   mod cli;
   mod executor;
   mod parser;
   ```

2. Create executor and parser at startup:
   ```rust
   let executor = Arc::new(executor::GhExecutor { repo: repo.clone() });
   let parser = Arc::new(parser::GhParser);
   ```

3. Replace all `gh::executor::X()` calls with `executor.X()`:
   - Line 268: `executor.fetch_runs(limit, wf.as_deref())`
   - Line 313: `executor.rerun_failed(run_id)`
   - Line 377: `executor.cancel_run(run_id)`
   - Line 405: `executor.delete_run(run_id)`
   - Line 434: `executor.open_in_browser(url)`
   - Line 498: `executor.copy_to_clipboard(&text)`
   - Line 865: `executor.fetch_failed_logs_for_job(run_id, jid)`
   - Line 867: `executor.fetch_failed_logs(run_id)`

4. Replace all `gh::parser::X()` calls with `parser.X()`:
   - Line 269: `parser.parse_runs(&json)`
   - Line 872: `parser.process_log_output(&raw, app::LOG_MAX_LINES)`

5. Pass `executor`/`parser` to `Poller::new()`:
   ```rust
   let poller = Poller::new(executor.clone(), parser.clone(), limit, wf, poller_tx, interval_rx);
   ```

6. Pass `executor`/`parser` to `fetch_jobs_for_run()`:
   ```rust
   poller::fetch_jobs_for_run(executor.as_ref(), parser.as_ref(), run_id, &tx2).await;
   ```

7. Update `run_startup()` call to pass `executor`/`parser`/`PlatformConfig`.

8. Update `build_detail_lines` and `build_log_title` — these stay in ghw's main.rs (or move to ciw-core as generic functions).

### Step 3.5: Delete old `src/` directory

After all code is successfully moved and compiling in the workspace, remove the old `src/` directory.

### Verification (CRITICAL — must pass before proceeding)

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace
cargo fmt --all --check
# Manual test: run ghw against a real GitHub repo
```

---

## Phase 4: Implement glw

**Goal**: Create the GitLab binary.

### Step 4.1: `crates/glw/src/cli.rs`

```rust
use clap::Parser;

const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "+", env!("BUILD_NUMBER"));

#[derive(Parser, Debug)]
#[command(name = "glw", version = VERSION, about = "GitLab CI Watcher TUI")]
pub struct Cli {
    /// Project path in group/project format (auto-detected from cwd)
    #[arg(short = 'p', long = "project")]
    pub project: Option<String>,

    /// Branch to filter (auto-detected from cwd)
    #[arg(short, long)]
    pub branch: Option<String>,

    /// Poll interval in seconds
    #[arg(short, long, default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..))]
    pub interval: u64,

    /// Maximum number of pipelines to display
    #[arg(short, long, default_value_t = 20)]
    pub limit: usize,

    /// Filter by pipeline source (push, schedule, merge_request_event, etc.)
    #[arg(short, long)]
    pub source: Option<String>,

    /// Disable desktop notifications
    #[arg(long)]
    pub no_notify: bool,

    /// Enable verbose logging to $XDG_STATE_HOME/glw/debug.log
    #[arg(long)]
    pub verbose: bool,
}

/// Validates that `project` has at least 2 path segments.
pub fn validate_project_format(project: &str) -> Result<(), String> {
    let parts: Vec<&str> = project.split('/').collect();
    if parts.len() < 2 || parts.iter().any(|p| p.is_empty()) || project.contains(char::is_whitespace) {
        return Err(format!(
            "Invalid project format '{project}'. Expected 'group/project' or 'group/subgroup/project'."
        ));
    }
    Ok(())
}
```

### Step 4.2: `crates/glw/src/executor.rs` — GlabExecutor

```rust
use ciw_core::traits::CiExecutor;
use async_trait::async_trait;
use color_eyre::eyre::{eyre, Result};
use std::time::Duration;
use tokio::process::Command;

const GLAB_TIMEOUT: Duration = Duration::from_secs(30);

pub struct GlabExecutor {
    pub project: String,
    pub encoded_project: String,  // URL-encoded for API calls
}

impl GlabExecutor {
    pub fn new(project: String) -> Self {
        let encoded = project.replace('/', "%2F");
        Self { project, encoded_project: encoded }
    }
}
```

**Command mapping (each trait method)**:

| Trait method | Implementation |
|---|---|
| `check_available()` | `glab auth status` |
| `detect_repo()` | `glab repo view --output json` → parse `path_with_namespace` from JSON |
| `detect_branch()` | `git rev-parse --abbrev-ref HEAD` (same as ghw) |
| `fetch_runs(limit, source)` | `glab ci list -F json --per-page {limit}` + optional `--source {source}` |
| `fetch_jobs(pipeline_id)` | `glab ci get -p {id} --with-job-details -F json` |
| `cancel_run(pipeline_id)` | `glab ci cancel pipeline {id}` |
| `delete_run(pipeline_id)` | `glab ci delete {id}` |
| `rerun_failed(pipeline_id)` | `glab api -X POST /projects/{encoded}/pipelines/{id}/retry` |
| `fetch_failed_logs(pipeline_id)` | 1) `fetch_jobs()` → find failed jobs, 2) `glab api /projects/{encoded}/jobs/{jid}/trace` per failed job, 3) concatenate with headers |
| `fetch_failed_logs_for_job(_, job_id)` | `glab api /projects/{encoded}/jobs/{job_id}/trace` |
| `open_in_browser(url)` | Same OS commands as ghw (copy from ciw-core or keep as shared free fn) |
| `copy_to_clipboard(text)` | Same as ghw |

**Error classification**:
```rust
fn classify_glab_error(stderr: &str) -> String {
    if stderr.contains("not logged") || stderr.contains("auth login") {
        "Not authenticated with glab. Run `glab auth login` first.".to_string()
    } else if stderr.contains("not a git repository") || stderr.contains("could not determine") {
        "Not in a GitLab repository. Use --project flag or cd into a repo.".to_string()
    } else {
        let trimmed = stderr.trim();
        if trimmed.is_empty() { "glab command failed".to_string() }
        else { format!("glab command failed: {trimmed}") }
    }
}
```

**Log fetching strategy** (most complex part):

`fetch_failed_logs(pipeline_id)`:
1. Call `self.fetch_jobs(pipeline_id)` → get JSON with all jobs
2. Parse to find jobs with status `"failed"`
3. For each failed job: `glab api /projects/{encoded}/jobs/{jid}/trace`
4. Concatenate: `"=== Job: {name} ===\n{trace}\n\n"`
5. Return combined output

### Step 4.3: `crates/glw/src/parser.rs` — GlabParser

**Pipeline list JSON** (`glab ci list -F json`):
```json
[{
    "id": 12345,
    "iid": 42,
    "ref": "main",
    "status": "success",
    "source": "push",
    "created_at": "2024-01-15T10:00:00.000Z",
    "updated_at": "2024-01-15T10:05:00.000Z",
    "web_url": "https://gitlab.com/group/project/-/pipelines/12345"
}]
```

**Mapping to WorkflowRun**:

| GitLab field | → WorkflowRun field |
|---|---|
| `id` | `database_id` |
| `iid` | `number` |
| `ref` | `head_branch` |
| `status` | `status` + `conclusion` (via mapping fn) |
| `source` | `event` |
| `created_at` | `created_at` |
| `updated_at` | `updated_at` |
| `web_url` | `url` |
| *(generated)* | `display_title` = `"Pipeline #{iid}"` |
| `source` | `name` |

**Status mapping function**:

```rust
fn map_gitlab_status(status: &str) -> (RunStatus, Option<Conclusion>) {
    match status {
        "success"              => (RunStatus::Completed, Some(Conclusion::Success)),
        "failed"               => (RunStatus::Completed, Some(Conclusion::Failure)),
        "canceled"             => (RunStatus::Completed, Some(Conclusion::Cancelled)),
        "skipped"              => (RunStatus::Completed, Some(Conclusion::Skipped)),
        "running"              => (RunStatus::InProgress, None),
        "pending"              => (RunStatus::Pending, None),
        "created"              => (RunStatus::Pending, None),
        "waiting_for_resource" => (RunStatus::Waiting, None),
        "preparing"            => (RunStatus::Pending, None),
        "manual"               => (RunStatus::Pending, None),
        "scheduled"            => (RunStatus::Pending, None),
        _                      => (RunStatus::Unknown, None),
    }
}
```

**Pipeline jobs JSON** (`glab ci get -p ID --with-job-details -F json`):
```json
{
    "id": 12345,
    "status": "success",
    "jobs": [{
        "id": 67890,
        "name": "build",
        "stage": "build",
        "status": "success",
        "started_at": "2024-01-15T10:00:00.000Z",
        "finished_at": "2024-01-15T10:02:00.000Z",
        "web_url": "https://gitlab.com/group/project/-/jobs/67890"
    }]
}
```

**Mapping to Job**:

| GitLab field | → Job field |
|---|---|
| `id` | `database_id` |
| `name` | `name` |
| `status` | `status` + `conclusion` (same mapping fn) |
| `started_at` | `started_at` |
| `finished_at` | `completed_at` (note: GitLab uses `finished_at`) |
| `web_url` | `url` |
| *(always)* | `steps` = `vec![]` (GitLab has no step-level API) |

### Step 4.4: `crates/glw/src/main.rs`

Adapted from `crates/ghw/src/main.rs` with these changes:

1. Import `GlabExecutor` instead of `GhExecutor`, `GlabParser` instead of `GhParser`
2. Use `glw` CLI args (project instead of repo, source instead of workflow)
3. Log directory: `$XDG_STATE_HOME/glw/` instead of `ghw/`
4. Terminal title: `"watching {project}"` instead of `"watching {repo}"`
5. `build_detail_lines`: "Pipeline #N" instead of "Run #N", "Source" instead of "Event"
6. Notification labels: "Pipeline Passed"/"Pipeline Failed" instead of "CI Passed"/"CI Failed"
7. GitLab ASCII art in startup

### Step 4.5: glw ASCII Art

Create a GitLab-themed art block similar in style to `GHW_ART`.

### Verification

```bash
cargo build -p glw
cargo test -p glw
# Manual: run against a public GitLab project
# glw --project gitlab-org/gitlab --limit 5
```

---

## Phase 5: Testing

### Step 5.1: glw parser unit tests

Create fixtures with real GitLab JSON:
- Pipeline list: success, failed, running, canceled, manual, scheduled statuses
- Pipeline with jobs: various stages, null timestamps, empty job list
- Edge cases: unknown status, empty response, oversized response

### Step 5.2: glw executor error tests

Test `classify_glab_error()` with:
- "You are not logged into any GitLab hosts"
- "not a git repository"
- Empty stderr
- Generic error messages

### Step 5.3: Existing ghw tests

- Move `tests/fixtures.rs` and `tests/integration.rs` under `crates/ghw/tests/`
- All existing parser/input/diff/app/poller tests must pass unchanged

### Step 5.4: ciw-core tests

- All tests that were in modules now in ciw-core (app, input, diff, poller backoff) keep working
- Run `cargo test -p ciw-core`

### Verification

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all --check
```

---

## Phase 6: Build System Updates

### Step 6.1: Makefile

```makefile
PREFIX ?= $(HOME)/.local
BINDIR := $(PREFIX)/bin

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
```

### Step 6.2: CI workflow updates

`.github/workflows/ci.yml`:
- Change `cargo test` → `cargo test --workspace`
- Change `cargo fmt --check` → `cargo fmt --all --check`
- Change `cargo clippy` → `cargo clippy --workspace`

`.github/workflows/release.yml`:
- Build both `ghw` and `glw` binaries
- Package both in release artifacts

---

## Verification Checklist

### After Phase 3 (workspace refactor):
- [ ] `cargo build --workspace` compiles
- [ ] `cargo test --workspace` — all existing tests pass
- [ ] `cargo clippy --workspace` — no warnings
- [ ] `cargo fmt --all --check` — formatted
- [ ] Manual: run `ghw` against a GitHub repo — identical behavior

### After Phase 4 (glw implementation):
- [ ] `cargo build -p glw` compiles
- [ ] `cargo test -p glw` — parser tests pass
- [ ] Manual: `glw --project GROUP/PROJECT --limit 5`
  - [ ] Pipeline list loads
  - [ ] Expand pipeline → jobs appear
  - [ ] Press `d` → detail overlay
  - [ ] Press `f` → filter cycles
  - [ ] Press `e` on failed job → log overlay
  - [ ] Press `c` on running pipeline → cancel with confirm
  - [ ] Press `x` on completed pipeline → delete with confirm
  - [ ] Press `R` on failed pipeline → retry
  - [ ] Press `o` → opens browser
  - [ ] Desktop notification on completion

### Regression:
- [ ] `cargo test --workspace` — everything green
- [ ] ghw still works identically after workspace refactor

---

## Critical Files Reference

| Current path | Role | Destination |
|---|---|---|
| `src/app.rs` | Core state, data models, 897 lines | `crates/ciw-core/src/app.rs` |
| `src/events.rs` | Event system, 127 lines | `crates/ciw-core/src/events.rs` |
| `src/input.rs` | Key mapping, 548 lines | `crates/ciw-core/src/input.rs` |
| `src/diff.rs` | Change detection, 256 lines | `crates/ciw-core/src/diff.rs` |
| `src/notify.rs` | Notifications, 37 lines | `crates/ciw-core/src/notify.rs` |
| `src/gh/executor.rs` | GitHub CLI wrapper, 315 lines | `crates/ghw/src/executor.rs` |
| `src/gh/parser.rs` | GitHub JSON parser, 354 lines | `crates/ghw/src/parser.rs` |
| `src/gh/poller.rs` | Polling loop, 224 lines | `crates/ciw-core/src/poller.rs` |
| `src/main.rs` | Event loop, 898 lines | `crates/ghw/src/main.rs` |
| `src/cli.rs` | CLI args, 92 lines | `crates/ghw/src/cli.rs` |
| `src/tui/header.rs` | Header bar, 84 lines | `crates/ciw-core/src/tui/header.rs` |
| `src/tui/startup.rs` | Startup screen, 263 lines | `crates/ciw-core/src/tui/startup.rs` |
| `src/tui/tree.rs` | Tree rendering, 428 lines | `crates/ciw-core/src/tui/tree.rs` |
| `Cargo.toml` | Package manifest | Workspace manifest + per-crate |
| `Makefile` | Build targets | Updated for workspace |
