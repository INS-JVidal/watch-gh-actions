# Pre-Implementation Verification Report

## Summary: 3 issues to fix, 0 blockers

Coherence check performed by 3 exploration agents against actual source code (5,679 lines across 22 files).

---

## Verified Correct

- **13/15 TUI/core files** are truly verbatim-movable (zero `gh::` imports): app.rs, events.rs, input.rs, diff.rs, notify.rs, tui/mod.rs, render.rs, tree.rs, footer.rs, spinner.rs, log_overlay.rs, detail_overlay.rs, confirm_overlay.rs
- **startup.rs** has `use crate::gh;` + 5 direct gh::executor/parser calls — plan correctly accounts for parameterization
- **header.rs** has hardcoded `"ghw v..."` — plan correctly accounts for parameterization
- **Only 3 files** import from `gh::`: main.rs, startup.rs, poller.rs — clean separation
- **All app.rs types** (`WorkflowRun`, `Job`, `Step`, `RunStatus`, `Conclusion`, `AppState`, `AppConfig`, etc.) are `pub` and platform-agnostic
- **No GitHub URLs** are constructed inline in main.rs — only uses pre-fetched `url` fields from parsed data
- **`open_in_browser`** is sync, **`copy_to_clipboard`** is async — both are OS-level, not GitHub-specific

---

## Issues Found & Amendments

### Issue 1: Function Rename Mapping (Worker 1 must know)

| Actual function in executor.rs | Trait method name | Other changes |
|---|---|---|
| `check_gh_available()` | `check_available()` | — |
| `rerun_workflow(repo, run_id)` | `rerun_failed(run_id)` | Name AND params change |
| `fetch_runs(repo, limit, workflow)` | `fetch_runs(limit, workflow_or_source)` | `repo` → `self.repo` |
| `fetch_jobs(repo, run_id)` | `fetch_jobs(run_id)` | `repo` → `self.repo` |
| `cancel_run(repo, run_id)` | `cancel_run(run_id)` | `repo` → `self.repo` |
| `delete_run(repo, run_id)` | `delete_run(run_id)` | `repo` → `self.repo` |
| `fetch_failed_logs(repo, run_id)` | `fetch_failed_logs(run_id)` | `repo` → `self.repo` |
| `fetch_failed_logs_for_job(repo, run_id, job_id)` | `fetch_failed_logs_for_job(run_id, job_id)` | `repo` → `self.repo` |

### Issue 2: `build_detail_lines` has GitHub-specific labels

In `main.rs:744-851`, `build_detail_lines` uses:
- `"Workflow"` (line 761)
- `"Event"` (line 763)
- `"Run"` (line 812)

**Decision**: Keep `build_detail_lines` in each binary's main.rs (not ciw-core). glw uses "Source", "Source", "Pipeline" respectively. Simpler than parameterizing.

### Issue 3: `fetch_jobs_for_run` called from main.rs too

`poller::fetch_jobs_for_run` is called at main.rs lines 253 and 664 (not just from poller internally). These call sites must be updated to pass `Arc<dyn CiExecutor>` and `Arc<dyn CiParser>` instead of `&repo`.

---

## Coupling Points (Complete & Verified)

### main.rs → gh::executor (8 calls)
| Line | Call |
|------|------|
| 268 | `gh::executor::fetch_runs(&repo2, limit, wf.as_deref())` |
| 313 | `gh::executor::rerun_workflow(&repo2, run_id)` |
| 377 | `gh::executor::cancel_run(&repo2, run_id)` |
| 405 | `gh::executor::delete_run(&repo2, run_id)` |
| 434 | `gh::executor::open_in_browser(url)` |
| 498 | `gh::executor::copy_to_clipboard(&text)` |
| 865 | `gh::executor::fetch_failed_logs_for_job(&repo, run_id, jid)` |
| 867 | `gh::executor::fetch_failed_logs(&repo, run_id)` |

### main.rs → gh::parser (2 calls)
| Line | Call |
|------|------|
| 269 | `gh::parser::parse_runs(&json)` |
| 872 | `gh::parser::process_log_output(&raw, app::LOG_MAX_LINES)` |

### main.rs → gh::poller (3 references)
| Line | Reference |
|------|-----------|
| 156-162 | `Poller::new(repo, limit, workflow, tx, interval_rx)` |
| 253 | `poller::fetch_jobs_for_run(&repo2, run_id, &tx2)` |
| 664 | `poller::fetch_jobs_for_run(&repo2, run_id, &tx2)` |

### startup.rs → gh:: (5 calls)
| Line | Call |
|------|------|
| 184 | `gh::executor::check_gh_available()` |
| 203 | `gh::executor::detect_repo()` |
| 230 | `gh::executor::detect_branch()` |
| 253 | `gh::executor::fetch_runs(&repo, args.limit, args.workflow.as_deref())` |
| 257 | `gh::parser::parse_runs(&json)` |

### poller.rs → gh:: (4 calls)
| Line | Call |
|------|------|
| 86 | `executor::fetch_runs(&self.repo, self.limit, self.workflow.as_deref())` |
| 87 | `parser::parse_runs(&json)` |
| 129/151 | `executor::fetch_jobs(repo, run_id)` |
| 130/152 | `parser::parse_jobs(&json)` |
