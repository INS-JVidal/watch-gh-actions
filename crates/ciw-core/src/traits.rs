use crate::app::{Job, WorkflowRun};
use async_trait::async_trait;
use color_eyre::eyre::Result;

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

pub trait CiParser: Send + Sync {
    fn parse_runs(&self, json: &str) -> Result<Vec<WorkflowRun>>;
    fn parse_jobs(&self, json: &str) -> Result<Vec<Job>>;
    fn process_log_output(&self, raw: &str, max_lines: usize) -> (String, bool);
}
