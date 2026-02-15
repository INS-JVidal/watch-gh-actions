use crate::app::{AppState, Notification, RunStatus, WorkflowRun};
use std::collections::HashMap;

pub fn detect_changes(state: &mut AppState, new_runs: &[WorkflowRun]) {
    let now = std::time::Instant::now();

    for run in new_runs {
        if let Some(&(old_status, old_conclusion)) = state.previous_snapshot.get(&run.database_id) {
            if old_status != run.status || old_conclusion != run.conclusion {
                let msg = match (run.status, run.conclusion) {
                    (RunStatus::Completed, Some(crate::app::Conclusion::Success)) => {
                        format!("{} completed successfully", run.display_title)
                    }
                    (RunStatus::Completed, Some(crate::app::Conclusion::Failure)) => {
                        format!("{} failed", run.display_title)
                    }
                    (RunStatus::Completed, Some(c)) => {
                        format!("{} completed ({:?})", run.display_title, c)
                    }
                    (RunStatus::InProgress, _) => {
                        format!("{} started", run.display_title)
                    }
                    _ => {
                        format!("{} changed to {:?}", run.display_title, run.status)
                    }
                };
                state.notifications.push(Notification {
                    run_id: run.database_id,
                    message: msg,
                    timestamp: now,
                });
            }
        }
    }

    // Update snapshot
    let mut new_snapshot = HashMap::new();
    for run in new_runs {
        new_snapshot.insert(run.database_id, (run.status, run.conclusion));
    }
    state.previous_snapshot = new_snapshot;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Conclusion, RunStatus};
    use chrono::Utc;

    fn make_run(id: u64, status: RunStatus, conclusion: Option<Conclusion>) -> WorkflowRun {
        WorkflowRun {
            database_id: id,
            display_title: format!("Run {}", id),
            name: "CI".to_string(),
            head_branch: "main".to_string(),
            status,
            conclusion,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            event: "push".to_string(),
            number: id,
            url: format!("https://github.com/test/repo/actions/runs/{}", id),
            jobs: Vec::new(),
            jobs_fetched: false,
        }
    }

    fn make_state() -> AppState {
        AppState::new("test/repo".to_string(), Some("main".to_string()), 20, None)
    }

    #[test]
    fn first_poll_no_notifications() {
        let mut state = make_state();
        let runs = vec![make_run(1, RunStatus::InProgress, None)];
        detect_changes(&mut state, &runs);
        assert!(state.notifications.is_empty());
    }

    #[test]
    fn no_change_no_notifications() {
        let mut state = make_state();
        let runs = vec![make_run(1, RunStatus::InProgress, None)];
        detect_changes(&mut state, &runs);
        // Second poll same state
        let runs2 = vec![make_run(1, RunStatus::InProgress, None)];
        detect_changes(&mut state, &runs2);
        assert!(state.notifications.is_empty());
    }

    #[test]
    fn in_progress_to_completed_success() {
        let mut state = make_state();
        let runs1 = vec![make_run(1, RunStatus::InProgress, None)];
        detect_changes(&mut state, &runs1);

        let runs2 = vec![make_run(1, RunStatus::Completed, Some(Conclusion::Success))];
        detect_changes(&mut state, &runs2);
        assert_eq!(state.notifications.len(), 1);
        assert!(state.notifications[0]
            .message
            .contains("completed successfully"));
    }

    #[test]
    fn in_progress_to_completed_failure() {
        let mut state = make_state();
        let runs1 = vec![make_run(1, RunStatus::InProgress, None)];
        detect_changes(&mut state, &runs1);

        let runs2 = vec![make_run(1, RunStatus::Completed, Some(Conclusion::Failure))];
        detect_changes(&mut state, &runs2);
        assert_eq!(state.notifications.len(), 1);
        assert!(state.notifications[0].message.contains("failed"));
    }

    #[test]
    fn in_progress_to_completed_cancelled() {
        let mut state = make_state();
        let runs1 = vec![make_run(1, RunStatus::InProgress, None)];
        detect_changes(&mut state, &runs1);

        let runs2 = vec![make_run(
            1,
            RunStatus::Completed,
            Some(Conclusion::Cancelled),
        )];
        detect_changes(&mut state, &runs2);
        assert_eq!(state.notifications.len(), 1);
        assert!(state.notifications[0].message.contains("Cancelled"));
    }

    #[test]
    fn queued_to_in_progress_started() {
        let mut state = make_state();
        let runs1 = vec![make_run(1, RunStatus::Queued, None)];
        detect_changes(&mut state, &runs1);

        let runs2 = vec![make_run(1, RunStatus::InProgress, None)];
        detect_changes(&mut state, &runs2);
        assert_eq!(state.notifications.len(), 1);
        assert!(state.notifications[0].message.contains("started"));
    }

    #[test]
    fn queued_to_waiting_changed_to() {
        let mut state = make_state();
        let runs1 = vec![make_run(1, RunStatus::Queued, None)];
        detect_changes(&mut state, &runs1);

        let runs2 = vec![make_run(1, RunStatus::Waiting, None)];
        detect_changes(&mut state, &runs2);
        assert_eq!(state.notifications.len(), 1);
        assert!(state.notifications[0].message.contains("changed to"));
    }

    #[test]
    fn multiple_runs_multiple_notifications() {
        let mut state = make_state();
        let runs1 = vec![
            make_run(1, RunStatus::InProgress, None),
            make_run(2, RunStatus::InProgress, None),
        ];
        detect_changes(&mut state, &runs1);

        let runs2 = vec![
            make_run(1, RunStatus::Completed, Some(Conclusion::Success)),
            make_run(2, RunStatus::Completed, Some(Conclusion::Failure)),
        ];
        detect_changes(&mut state, &runs2);
        assert_eq!(state.notifications.len(), 2);
    }

    #[test]
    fn new_run_appearing_no_notification() {
        let mut state = make_state();
        let runs1 = vec![make_run(1, RunStatus::InProgress, None)];
        detect_changes(&mut state, &runs1);

        // Run 2 is new - not in previous snapshot
        let runs2 = vec![
            make_run(1, RunStatus::InProgress, None),
            make_run(2, RunStatus::Queued, None),
        ];
        detect_changes(&mut state, &runs2);
        assert!(state.notifications.is_empty());
    }

    #[test]
    fn snapshot_updated_after_detect() {
        let mut state = make_state();
        let runs1 = vec![make_run(1, RunStatus::InProgress, None)];
        detect_changes(&mut state, &runs1);
        assert!(state.previous_snapshot.contains_key(&1));
        assert_eq!(state.previous_snapshot[&1].0, RunStatus::InProgress);
    }

    #[test]
    fn snapshot_replaces_old_runs() {
        let mut state = make_state();
        let runs1 = vec![make_run(1, RunStatus::InProgress, None)];
        detect_changes(&mut state, &runs1);

        // Run 1 disappears, run 2 appears
        let runs2 = vec![make_run(2, RunStatus::Queued, None)];
        detect_changes(&mut state, &runs2);
        assert!(!state.previous_snapshot.contains_key(&1));
        assert!(state.previous_snapshot.contains_key(&2));
    }

    #[test]
    fn notification_contains_correct_run_id() {
        let mut state = make_state();
        let runs1 = vec![make_run(42, RunStatus::InProgress, None)];
        detect_changes(&mut state, &runs1);

        let runs2 = vec![make_run(
            42,
            RunStatus::Completed,
            Some(Conclusion::Success),
        )];
        detect_changes(&mut state, &runs2);
        assert_eq!(state.notifications[0].run_id, 42);
    }
}
