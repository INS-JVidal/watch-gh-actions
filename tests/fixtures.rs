#![allow(dead_code)]

use chrono::Utc;
use ghw::app::{AppState, Conclusion, Job, RunStatus, Step, WorkflowRun};

pub fn default_run() -> WorkflowRun {
    run_with_id(1)
}

pub fn run_with_id(id: u64) -> WorkflowRun {
    WorkflowRun {
        database_id: id,
        display_title: format!("CI Build #{}", id),
        name: "CI".to_string(),
        head_branch: "main".to_string(),
        status: RunStatus::Completed,
        conclusion: Some(Conclusion::Success),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        event: "push".to_string(),
        number: id,
        url: format!("https://github.com/test/repo/actions/runs/{}", id),
        jobs: None,
    }
}

pub fn run_in_progress(id: u64) -> WorkflowRun {
    let mut run = run_with_id(id);
    run.status = RunStatus::InProgress;
    run.conclusion = None;
    run
}

pub fn run_failed(id: u64) -> WorkflowRun {
    let mut run = run_with_id(id);
    run.status = RunStatus::Completed;
    run.conclusion = Some(Conclusion::Failure);
    run
}

pub fn run_on_branch(id: u64, branch: &str) -> WorkflowRun {
    let mut run = run_with_id(id);
    run.head_branch = branch.to_string();
    run
}

pub fn default_job() -> Job {
    Job {
        database_id: Some(1),
        name: "build".to_string(),
        status: RunStatus::Completed,
        conclusion: Some(Conclusion::Success),
        started_at: Some(Utc::now()),
        completed_at: Some(Utc::now()),
        url: "https://github.com/test/repo/actions/runs/1/jobs/1".to_string(),
        steps: vec![default_step()],
    }
}

pub fn default_step() -> Step {
    Step {
        name: "Checkout".to_string(),
        status: RunStatus::Completed,
        conclusion: Some(Conclusion::Success),
        number: 1,
        started_at: None,
        completed_at: None,
    }
}

pub fn make_state_with_runs(runs: Vec<WorkflowRun>) -> AppState {
    let mut state = AppState::new("test/repo".to_string(), Some("main".to_string()), 20, None);
    state.runs = runs;
    state.rebuild_tree();
    state
}
