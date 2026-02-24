//! Platform abstraction traits for CI systems (GitHub Actions / GitLab CI).
//!
//! The library only sees `Arc<dyn CiExecutor>` / `Arc<dyn CiParser>` — concrete
//! implementations live in each binary crate.

use crate::app::{Job, WorkflowRun};
use async_trait::async_trait;
use color_eyre::eyre::Result;

/// Side-effecting operations against a CI platform's CLI tool.
///
/// Fetch methods return raw JSON strings — parsing stays in [`CiParser`] so the
/// two can be tested and validated independently. Errors are displayed directly
/// in the TUI toast, so implementors should return actionable messages.
#[async_trait]
pub trait CiExecutor: Send + Sync {
    async fn check_available(&self) -> Result<()>;
    async fn detect_repo(&self) -> Result<String>;
    async fn detect_branch(&self) -> Result<String>;
    /// Returns raw JSON passed to [`CiParser::parse_runs`]. `filter` is a workflow
    /// name (GitHub) or pipeline source (GitLab).
    async fn fetch_runs(&self, limit: usize, filter: Option<&str>) -> Result<String>;
    /// Returns raw JSON passed to [`CiParser::parse_jobs`].
    async fn fetch_jobs(&self, run_id: u64) -> Result<String>;
    async fn cancel_run(&self, run_id: u64) -> Result<()>;
    async fn delete_run(&self, run_id: u64) -> Result<()>;
    async fn rerun_failed(&self, run_id: u64) -> Result<()>;
    async fn fetch_failed_logs(&self, run_id: u64) -> Result<String>;
    /// Separate from `fetch_failed_logs` because GitLab can't filter logs server-side;
    /// it must fetch each job's trace individually by ID.
    async fn fetch_failed_logs_for_job(&self, run_id: u64, job_id: u64) -> Result<String>;
    /// Sync (not async) because it only spawns a detached child process — no waiting.
    fn open_in_browser(&self, url: &str) -> Result<()>;
    async fn copy_to_clipboard(&self, text: &str) -> Result<()>;
}

/// Deserializes platform-specific JSON into the shared data model.
/// Sync — parsing is CPU-bound, no I/O.
pub trait CiParser: Send + Sync {
    fn parse_runs(&self, json: &str) -> Result<Vec<WorkflowRun>>;
    fn parse_jobs(&self, json: &str) -> Result<Vec<Job>>;
    /// Truncate raw log output to at most `max_lines`, keeping the **tail** (most
    /// relevant for debugging). Returns `(processed_text, was_truncated)`.
    fn process_log_output(&self, raw: &str, max_lines: usize) -> (String, bool);
}
