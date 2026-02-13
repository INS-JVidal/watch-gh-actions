use crate::events::AppEvent;
use crate::gh::{executor, parser};
use tokio::sync::{mpsc, watch};
use tokio::time;

pub struct Poller {
    repo: String,
    limit: usize,
    workflow: Option<String>,
    tx: mpsc::UnboundedSender<AppEvent>,
    interval_rx: watch::Receiver<u64>,
}

impl Poller {
    pub fn new(
        repo: String,
        limit: usize,
        workflow: Option<String>,
        tx: mpsc::UnboundedSender<AppEvent>,
        interval_rx: watch::Receiver<u64>,
    ) -> Self {
        Self {
            repo,
            limit,
            workflow,
            tx,
            interval_rx,
        }
    }

    pub async fn run(self) {
        // Initial fetch
        self.poll_once().await;

        loop {
            let interval = *self.interval_rx.borrow();
            time::sleep(time::Duration::from_secs(interval)).await;
            self.poll_once().await;
        }
    }

    async fn poll_once(&self) {
        match executor::fetch_runs(&self.repo, self.limit, self.workflow.as_deref()).await {
            Ok(json) => match parser::parse_runs(&json) {
                Ok(runs) => {
                    let _ = self.tx.send(AppEvent::PollResult(runs));
                }
                Err(e) => {
                    let _ = self.tx.send(AppEvent::Error(format!("Parse error: {}", e)));
                }
            },
            Err(e) => {
                let _ = self.tx.send(AppEvent::Error(format!("{}", e)));
            }
        }
    }
}

pub async fn fetch_jobs_for_run(repo: &str, run_id: u64, tx: &mpsc::UnboundedSender<AppEvent>) {
    match executor::fetch_jobs(repo, run_id).await {
        Ok(json) => match parser::parse_jobs(&json) {
            Ok(jobs) => {
                let _ = tx.send(AppEvent::JobsResult { run_id, jobs });
            }
            Err(e) => {
                let _ = tx.send(AppEvent::Error(format!("Job parse error: {}", e)));
            }
        },
        Err(e) => {
            let _ = tx.send(AppEvent::Error(format!("{}", e)));
        }
    }
}
