//! Background polling loop with adaptive interval and exponential backoff.
//!
//! The polling interval is controlled externally via a `watch::Receiver<u64>` — the
//! main event loop writes new intervals when activity changes (3s active → 10s recent
//! → 30s idle), and the poller picks up the change on its next `tokio::select!` cycle
//! without needing to be restarted.
//!
//! On consecutive failures, exponential backoff (`base × 2^failures`) is applied up
//! to `MAX_BACKOFF_SECS` (5 minutes). The backoff resets to the base interval on the next
//! successful poll.

use crate::events::AppEvent;
use crate::traits::{CiExecutor, CiParser};
use std::sync::Arc;
use tokio::sync::{mpsc, watch};
use tokio::time;

/// 5 minutes — the longest a user should wait between automatic retries. Beyond this,
/// the error toast has been visible long enough for them to investigate manually.
const MAX_BACKOFF_SECS: u64 = 300;

pub struct Poller {
    executor: Arc<dyn CiExecutor>,
    parser: Arc<dyn CiParser>,
    limit: usize,
    filter: Option<String>,
    tx: mpsc::UnboundedSender<AppEvent>,
    interval_rx: watch::Receiver<u64>,
}

/// Compute backoff delay: `min(base_interval * 2^failures, MAX_BACKOFF_SECS)`.
pub fn backoff_delay(base_interval: u64, failures: u32) -> u64 {
    let multiplier = 1u64.checked_shl(failures).unwrap_or(u64::MAX);
    base_interval
        .saturating_mul(multiplier)
        .clamp(1, MAX_BACKOFF_SECS)
}

impl Poller {
    pub fn new(
        executor: Arc<dyn CiExecutor>,
        parser: Arc<dyn CiParser>,
        limit: usize,
        filter: Option<String>,
        tx: mpsc::UnboundedSender<AppEvent>,
        interval_rx: watch::Receiver<u64>,
    ) -> Self {
        Self {
            executor,
            parser,
            limit,
            filter,
            tx,
            interval_rx,
        }
    }

    pub async fn run(mut self) {
        let mut failures: u32 = 0;

        // Initial fetch
        match self.poll_once().await {
            PollOutcome::Success => failures = 0,
            PollOutcome::Failure => failures = failures.saturating_add(1),
            PollOutcome::ChannelClosed => return,
        }

        loop {
            let base_interval = *self.interval_rx.borrow();
            let delay = if failures > 0 {
                backoff_delay(base_interval, failures)
            } else {
                base_interval
            };
            // Wake early if the polling interval changes (e.g. idle -> active)
            tokio::select! {
                () = time::sleep(time::Duration::from_secs(delay)) => {},
                _ = self.interval_rx.changed() => {},
            }

            match self.poll_once().await {
                PollOutcome::Success => failures = 0,
                PollOutcome::Failure => {
                    failures = failures.saturating_add(1);
                    let next_delay = backoff_delay(base_interval, failures);
                    if self
                        .tx
                        .send(AppEvent::Error(format!(
                            "Poll failed, retrying in {next_delay}s"
                        )))
                        .is_err()
                    {
                        return; // Receiver dropped
                    }
                }
                PollOutcome::ChannelClosed => return,
            }
        }
    }

    /// Returns the outcome of a single poll attempt.
    async fn poll_once(&self) -> PollOutcome {
        match self
            .executor
            .fetch_runs(self.limit, self.filter.as_deref())
            .await
        {
            Ok(json) => match self.parser.parse_runs(&json) {
                Ok(runs) => {
                    if self
                        .tx
                        .send(AppEvent::PollResult {
                            runs,
                            manual: false,
                        })
                        .is_err()
                    {
                        return PollOutcome::ChannelClosed;
                    }
                    PollOutcome::Success
                }
                Err(e) => {
                    if self
                        .tx
                        .send(AppEvent::Error(format!("Parse error: {e}")))
                        .is_err()
                    {
                        return PollOutcome::ChannelClosed;
                    }
                    PollOutcome::Failure
                }
            },
            Err(e) => {
                if self.tx.send(AppEvent::Error(format!("{e}"))).is_err() {
                    return PollOutcome::ChannelClosed;
                }
                PollOutcome::Failure
            }
        }
    }
}

enum PollOutcome {
    Success,
    Failure,
    ChannelClosed,
}

pub async fn fetch_jobs_for_run(
    executor: &dyn CiExecutor,
    parser: &dyn CiParser,
    run_id: u64,
    tx: &mpsc::UnboundedSender<AppEvent>,
) {
    match executor.fetch_jobs(run_id).await {
        Ok(json) => match parser.parse_jobs(&json) {
            Ok(jobs) => {
                if tx.send(AppEvent::JobsResult { run_id, jobs }).is_err() {
                    tracing::warn!("fetch_jobs: channel closed");
                }
            }
            Err(e) => {
                if tx
                    .send(AppEvent::RunError {
                        run_id,
                        error: format!("Job parse error: {e}"),
                    })
                    .is_err()
                {
                    tracing::warn!("fetch_jobs: channel closed");
                }
            }
        },
        Err(first_err) => {
            // Single retry after 2s for transient network failures
            time::sleep(time::Duration::from_secs(2)).await;
            match executor.fetch_jobs(run_id).await {
                Ok(json) => match parser.parse_jobs(&json) {
                    Ok(jobs) => {
                        if tx.send(AppEvent::JobsResult { run_id, jobs }).is_err() {
                            tracing::warn!("fetch_jobs: channel closed");
                        }
                    }
                    Err(e) => {
                        if tx
                            .send(AppEvent::RunError {
                                run_id,
                                error: format!("Job parse error: {e}"),
                            })
                            .is_err()
                        {
                            tracing::warn!("fetch_jobs: channel closed");
                        }
                    }
                },
                Err(retry_err) => {
                    if tx
                        .send(AppEvent::RunError {
                            run_id,
                            error: format!("{first_err} (retry also failed: {retry_err})"),
                        })
                        .is_err()
                    {
                        tracing::warn!("fetch_jobs: channel closed");
                    }
                }
            }
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
    fn backoff_base_zero_floors_to_one() {
        assert_eq!(backoff_delay(0, 5), 1);
    }
}
