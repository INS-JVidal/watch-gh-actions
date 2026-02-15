use crate::events::AppEvent;
use crate::gh::{executor, parser};
use tokio::sync::{mpsc, watch};
use tokio::time;

const MAX_BACKOFF_SECS: u64 = 300;

pub struct Poller {
    repo: String,
    limit: usize,
    workflow: Option<String>,
    tx: mpsc::UnboundedSender<AppEvent>,
    interval_rx: watch::Receiver<u64>,
}

/// Compute backoff delay: `min(base_interval * 2^failures, MAX_BACKOFF_SECS)`.
pub fn backoff_delay(base_interval: u64, failures: u32) -> u64 {
    let multiplier = 1u64.checked_shl(failures).unwrap_or(u64::MAX);
    base_interval
        .saturating_mul(multiplier)
        .min(MAX_BACKOFF_SECS)
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
        let mut failures: u32 = 0;

        // Initial fetch
        if self.poll_once().await {
            failures = 0;
        } else {
            failures = failures.saturating_add(1);
        }

        loop {
            let base_interval = *self.interval_rx.borrow();
            let delay = if failures > 0 {
                backoff_delay(base_interval, failures)
            } else {
                base_interval
            };
            time::sleep(time::Duration::from_secs(delay)).await;

            if self.poll_once().await {
                failures = 0;
            } else {
                failures = failures.saturating_add(1);
                let next_delay = backoff_delay(base_interval, failures);
                let _ = self.tx.send(AppEvent::Error(format!(
                    "Poll failed, retrying in {next_delay}s"
                )));
            }
        }
    }

    /// Returns `true` on success, `false` on failure.
    async fn poll_once(&self) -> bool {
        match executor::fetch_runs(&self.repo, self.limit, self.workflow.as_deref()).await {
            Ok(json) => match parser::parse_runs(&json) {
                Ok(runs) => {
                    let _ = self.tx.send(AppEvent::PollResult(runs));
                    true
                }
                Err(e) => {
                    let _ = self.tx.send(AppEvent::Error(format!("Parse error: {e}")));
                    false
                }
            },
            Err(e) => {
                let _ = self.tx.send(AppEvent::Error(format!("{e}")));
                false
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
                let _ = tx.send(AppEvent::RunError {
                    run_id,
                    error: format!("Job parse error: {e}"),
                });
            }
        },
        Err(e) => {
            let _ = tx.send(AppEvent::RunError {
                run_id,
                error: format!("{e}"),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_zero_failures_returns_base() {
        assert_eq!(backoff_delay(10, 0), 10);
    }

    #[test]
    fn backoff_one_failure_doubles() {
        assert_eq!(backoff_delay(10, 1), 20);
    }

    #[test]
    fn backoff_two_failures_quadruples() {
        assert_eq!(backoff_delay(10, 2), 40);
    }

    #[test]
    fn backoff_capped_at_max() {
        assert_eq!(backoff_delay(10, 10), MAX_BACKOFF_SECS);
    }

    #[test]
    fn backoff_large_failure_count_saturates() {
        assert_eq!(backoff_delay(10, 100), MAX_BACKOFF_SECS);
    }

    #[test]
    fn backoff_base_one() {
        assert_eq!(backoff_delay(1, 5), 32);
    }

    #[test]
    fn backoff_base_zero_stays_zero() {
        assert_eq!(backoff_delay(0, 5), 0);
    }
}
