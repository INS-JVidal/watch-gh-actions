# Exploration Report: ghw vs glw Feature Comparison

## 1. Current ghw Architecture Overview

**ghw** is a terminal-based TUI for monitoring GitHub Actions workflows in real-time. Written in Rust (~5,700 lines), it uses the `gh` CLI as its backend for all GitHub API interactions.

### Tech Stack

| Component | Technology |
|-----------|-----------|
| Language | Rust 2021 edition |
| TUI framework | ratatui 0.30 + crossterm 0.29 |
| Async runtime | tokio (multi-threaded) |
| CLI parsing | clap 4 (derive macros) |
| JSON parsing | serde + serde_json |
| Date/time | chrono 0.4 |
| Notifications | notify-rust 4 (optional feature) |
| Error handling | color-eyre 0.6 |
| Logging | tracing + tracing-subscriber |

### Module Structure

| Module | File | Lines | Responsibility |
|--------|------|-------|----------------|
| Entry point | `src/main.rs` | ~898 | Event loop, action dispatch, app wiring |
| App state | `src/app.rs` | ~897 | State management, tree navigation, overlays, data models |
| CLI args | `src/cli.rs` | ~92 | Argument parsing with clap |
| Events | `src/events.rs` | ~127 | Cross-thread event system |
| Input | `src/input.rs` | ~548 | Key mapping → Action enum |
| Diff | `src/diff.rs` | ~256 | Change detection between polls |
| Notifications | `src/notify.rs` | ~37 | Desktop notifications |
| GH executor | `src/gh/executor.rs` | ~315 | Wraps `gh` CLI commands |
| GH parser | `src/gh/parser.rs` | ~354 | JSON parsing → typed structs |
| Poller | `src/gh/poller.rs` | ~224 | Background polling with backoff |
| TUI render | `src/tui/render.rs` | ~59 | Main render dispatcher |
| TUI header | `src/tui/header.rs` | ~84 | Title bar |
| TUI tree | `src/tui/tree.rs` | ~428 | Tree view rendering |
| TUI footer | `src/tui/footer.rs` | ~74 | Key binding hints |
| TUI startup | `src/tui/startup.rs` | ~263 | Animated startup sequence |
| TUI spinner | `src/tui/spinner.rs` | ~41 | Loading animation |
| TUI log overlay | `src/tui/log_overlay.rs` | ~63 | Failed log viewer |
| TUI detail overlay | `src/tui/detail_overlay.rs` | ~64 | Run/job/step details |
| TUI confirm overlay | `src/tui/confirm_overlay.rs` | ~51 | Yes/no confirmation |

### Data Models

```
WorkflowRun {
    database_id: u64,         // unique run ID
    display_title: String,    // human-readable title
    name: String,             // workflow name
    head_branch: String,      // branch
    status: RunStatus,        // Completed, InProgress, Queued, etc.
    conclusion: Option<Conclusion>,  // Success, Failure, Cancelled, etc.
    created_at, updated_at: DateTime<Utc>,
    event: String,            // push, pull_request, etc.
    number: u64,              // run number
    url: String,              // browser URL
    jobs: Option<Vec<Job>>,   // lazily fetched on expand
}

Job {
    database_id: Option<u64>,
    name: String,
    status: RunStatus,
    conclusion: Option<Conclusion>,
    started_at, completed_at: Option<DateTime<Utc>>,
    url: String,
    steps: Vec<Step>,
}

Step {
    name: String,
    status: RunStatus,
    conclusion: Option<Conclusion>,
    number: u64,
    started_at, completed_at: Option<DateTime<Utc>>,
}
```

### Keybindings

| Key | Action | Context |
|-----|--------|---------|
| `q` / `Ctrl+C` | Quit | Always |
| `↑`/`k`, `↓`/`j` | Navigate | Main view |
| `→`/`l`/`Enter` | Expand | Main view |
| `←`/`h` | Collapse | Main view |
| `Space` | Toggle expand/collapse | Main view |
| `r` | Manual refresh | Main view |
| `R` | Rerun failed workflows | Completed failed run |
| `c` | Cancel run (with confirm) | In-progress run |
| `x` | Delete run (with confirm) | Completed run |
| `o` | Open in browser | Main view |
| `e` | View failure logs | Failed item |
| `y` | Copy logs to clipboard | Log overlay |
| `d` | Show details | Main view |
| `f` | Cycle filter (All/Active/Branch) | Main view |
| `b` | Filter to current branch | Main view |
| `1`-`9` | Quick-select run | Main view |
| `j`/`k`, PgUp/PgDn, `g`/`G` | Scroll | Log overlay |

### gh CLI Commands Used

| Operation | Command |
|-----------|---------|
| Auth check | `gh auth status` |
| Detect repo | `gh repo view --json nameWithOwner -q .nameWithOwner` |
| Detect branch | `git rev-parse --abbrev-ref HEAD` |
| List runs | `gh run list --repo R --limit N --json fields [--workflow W]` |
| Get jobs | `gh run view --repo R ID --json jobs` |
| Cancel | `gh run cancel --repo R ID` |
| Delete | `gh run delete --repo R ID` |
| Rerun failed | `gh run rerun --failed --repo R ID` |
| Fetch failed logs | `gh run view --repo R ID --log-failed [--job J]` |
| Open browser | OS-specific (`xdg-open`, `wslview`, `open`, `cmd start`) |
| Clipboard | OS-specific (`xclip`, `wl-copy`, `pbcopy`, `clip.exe`) |

### CLI Arguments

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-r, --repo` | String | auto-detect | Repository `owner/repo` |
| `-b, --branch` | String | auto-detect | Branch to filter |
| `-i, --interval` | u64 | 10 | Poll interval (seconds) |
| `-l, --limit` | usize | 20 | Max runs to display |
| `-w, --workflow` | String | none | Filter by workflow name |
| `--no-notify` | flag | false | Disable desktop notifications |
| `--verbose` | flag | false | Enable debug logging |

---

## 2. GitLab CLI (`glab`) Capabilities

### Available Commands for CI/CD

| Operation | Command | Notes |
|-----------|---------|-------|
| Auth check | `glab auth status` | Direct equivalent |
| Detect project | `glab repo view --output json` | Returns `path_with_namespace` |
| List pipelines | `glab ci list -F json --per-page N` | Supports `--status`, `--ref`, `--source` filters |
| Get pipeline + jobs | `glab ci get -p ID --with-job-details -F json` | Direct equivalent |
| Cancel pipeline | `glab ci cancel pipeline ID` | Direct equivalent |
| Delete pipeline | `glab ci delete ID` | Also supports `--status`, `--older-than` filters |
| Retry pipeline | `glab api -X POST /projects/:id/pipelines/:pid/retry` | Retries all failed/canceled jobs |
| Retry single job | `glab ci retry JOB_ID` | Also by name: `glab ci retry job-name` |
| Fetch job log | `glab api /projects/:id/jobs/:jid/trace` | Returns plain text |
| Play manual job | `glab api -X POST /projects/:id/jobs/:jid/play` | GitLab-specific |
| Get pipeline vars | `glab ci get --with-variables -F json` | Requires Maintainer role |
| Get test report | `glab api /projects/:id/pipelines/:pid/test_report_summary` | GitLab-specific |

### GitLab CI/CD Concepts vs GitHub Actions

| Concept | GitHub Actions | GitLab CI/CD |
|---------|----------------|-------------|
| Config file | `.github/workflows/*.yml` (multiple) | `.gitlab-ci.yml` (single) |
| Top-level unit | Workflow run | Pipeline |
| Execution unit | Job | Job |
| Sub-unit | Step (within job) | Script command (not exposed via API) |
| Grouping | None (flat jobs) | Stages (build, test, deploy) |
| Dependencies | `needs:` key | `needs:` + DAG pipelines |
| Trigger types | Events (push, PR, schedule) | Sources (push, merge_request, schedule, trigger, web, api) |
| Manual gate | No native equivalent | `when: manual` jobs |
| Environments | Basic | Full lifecycle with deployment approvals |

### GitLab Job Status Values

GitLab has more granular statuses than GitHub:

| Status | Description | GitHub equivalent |
|--------|-------------|-------------------|
| `created` | Job created, not scheduled | Queued |
| `waiting_for_resource` | Waiting for runner | Waiting |
| `preparing` | Runner preparing | Pending |
| `pending` | Queued for execution | Pending |
| `running` | Currently executing | InProgress |
| `success` | Completed successfully | Completed + Success |
| `failed` | Execution failed | Completed + Failure |
| `canceled` | Manually canceled | Completed + Cancelled |
| `skipped` | Skipped (conditions not met) | Completed + Skipped |
| `manual` | Waiting for manual trigger | *(no equivalent)* |
| `scheduled` | Scheduled for later | *(no equivalent)* |

---

## 3. Feature Equivalence: ghw → glw

### Core Features (Full Parity)

| # | Feature | ghw | glw | Status |
|---|---------|-----|-----|--------|
| 1 | Real-time monitoring | Poll `gh run list` | Poll `glab ci list` | Direct equivalent |
| 2 | Hierarchical tree | Run → Job → Step | Pipeline → Job | 2 levels instead of 3 |
| 3 | Status icons | ✓ ✗ ⟳ · ⊘ ! | Same icons | Same mapping |
| 4 | Cancel (`c`) | `gh run cancel` | `glab ci cancel pipeline` | Direct |
| 5 | Delete (`x`) | `gh run delete` | `glab ci delete` | Direct |
| 6 | Rerun failed (`R`) | `gh run rerun --failed` | API POST .../retry | Direct |
| 7 | View logs (`e`) | `gh run view --log-failed` | API GET .../trace per failed job | Multi-step |
| 8 | Open browser (`o`) | OS commands | Same OS commands | Different URL format |
| 9 | Copy logs (`y`) | OS clipboard | Same | Fully reusable |
| 10 | Filters (`f`/`b`) | All/Active/Branch | Same | Fully reusable |
| 11 | Adaptive polling | 3s/10s/30s | Same | Fully reusable |
| 12 | Desktop notify | notify-rust | Same | Fully reusable |
| 13 | Quick-select | 1-9 keys | Same | Fully reusable |
| 14 | Detail overlay (`d`) | Run/Job/Step info | Pipeline/Job info | Adapted labels |
| 15 | Confirm dialog | Yes/No | Same | Fully reusable |
| 16 | Auto-detect repo | `gh repo view` | `glab repo view` | Different format |
| 17 | Auto-detect branch | `git rev-parse` | Same | Fully reusable |
| 18 | Auth check | `gh auth status` | `glab auth status` | Direct |

### CLI Arguments Mapping

| ghw flag | glw flag | Reason for change |
|----------|----------|-------------------|
| `--repo owner/repo` | `--project group/project` | GitLab uses "project" + nested groups |
| `--workflow NAME` | `--source TYPE` | GitLab filters by trigger source, not workflow |
| `--interval N` | `--interval N` | Same |
| `--limit N` | `--limit N` | Same |
| `--branch NAME` | `--branch NAME` | Same |
| `--no-notify` | `--no-notify` | Same |
| `--verbose` | `--verbose` | Same |

### Key Limitations

| Limitation | Impact | Mitigation |
|-----------|--------|------------|
| No step-level API | Tree is 2 levels (Pipeline > Job) instead of 3 | Acceptable — jobs are the primary unit |
| No `display_title` | Pipelines lack a human-readable title | Generate: `"Pipeline #{iid}"` |
| No `--log-failed` shortcut | Must identify failed jobs, then fetch each trace | Multi-step in executor, transparent to user |
| Project path format | GitLab uses `group/subgroup/project` (2+ segments) | Relaxed validation (2+ parts vs exactly 2) |

---

## 4. GitLab-Specific Features NOT in GitHub Actions

These are features that GitLab supports but were **excluded from v1** (core parity only):

| Feature | Description | Why excluded for v1 |
|---------|-------------|---------------------|
| Manual job triggering | `when: manual` jobs, play via API | Adds new action + UI complexity |
| Pipeline source display | Show trigger type (push/schedule/MR) in tree | Display-only, low priority |
| Stage grouping | Group jobs by stage name | Cosmetic change |
| Bulk delete with filters | `glab ci delete --status=failed --older-than 24h` | Power-user feature |
| Pipeline variables | View CI variables in detail overlay | Requires Maintainer role |
| Self-managed URL | `--url` flag for self-hosted GitLab | Can be added later |
| Test report summaries | Show pass/fail/error counts | Future enhancement |
| Scheduled pipeline icon | Distinct `⏲` icon for scheduled status | Low impact |

---

## 5. Code Reuse Analysis

### Platform-Agnostic (→ ciw-core shared library, ~80%)

| Module | Lines | Reuse | Changes needed |
|--------|-------|-------|----------------|
| `app.rs` | 897 | 100% | None |
| `events.rs` | 127 | 100% | None |
| `input.rs` | 548 | 100% | None |
| `diff.rs` | 256 | 100% | None |
| `notify.rs` | 37 | 95% | Parameterize notification labels |
| `tui/render.rs` | 59 | 100% | None |
| `tui/tree.rs` | 428 | 100% | None |
| `tui/footer.rs` | 74 | 100% | None |
| `tui/spinner.rs` | 41 | 100% | None |
| `tui/log_overlay.rs` | 63 | 100% | None |
| `tui/detail_overlay.rs` | 64 | 100% | None |
| `tui/confirm_overlay.rs` | 51 | 100% | None |
| `tui/header.rs` | 84 | 90% | Parameterize version string |
| `tui/startup.rs` | 263 | 80% | Parameterize art, phase labels, executor calls |
| `gh/poller.rs` | 224 | 90% | Replace direct calls with trait objects |

### Platform-Specific (stays in each binary, ~20%)

| Module | Lines | What changes |
|--------|-------|-------------|
| `gh/executor.rs` | 315 | Wraps as `impl CiExecutor for GhExecutor` |
| `gh/parser.rs` | 354 | Wraps as `impl CiParser for GhParser` |
| `cli.rs` | 92 | Different flags per platform |
| `main.rs` | 898 | Uses trait objects instead of direct `gh::` calls |

### Coupling Points in main.rs

These are the exact lines where `main.rs` calls `gh::executor` or `gh::parser` directly — each must be routed through trait objects:

| Line | Current call | Trait method |
|------|-------------|-------------|
| 268 | `gh::executor::fetch_runs(&repo2, limit, ...)` | `executor.fetch_runs(limit, ...)` |
| 269 | `gh::parser::parse_runs(&json)` | `parser.parse_runs(&json)` |
| 313 | `gh::executor::rerun_workflow(&repo2, run_id)` | `executor.rerun_failed(run_id)` |
| 377 | `gh::executor::cancel_run(&repo2, run_id)` | `executor.cancel_run(run_id)` |
| 405 | `gh::executor::delete_run(&repo2, run_id)` | `executor.delete_run(run_id)` |
| 434 | `gh::executor::open_in_browser(url)` | `executor.open_in_browser(url)` |
| 498 | `gh::executor::copy_to_clipboard(&text)` | `executor.copy_to_clipboard(&text)` |
| 865 | `gh::executor::fetch_failed_logs_for_job(...)` | `executor.fetch_failed_logs_for_job(...)` |
| 867 | `gh::executor::fetch_failed_logs(...)` | `executor.fetch_failed_logs(...)` |
| 872 | `gh::parser::process_log_output(...)` | `parser.process_log_output(...)` |

### Coupling Points in poller.rs

| Line | Current call | Trait method |
|------|-------------|-------------|
| 86 | `executor::fetch_runs(&self.repo, ...)` | `self.executor.fetch_runs(...)` |
| 87 | `parser::parse_runs(&json)` | `self.parser.parse_runs(&json)` |
| 129 | `executor::fetch_jobs(repo, run_id)` | `executor.fetch_jobs(run_id)` |
| 130 | `parser::parse_jobs(&json)` | `parser.parse_jobs(&json)` |
| 151 | `executor::fetch_jobs(repo, run_id)` | `executor.fetch_jobs(run_id)` (retry) |

### Coupling Points in startup.rs

| Area | Current code | Change |
|------|-------------|--------|
| Line 15-28 | Hardcoded `GHW_ART` | Accept from `PlatformConfig` |
| Phase labels | "Checking GitHub CLI..." | Accept from `PlatformConfig` |
| Phase 1 | `gh::executor::check_gh_available()` | `executor.check_available()` |
| Phase 2 | `gh::executor::detect_repo()` | `executor.detect_repo()` |
| Phase 3 | `gh::executor::detect_branch()` | `executor.detect_branch()` |
| Phase 4 | `gh::executor::fetch_runs()` / `gh::parser::parse_runs()` | `executor.fetch_runs()` / `parser.parse_runs()` |
