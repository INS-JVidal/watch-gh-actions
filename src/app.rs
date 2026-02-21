use chrono::{DateTime, Utc};
use std::collections::HashMap;

// Polling intervals (seconds)
pub const POLL_INTERVAL_ACTIVE: u64 = 3;
pub const POLL_INTERVAL_RECENT: u64 = 10;
pub const POLL_INTERVAL_IDLE: u64 = 30;
pub const POLL_RECENT_THRESHOLD_SECS: u64 = 60;

// UI constants
pub const NOTIFICATION_TTL_SECS: u64 = 5;
pub const SPINNER_FRAME_COUNT: usize = 10;
pub const QUICK_SELECT_MAX: usize = 9;
pub const NARROW_WIDTH_THRESHOLD: u16 = 60;
pub const ERROR_TTL_SECS: u64 = 10;

// Log overlay constants
pub const LOG_MAX_LINES: usize = 500;
pub const LOG_CACHE_TTL_SECS: u64 = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Completed,
    InProgress,
    Queued,
    Requested,
    Waiting,
    Pending,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Conclusion {
    Success,
    Failure,
    Cancelled,
    Skipped,
    TimedOut,
    ActionRequired,
    StartupFailure,
    Stale,
    Neutral,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // Fields are part of the GitHub API contract
pub struct WorkflowRun {
    pub database_id: u64,
    pub display_title: String,
    pub name: String,
    pub head_branch: String,
    pub status: RunStatus,
    pub conclusion: Option<Conclusion>,
    #[serde(rename = "createdAt")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updatedAt")]
    pub updated_at: DateTime<Utc>,
    pub event: String,
    pub number: u64,
    pub url: String,
    #[serde(skip)]
    pub jobs: Vec<Job>,
    #[serde(skip)]
    pub jobs_fetched: bool,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Job {
    #[serde(default)]
    pub database_id: Option<u64>,
    pub name: String,
    pub status: RunStatus,
    pub conclusion: Option<Conclusion>,
    #[serde(rename = "startedAt")]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(rename = "completedAt")]
    pub completed_at: Option<DateTime<Utc>>,
    pub url: String,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Step {
    pub name: String,
    pub status: RunStatus,
    pub conclusion: Option<Conclusion>,
    pub number: u64,
    #[serde(default, rename = "startedAt")]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default, rename = "completedAt")]
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeLevel {
    Run,
    Job,
    Step,
    Loading,
}

#[derive(Debug, Clone)]
pub struct TreeItem {
    pub level: TreeLevel,
    pub run_idx: usize,
    pub job_idx: Option<usize>,
    pub step_idx: Option<usize>,
    pub expanded: bool,
}

pub enum ResolvedItem<'a> {
    Run(&'a WorkflowRun),
    Job(&'a Job),
    Step(&'a Step),
}

impl AppState {
    pub fn resolve_item(&self, item: &TreeItem) -> Option<ResolvedItem<'_>> {
        let run = self.runs.get(item.run_idx)?;
        match item.level {
            TreeLevel::Run => Some(ResolvedItem::Run(run)),
            TreeLevel::Job => {
                let job = run.jobs.get(item.job_idx?)?;
                Some(ResolvedItem::Job(job))
            }
            TreeLevel::Step => {
                let job = run.jobs.get(item.job_idx?)?;
                let step = job.steps.get(item.step_idx?)?;
                Some(ResolvedItem::Step(step))
            }
            TreeLevel::Loading => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    All,
    ActiveOnly,
    CurrentBranch,
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub run_id: u64,
    pub message: String,
    pub timestamp: std::time::Instant,
}

pub struct FailedLog {
    pub content: String,
    pub fetched_at: std::time::Instant,
}

pub struct LogOverlay {
    pub title: String,
    pub lines: Vec<String>,
    pub scroll: usize,
    pub run_id: u64,
    pub job_id: Option<u64>,
}

pub struct DetailOverlay {
    pub title: String,
    pub lines: Vec<(String, String)>,
}

pub enum ActiveOverlay {
    None,
    Log(LogOverlay),
    Detail(DetailOverlay),
}

/// Immutable configuration set at startup.
pub struct AppConfig {
    pub repo: String,
    pub branch: Option<String>,
    pub limit: usize,
    pub workflow_filter: Option<String>,
}

pub struct AppState {
    pub config: AppConfig,

    // Run data
    pub runs: Vec<WorkflowRun>,
    pub previous_snapshot: HashMap<u64, (RunStatus, Option<Conclusion>, u64)>,
    pub poll_count: u64,

    // Tree navigation
    pub tree_items: Vec<TreeItem>,
    pub cursor: usize,
    pub expanded_runs: std::collections::HashSet<u64>,
    pub expanded_jobs: std::collections::HashSet<(u64, u64)>,
    pub filter: FilterMode,

    // Polling
    pub last_poll: Option<std::time::Instant>,
    pub next_poll_in: u64,
    pub poll_interval: u64,

    // Transient UI
    pub notifications: Vec<Notification>,
    pub error: Option<(String, std::time::Instant)>,
    pub spinner_frame: usize,
    pub loading_count: u16,
    pub should_quit: bool,

    // Log overlay cache
    pub log_cache: HashMap<(u64, Option<u64>), FailedLog>,

    // Active overlay (mutually exclusive)
    pub overlay: ActiveOverlay,

    // Per-run errors (e.g. job-fetch failures)
    pub run_errors: HashMap<u64, String>,

    // Desktop notifications
    pub desktop_notify: bool,
}

impl AppState {
    pub fn new(
        repo: String,
        branch: Option<String>,
        limit: usize,
        workflow_filter: Option<String>,
    ) -> Self {
        Self {
            config: AppConfig {
                repo,
                branch,
                limit,
                workflow_filter,
            },
            runs: Vec::new(),
            previous_snapshot: HashMap::new(),
            poll_count: 0,
            tree_items: Vec::new(),
            cursor: 0,
            expanded_runs: std::collections::HashSet::new(),
            expanded_jobs: std::collections::HashSet::new(),
            filter: FilterMode::All,
            last_poll: None,
            next_poll_in: 0,
            poll_interval: 10,
            notifications: Vec::new(),
            error: None,
            spinner_frame: 0,
            loading_count: 0,
            should_quit: false,
            log_cache: HashMap::new(),
            overlay: ActiveOverlay::None,
            run_errors: HashMap::new(),
            desktop_notify: true,
        }
    }

    pub fn rebuild_tree(&mut self) {
        let mut items = Vec::new();
        let filtered = self.filtered_runs_indices();
        for (run_idx, _run_ref) in &filtered {
            let run = &self.runs[*run_idx];
            let run_id = run.database_id;
            let run_expanded = self.expanded_runs.contains(&run_id);
            items.push(TreeItem {
                level: TreeLevel::Run,
                run_idx: *run_idx,
                job_idx: None,
                step_idx: None,

                expanded: run_expanded,
            });
            if run_expanded {
                if run.jobs_fetched {
                    for (job_idx, job) in run.jobs.iter().enumerate() {
                        let job_db_id = match job.database_id {
                            Some(id) => id,
                            None => continue,
                        };
                        let job_expanded = self.expanded_jobs.contains(&(run_id, job_db_id));
                        items.push(TreeItem {
                            level: TreeLevel::Job,
                            run_idx: *run_idx,
                            job_idx: Some(job_idx),
                            step_idx: None,
                            expanded: job_expanded,
                        });
                        if job_expanded {
                            for (step_idx, _step) in run.jobs[job_idx].steps.iter().enumerate() {
                                items.push(TreeItem {
                                    level: TreeLevel::Step,
                                    run_idx: *run_idx,
                                    job_idx: Some(job_idx),
                                    step_idx: Some(step_idx),
                                    expanded: false,
                                });
                            }
                        }
                    }
                } else {
                    items.push(TreeItem {
                        level: TreeLevel::Loading,
                        run_idx: *run_idx,
                        job_idx: None,
                        step_idx: None,
                        expanded: false,
                    });
                }
            }
        }
        self.tree_items = items;
        if self.cursor >= self.tree_items.len() && !self.tree_items.is_empty() {
            self.cursor = self.tree_items.len() - 1;
        } else if self.tree_items.is_empty() {
            self.cursor = 0;
        }
    }

    fn filter_predicate(&self, r: &WorkflowRun) -> bool {
        match self.filter {
            FilterMode::All => true,
            FilterMode::ActiveOnly => {
                matches!(
                    r.status,
                    RunStatus::InProgress
                        | RunStatus::Queued
                        | RunStatus::Waiting
                        | RunStatus::Pending
                        | RunStatus::Requested
                )
            }
            FilterMode::CurrentBranch => self
                .config
                .branch
                .as_ref()
                .is_some_and(|b| r.head_branch == *b),
        }
    }

    /// Returns (original_index_in_self.runs, &WorkflowRun) for filtered runs.
    pub fn filtered_runs_indices(&self) -> Vec<(usize, &WorkflowRun)> {
        self.runs
            .iter()
            .enumerate()
            .filter(|(_, r)| self.filter_predicate(r))
            .collect()
    }

    pub fn move_cursor_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_cursor_down(&mut self) {
        if !self.tree_items.is_empty() && self.cursor < self.tree_items.len() - 1 {
            self.cursor += 1;
        }
    }

    fn run_id_for(&self, run_idx: usize) -> Option<u64> {
        self.runs.get(run_idx).map(|r| r.database_id)
    }

    fn job_db_id_for(&self, run_idx: usize, job_idx: usize) -> Option<u64> {
        self.runs.get(run_idx)?.jobs.get(job_idx)?.database_id
    }

    pub fn toggle_expand(&mut self) {
        if let Some(item) = self.tree_items.get(self.cursor).cloned() {
            let Some(run_id) = self.run_id_for(item.run_idx) else {
                return;
            };
            match item.level {
                TreeLevel::Run => {
                    if self.expanded_runs.contains(&run_id) {
                        self.expanded_runs.remove(&run_id);
                        let keys: Vec<_> = self
                            .expanded_jobs
                            .iter()
                            .filter(|(r, _)| *r == run_id)
                            .copied()
                            .collect();
                        for k in keys {
                            self.expanded_jobs.remove(&k);
                        }
                    } else {
                        self.expanded_runs.insert(run_id);
                    }
                }
                TreeLevel::Job => {
                    if let Some(job_idx) = item.job_idx {
                        if let Some(job_db_id) = self.job_db_id_for(item.run_idx, job_idx) {
                            let key = (run_id, job_db_id);
                            if self.expanded_jobs.contains(&key) {
                                self.expanded_jobs.remove(&key);
                            } else {
                                self.expanded_jobs.insert(key);
                            }
                        }
                    }
                }
                TreeLevel::Step | TreeLevel::Loading => {}
            }
            self.rebuild_tree();
        }
    }

    /// Returns Some((run_idx_in_self.runs, needs_job_fetch)) if expanded a run.
    pub fn expand_current(&mut self) -> Option<(usize, bool)> {
        if let Some(item) = self.tree_items.get(self.cursor).cloned() {
            let run_id = self.run_id_for(item.run_idx)?;
            match item.level {
                TreeLevel::Run => {
                    if !self.expanded_runs.contains(&run_id) {
                        self.expanded_runs.insert(run_id);
                        self.rebuild_tree();
                        let needs_fetch =
                            self.runs.get(item.run_idx).is_some_and(|r| !r.jobs_fetched);
                        return Some((item.run_idx, needs_fetch));
                    }
                }
                TreeLevel::Job => {
                    if let Some(job_idx) = item.job_idx {
                        if let Some(job_db_id) = self.job_db_id_for(item.run_idx, job_idx) {
                            let key = (run_id, job_db_id);
                            if !self.expanded_jobs.contains(&key) {
                                self.expanded_jobs.insert(key);
                                self.rebuild_tree();
                            }
                        }
                    }
                }
                TreeLevel::Step | TreeLevel::Loading => {}
            }
        }
        None
    }

    pub fn collapse_current(&mut self) {
        if let Some(item) = self.tree_items.get(self.cursor).cloned() {
            let Some(run_id) = self.run_id_for(item.run_idx) else {
                return;
            };
            match item.level {
                TreeLevel::Run => {
                    self.expanded_runs.remove(&run_id);
                    let keys: Vec<_> = self
                        .expanded_jobs
                        .iter()
                        .filter(|(r, _)| *r == run_id)
                        .copied()
                        .collect();
                    for k in keys {
                        self.expanded_jobs.remove(&k);
                    }
                    self.rebuild_tree();
                }
                TreeLevel::Job => {
                    if let Some(job_idx) = item.job_idx {
                        if let Some(job_db_id) = self.job_db_id_for(item.run_idx, job_idx) {
                            let key = (run_id, job_db_id);
                            if self.expanded_jobs.contains(&key) {
                                self.expanded_jobs.remove(&key);
                                self.rebuild_tree();
                            } else {
                                // Go up to parent run
                                for (i, ti) in self.tree_items.iter().enumerate() {
                                    if ti.level == TreeLevel::Run && ti.run_idx == item.run_idx {
                                        self.cursor = i;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                TreeLevel::Step => {
                    // Go up to parent job
                    for (i, ti) in self.tree_items.iter().enumerate() {
                        if ti.level == TreeLevel::Job
                            && ti.run_idx == item.run_idx
                            && ti.job_idx == item.job_idx
                        {
                            self.cursor = i;
                            break;
                        }
                    }
                }
                TreeLevel::Loading => {}
            }
        }
    }

    pub fn current_run_url(&self) -> Option<&str> {
        self.tree_items
            .get(self.cursor)
            .and_then(|item| self.runs.get(item.run_idx).map(|r| r.url.as_str()))
    }

    pub fn current_run_id(&self) -> Option<u64> {
        self.tree_items
            .get(self.cursor)
            .and_then(|item| self.runs.get(item.run_idx).map(|r| r.database_id))
    }

    pub fn current_run_status(&self) -> Option<RunStatus> {
        self.tree_items
            .get(self.cursor)
            .and_then(|item| self.runs.get(item.run_idx).map(|r| r.status))
    }

    pub fn has_active_runs(&self) -> bool {
        self.runs.iter().any(|r| {
            matches!(
                r.status,
                RunStatus::InProgress
                    | RunStatus::Queued
                    | RunStatus::Waiting
                    | RunStatus::Pending
                    | RunStatus::Requested
            )
        })
    }

    pub fn quick_select(&mut self, n: usize) {
        // Select the nth visible run (1-indexed)
        let mut run_count = 0;
        for (i, item) in self.tree_items.iter().enumerate() {
            if item.level == TreeLevel::Run {
                run_count += 1;
                if run_count == n {
                    self.cursor = i;
                    break;
                }
            }
        }
    }

    pub fn cycle_filter(&mut self) {
        self.filter = match self.filter {
            FilterMode::All => FilterMode::ActiveOnly,
            FilterMode::ActiveOnly => FilterMode::CurrentBranch,
            FilterMode::CurrentBranch => FilterMode::All,
        };
        self.rebuild_tree();
    }

    pub fn prune_notifications(&mut self) {
        let now = std::time::Instant::now();
        self.notifications
            .retain(|n| now.duration_since(n.timestamp).as_secs() < NOTIFICATION_TTL_SECS);
    }

    pub fn is_loading(&self) -> bool {
        self.loading_count > 0
    }

    pub fn advance_spinner(&mut self) {
        self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAME_COUNT;
    }

    pub fn set_error(&mut self, msg: String) {
        self.error = Some((msg, std::time::Instant::now()));
    }

    pub fn clear_error(&mut self) {
        self.error = None;
    }

    pub fn prune_error(&mut self) {
        if let Some((_, ts)) = &self.error {
            if ts.elapsed().as_secs() >= ERROR_TTL_SECS {
                self.error = None;
            }
        }
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error.as_ref().map(|(msg, _)| msg.as_str())
    }

    // --- Log overlay methods ---

    pub fn has_log_overlay(&self) -> bool {
        matches!(self.overlay, ActiveOverlay::Log(_))
    }

    pub fn open_log_overlay(
        &mut self,
        title: String,
        content: &str,
        run_id: u64,
        job_id: Option<u64>,
    ) {
        let lines: Vec<String> = content
            .lines()
            .map(std::string::ToString::to_string)
            .collect();
        let lines = if lines.len() > LOG_MAX_LINES {
            lines[lines.len() - LOG_MAX_LINES..].to_vec()
        } else {
            lines
        };
        self.overlay = ActiveOverlay::Log(LogOverlay {
            title,
            lines,
            scroll: 0,
            run_id,
            job_id,
        });
    }

    pub fn close_log_overlay(&mut self) {
        if matches!(self.overlay, ActiveOverlay::Log(_)) {
            self.overlay = ActiveOverlay::None;
        }
    }

    pub fn scroll_log_up(&mut self, amount: usize) {
        if let ActiveOverlay::Log(ref mut overlay) = self.overlay {
            overlay.scroll = overlay.scroll.saturating_sub(amount);
        }
    }

    pub fn scroll_log_down(&mut self, amount: usize, visible_height: usize) {
        if let ActiveOverlay::Log(ref mut overlay) = self.overlay {
            let max_scroll = overlay.lines.len().saturating_sub(visible_height);
            overlay.scroll = (overlay.scroll + amount).min(max_scroll);
        }
    }

    pub fn scroll_log_to_top(&mut self) {
        if let ActiveOverlay::Log(ref mut overlay) = self.overlay {
            overlay.scroll = 0;
        }
    }

    pub fn scroll_log_to_bottom(&mut self, visible_height: usize) {
        if let ActiveOverlay::Log(ref mut overlay) = self.overlay {
            overlay.scroll = overlay.lines.len().saturating_sub(visible_height);
        }
    }

    // --- Detail overlay methods ---

    pub fn has_detail_overlay(&self) -> bool {
        matches!(self.overlay, ActiveOverlay::Detail(_))
    }

    pub fn open_detail_overlay(&mut self, title: String, lines: Vec<(String, String)>) {
        self.overlay = ActiveOverlay::Detail(DetailOverlay { title, lines });
    }

    pub fn close_detail_overlay(&mut self) {
        if matches!(self.overlay, ActiveOverlay::Detail(_)) {
            self.overlay = ActiveOverlay::None;
        }
    }

    pub fn current_item_ids(&self) -> Option<(u64, Option<u64>)> {
        let item = self.tree_items.get(self.cursor)?;
        let run = self.runs.get(item.run_idx)?;
        let run_id = run.database_id;
        match item.level {
            TreeLevel::Run => Some((run_id, None)),
            TreeLevel::Job | TreeLevel::Step => {
                let job = run.jobs.get(item.job_idx?)?;
                Some((run_id, job.database_id))
            }
            TreeLevel::Loading => None,
        }
    }

    pub fn current_item_is_failed(&self) -> bool {
        let Some(item) = self.tree_items.get(self.cursor) else {
            return false;
        };
        let Some(resolved) = self.resolve_item(item) else {
            return false;
        };
        match resolved {
            ResolvedItem::Run(r) => r.conclusion == Some(Conclusion::Failure),
            ResolvedItem::Job(j) => j.conclusion == Some(Conclusion::Failure),
            ResolvedItem::Step(s) => s.conclusion == Some(Conclusion::Failure),
        }
    }

    pub fn log_overlay_ref(&self) -> Option<&LogOverlay> {
        if let ActiveOverlay::Log(ref overlay) = self.overlay {
            Some(overlay)
        } else {
            None
        }
    }

    pub fn log_overlay_text(&self) -> Option<String> {
        self.log_overlay_ref().map(|o| o.lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn make_run_on_branch(id: u64, branch: &str) -> WorkflowRun {
        let mut run = make_run(id, RunStatus::Completed, Some(Conclusion::Success));
        run.head_branch = branch.to_string();
        run
    }

    fn make_job(name: &str, status: RunStatus, conclusion: Option<Conclusion>) -> Job {
        Job {
            database_id: Some(1),
            name: name.to_string(),
            status,
            conclusion,
            started_at: Some(Utc::now()),
            completed_at: Some(Utc::now()),
            url: "https://github.com/test/repo/actions/runs/1/jobs/1".to_string(),
            steps: vec![
                Step {
                    name: "Checkout".to_string(),
                    status: RunStatus::Completed,
                    conclusion: Some(Conclusion::Success),
                    number: 1,
                    started_at: None,
                    completed_at: None,
                },
                Step {
                    name: "Build".to_string(),
                    status: RunStatus::Completed,
                    conclusion: Some(Conclusion::Success),
                    number: 2,
                    started_at: None,
                    completed_at: None,
                },
            ],
        }
    }

    fn state_with_runs(runs: Vec<WorkflowRun>) -> AppState {
        let mut state = AppState::new("test/repo".to_string(), Some("main".to_string()), 20, None);
        state.runs = runs;
        state.rebuild_tree();
        state
    }

    // --- Cursor movement ---

    #[test]
    fn cursor_up_at_zero_stays() {
        let mut state = state_with_runs(vec![
            make_run(1, RunStatus::Completed, Some(Conclusion::Success)),
            make_run(2, RunStatus::Completed, Some(Conclusion::Success)),
        ]);
        assert_eq!(state.cursor, 0);
        state.move_cursor_up();
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn cursor_down_advances() {
        let mut state = state_with_runs(vec![
            make_run(1, RunStatus::Completed, Some(Conclusion::Success)),
            make_run(2, RunStatus::Completed, Some(Conclusion::Success)),
        ]);
        state.move_cursor_down();
        assert_eq!(state.cursor, 1);
    }

    #[test]
    fn cursor_down_at_end_stays() {
        let mut state = state_with_runs(vec![
            make_run(1, RunStatus::Completed, Some(Conclusion::Success)),
            make_run(2, RunStatus::Completed, Some(Conclusion::Success)),
        ]);
        state.move_cursor_down();
        state.move_cursor_down(); // at end
        assert_eq!(state.cursor, 1);
    }

    #[test]
    fn cursor_up_on_empty_state() {
        let mut state = state_with_runs(vec![]);
        state.move_cursor_up();
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn cursor_down_on_empty_state() {
        let mut state = state_with_runs(vec![]);
        state.move_cursor_down();
        assert_eq!(state.cursor, 0);
    }

    // --- Tree rebuild ---

    #[test]
    fn rebuild_creates_run_items() {
        let state = state_with_runs(vec![
            make_run(1, RunStatus::Completed, Some(Conclusion::Success)),
            make_run(2, RunStatus::InProgress, None),
        ]);
        assert_eq!(state.tree_items.len(), 2);
        assert_eq!(state.tree_items[0].level, TreeLevel::Run);
        assert_eq!(state.tree_items[1].level, TreeLevel::Run);
    }

    #[test]
    fn rebuild_empty_runs() {
        let state = state_with_runs(vec![]);
        assert!(state.tree_items.is_empty());
    }

    #[test]
    fn expanded_run_shows_jobs() {
        let mut run = make_run(1, RunStatus::Completed, Some(Conclusion::Success));
        run.jobs = vec![make_job(
            "build",
            RunStatus::Completed,
            Some(Conclusion::Success),
        )];
        run.jobs_fetched = true;
        let mut state = state_with_runs(vec![run]);
        state.expanded_runs.insert(1);
        state.rebuild_tree();
        assert_eq!(state.tree_items.len(), 2); // run + job
        assert_eq!(state.tree_items[1].level, TreeLevel::Job);
    }

    #[test]
    fn expanded_job_shows_steps() {
        let mut run = make_run(1, RunStatus::Completed, Some(Conclusion::Success));
        run.jobs = vec![make_job(
            "build",
            RunStatus::Completed,
            Some(Conclusion::Success),
        )];
        run.jobs_fetched = true;
        let mut state = state_with_runs(vec![run]);
        state.expanded_runs.insert(1);
        state.expanded_jobs.insert((1, 1));
        state.rebuild_tree();
        // run + job + 2 steps
        assert_eq!(state.tree_items.len(), 4);
        assert_eq!(state.tree_items[2].level, TreeLevel::Step);
        assert_eq!(state.tree_items[3].level, TreeLevel::Step);
    }

    // --- Expand/collapse/toggle ---

    #[test]
    fn expand_returns_needs_fetch() {
        let mut state = state_with_runs(vec![make_run(
            1,
            RunStatus::Completed,
            Some(Conclusion::Success),
        )]);
        let result = state.expand_current();
        assert_eq!(result, Some((0, true))); // needs_fetch because jobs_fetched is false
        assert!(state.expanded_runs.contains(&1));
    }

    #[test]
    fn expand_already_fetched_returns_no_fetch() {
        let mut run = make_run(1, RunStatus::Completed, Some(Conclusion::Success));
        run.jobs_fetched = true;
        let mut state = state_with_runs(vec![run]);
        let result = state.expand_current();
        assert_eq!(result, Some((0, false))); // no fetch needed
    }

    #[test]
    fn expand_already_expanded_returns_none() {
        let mut state = state_with_runs(vec![make_run(
            1,
            RunStatus::Completed,
            Some(Conclusion::Success),
        )]);
        state.expanded_runs.insert(1);
        state.rebuild_tree();
        let result = state.expand_current();
        assert_eq!(result, None);
    }

    #[test]
    fn collapse_removes_from_expanded() {
        let mut state = state_with_runs(vec![make_run(
            1,
            RunStatus::Completed,
            Some(Conclusion::Success),
        )]);
        state.expanded_runs.insert(1);
        state.rebuild_tree();
        state.collapse_current();
        assert!(!state.expanded_runs.contains(&1));
    }

    #[test]
    fn collapse_on_unexpanded_job_navigates_to_parent() {
        let mut run = make_run(1, RunStatus::Completed, Some(Conclusion::Success));
        run.jobs = vec![make_job(
            "build",
            RunStatus::Completed,
            Some(Conclusion::Success),
        )];
        run.jobs_fetched = true;
        let mut state = state_with_runs(vec![run]);
        state.expanded_runs.insert(1);
        state.rebuild_tree();
        state.cursor = 1; // on the job
        state.collapse_current();
        assert_eq!(state.cursor, 0); // navigated to parent run
    }

    #[test]
    fn toggle_expand_then_collapse() {
        let mut state = state_with_runs(vec![make_run(
            1,
            RunStatus::Completed,
            Some(Conclusion::Success),
        )]);
        state.toggle_expand();
        assert!(state.expanded_runs.contains(&1));
        state.toggle_expand();
        assert!(!state.expanded_runs.contains(&1));
    }

    #[test]
    fn collapse_run_cascades_to_child_jobs() {
        let mut run = make_run(1, RunStatus::Completed, Some(Conclusion::Success));
        run.jobs = vec![make_job(
            "build",
            RunStatus::Completed,
            Some(Conclusion::Success),
        )];
        run.jobs_fetched = true;
        let mut state = state_with_runs(vec![run]);
        state.expanded_runs.insert(1);
        state.expanded_jobs.insert((1, 1));
        state.rebuild_tree();
        state.cursor = 0; // on the run
        state.collapse_current();
        assert!(!state.expanded_runs.contains(&1));
        assert!(!state.expanded_jobs.contains(&(1, 1)));
    }

    // --- Filtering ---

    #[test]
    fn filter_all_shows_everything() {
        let state = state_with_runs(vec![
            make_run(1, RunStatus::Completed, Some(Conclusion::Success)),
            make_run(2, RunStatus::InProgress, None),
        ]);
        assert_eq!(state.tree_items.len(), 2);
    }

    #[test]
    fn filter_active_only_hides_completed() {
        let mut state = state_with_runs(vec![
            make_run(1, RunStatus::Completed, Some(Conclusion::Success)),
            make_run(2, RunStatus::InProgress, None),
        ]);
        state.filter = FilterMode::ActiveOnly;
        state.rebuild_tree();
        assert_eq!(state.tree_items.len(), 1);
        assert_eq!(state.tree_items[0].run_idx, 1);
    }

    #[test]
    fn filter_active_includes_all_active_statuses() {
        let mut state = state_with_runs(vec![
            make_run(1, RunStatus::InProgress, None),
            make_run(2, RunStatus::Queued, None),
            make_run(3, RunStatus::Waiting, None),
            make_run(4, RunStatus::Pending, None),
            make_run(5, RunStatus::Requested, None),
            make_run(6, RunStatus::Completed, Some(Conclusion::Success)),
        ]);
        state.filter = FilterMode::ActiveOnly;
        state.rebuild_tree();
        assert_eq!(state.tree_items.len(), 5);
    }

    #[test]
    fn filter_current_branch_shows_matching() {
        let mut state = state_with_runs(vec![
            make_run_on_branch(1, "main"),
            make_run_on_branch(2, "feature"),
        ]);
        state.filter = FilterMode::CurrentBranch;
        state.rebuild_tree();
        assert_eq!(state.tree_items.len(), 1);
        assert_eq!(state.tree_items[0].run_idx, 0);
    }

    #[test]
    fn filter_current_branch_with_no_branch_is_empty() {
        let mut state = AppState::new("test/repo".to_string(), None, 20, None);
        state.runs = vec![make_run_on_branch(1, "main")];
        state.filter = FilterMode::CurrentBranch;
        state.rebuild_tree();
        assert!(state.tree_items.is_empty());
    }

    #[test]
    fn cycle_filter_order() {
        let mut state = state_with_runs(vec![]);
        assert_eq!(state.filter, FilterMode::All);
        state.cycle_filter();
        assert_eq!(state.filter, FilterMode::ActiveOnly);
        state.cycle_filter();
        assert_eq!(state.filter, FilterMode::CurrentBranch);
        state.cycle_filter();
        assert_eq!(state.filter, FilterMode::All);
    }

    // --- Quick select ---

    #[test]
    fn quick_select_first() {
        let mut state = state_with_runs(vec![
            make_run(1, RunStatus::Completed, Some(Conclusion::Success)),
            make_run(2, RunStatus::Completed, Some(Conclusion::Success)),
            make_run(3, RunStatus::Completed, Some(Conclusion::Success)),
        ]);
        state.quick_select(1);
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn quick_select_second() {
        let mut state = state_with_runs(vec![
            make_run(1, RunStatus::Completed, Some(Conclusion::Success)),
            make_run(2, RunStatus::Completed, Some(Conclusion::Success)),
            make_run(3, RunStatus::Completed, Some(Conclusion::Success)),
        ]);
        state.quick_select(2);
        assert_eq!(state.cursor, 1);
    }

    #[test]
    fn quick_select_skips_non_run_items() {
        let mut run = make_run(1, RunStatus::Completed, Some(Conclusion::Success));
        run.jobs = vec![make_job(
            "build",
            RunStatus::Completed,
            Some(Conclusion::Success),
        )];
        run.jobs_fetched = true;
        let mut state = state_with_runs(vec![
            run,
            make_run(2, RunStatus::Completed, Some(Conclusion::Success)),
        ]);
        state.expanded_runs.insert(1);
        state.rebuild_tree();
        // tree: run1(0), job(1), run2(2)
        state.quick_select(2);
        assert_eq!(state.cursor, 2); // jumped to second run, skipping job
    }

    #[test]
    fn quick_select_out_of_range_does_nothing() {
        let mut state = state_with_runs(vec![make_run(
            1,
            RunStatus::Completed,
            Some(Conclusion::Success),
        )]);
        state.cursor = 0;
        state.quick_select(5);
        assert_eq!(state.cursor, 0); // unchanged
    }

    // --- Misc ---

    #[test]
    fn has_active_runs_true() {
        let state = state_with_runs(vec![make_run(1, RunStatus::InProgress, None)]);
        assert!(state.has_active_runs());
    }

    #[test]
    fn has_active_runs_false() {
        let state = state_with_runs(vec![make_run(
            1,
            RunStatus::Completed,
            Some(Conclusion::Success),
        )]);
        assert!(!state.has_active_runs());
    }

    #[test]
    fn has_active_runs_empty() {
        let state = state_with_runs(vec![]);
        assert!(!state.has_active_runs());
    }

    #[test]
    fn error_lifecycle() {
        let mut state = state_with_runs(vec![]);
        assert!(state.error_message().is_none());
        state.set_error("something broke".to_string());
        assert_eq!(state.error_message(), Some("something broke"));
        state.clear_error();
        assert!(state.error_message().is_none());
    }

    #[test]
    fn resolve_item_run() {
        let state = state_with_runs(vec![make_run(
            1,
            RunStatus::Completed,
            Some(Conclusion::Success),
        )]);
        let item = &state.tree_items[0];
        assert!(matches!(
            state.resolve_item(item),
            Some(ResolvedItem::Run(_))
        ));
    }

    #[test]
    fn resolve_item_job() {
        let mut run = make_run(1, RunStatus::Completed, Some(Conclusion::Success));
        run.jobs = vec![make_job(
            "build",
            RunStatus::Completed,
            Some(Conclusion::Success),
        )];
        run.jobs_fetched = true;
        let mut state = state_with_runs(vec![run]);
        state.expanded_runs.insert(1);
        state.rebuild_tree();
        let item = &state.tree_items[1];
        assert!(matches!(
            state.resolve_item(item),
            Some(ResolvedItem::Job(_))
        ));
    }

    #[test]
    fn resolve_item_step() {
        let mut run = make_run(1, RunStatus::Completed, Some(Conclusion::Success));
        run.jobs = vec![make_job(
            "build",
            RunStatus::Completed,
            Some(Conclusion::Success),
        )];
        run.jobs_fetched = true;
        let mut state = state_with_runs(vec![run]);
        state.expanded_runs.insert(1);
        state.expanded_jobs.insert((1, 1));
        state.rebuild_tree();
        let item = &state.tree_items[2]; // first step
        assert!(matches!(
            state.resolve_item(item),
            Some(ResolvedItem::Step(_))
        ));
    }

    #[test]
    fn resolve_item_invalid_index() {
        let state = state_with_runs(vec![]);
        let item = TreeItem {
            level: TreeLevel::Run,
            run_idx: 99,
            job_idx: None,
            step_idx: None,
            expanded: false,
        };
        assert!(state.resolve_item(&item).is_none());
    }

    #[test]
    fn cursor_clamped_on_tree_shrink() {
        let mut state = state_with_runs(vec![
            make_run(1, RunStatus::Completed, Some(Conclusion::Success)),
            make_run(2, RunStatus::InProgress, None),
            make_run(3, RunStatus::Completed, Some(Conclusion::Success)),
        ]);
        state.cursor = 2;
        // Switch to active-only filter: only 1 item
        state.filter = FilterMode::ActiveOnly;
        state.rebuild_tree();
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn spinner_wraps() {
        let mut state = state_with_runs(vec![]);
        for _ in 0..SPINNER_FRAME_COUNT {
            state.advance_spinner();
        }
        assert_eq!(state.spinner_frame, 0); // wrapped back
    }

    #[test]
    fn current_run_url_returns_url() {
        let state = state_with_runs(vec![make_run(
            42,
            RunStatus::Completed,
            Some(Conclusion::Success),
        )]);
        assert_eq!(
            state.current_run_url(),
            Some("https://github.com/test/repo/actions/runs/42")
        );
    }

    #[test]
    fn current_run_id_returns_id() {
        let state = state_with_runs(vec![make_run(
            42,
            RunStatus::Completed,
            Some(Conclusion::Success),
        )]);
        assert_eq!(state.current_run_id(), Some(42));
    }

    #[test]
    fn current_run_url_empty() {
        let state = state_with_runs(vec![]);
        assert_eq!(state.current_run_url(), None);
    }

    // --- Log overlay tests ---

    #[test]
    fn current_item_is_failed_on_failed_run() {
        let state = state_with_runs(vec![make_run(
            1,
            RunStatus::Completed,
            Some(Conclusion::Failure),
        )]);
        assert!(state.current_item_is_failed());
    }

    #[test]
    fn current_item_is_failed_on_success_run() {
        let state = state_with_runs(vec![make_run(
            1,
            RunStatus::Completed,
            Some(Conclusion::Success),
        )]);
        assert!(!state.current_item_is_failed());
    }

    #[test]
    fn current_item_is_failed_on_failed_job() {
        let mut run = make_run(1, RunStatus::Completed, Some(Conclusion::Failure));
        run.jobs = vec![make_job(
            "build",
            RunStatus::Completed,
            Some(Conclusion::Failure),
        )];
        run.jobs_fetched = true;
        let mut state = state_with_runs(vec![run]);
        state.expanded_runs.insert(1);
        state.rebuild_tree();
        state.cursor = 1; // on the job
        assert!(state.current_item_is_failed());
    }

    #[test]
    fn current_item_is_failed_empty() {
        let state = state_with_runs(vec![]);
        assert!(!state.current_item_is_failed());
    }

    #[test]
    fn current_item_ids_run() {
        let state = state_with_runs(vec![make_run(
            42,
            RunStatus::Completed,
            Some(Conclusion::Failure),
        )]);
        assert_eq!(state.current_item_ids(), Some((42, None)));
    }

    #[test]
    fn current_item_ids_job() {
        let mut run = make_run(42, RunStatus::Completed, Some(Conclusion::Failure));
        let mut job = make_job("build", RunStatus::Completed, Some(Conclusion::Failure));
        job.database_id = Some(99);
        run.jobs = vec![job];
        run.jobs_fetched = true;
        let mut state = state_with_runs(vec![run]);
        state.expanded_runs.insert(42);
        state.rebuild_tree();
        state.cursor = 1; // on the job
        assert_eq!(state.current_item_ids(), Some((42, Some(99))));
    }

    #[test]
    fn current_item_ids_empty() {
        let state = state_with_runs(vec![]);
        assert_eq!(state.current_item_ids(), None);
    }

    fn unwrap_log_overlay(state: &AppState) -> &LogOverlay {
        match &state.overlay {
            ActiveOverlay::Log(o) => o,
            _ => panic!("Expected Log overlay"),
        }
    }

    #[test]
    fn open_close_log_overlay() {
        let mut state = state_with_runs(vec![]);
        assert!(!state.has_log_overlay());

        state.open_log_overlay("Test".to_string(), "line1\nline2", 1, None);
        assert!(state.has_log_overlay());
        assert_eq!(unwrap_log_overlay(&state).lines.len(), 2);
        assert_eq!(state.log_overlay_text(), Some("line1\nline2".to_string()));

        state.close_log_overlay();
        assert!(!state.has_log_overlay());
        assert_eq!(state.log_overlay_text(), None);
    }

    #[test]
    fn log_overlay_truncates_long_content() {
        let mut state = state_with_runs(vec![]);
        let content: String = (0..600)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        state.open_log_overlay("Test".to_string(), &content, 1, None);
        assert_eq!(unwrap_log_overlay(&state).lines.len(), LOG_MAX_LINES);
        // Should keep last 500 lines (100..599)
        assert!(unwrap_log_overlay(&state).lines[0].contains("100"));
    }

    #[test]
    fn scroll_log_bounds() {
        let mut state = state_with_runs(vec![]);
        let content = (0..50)
            .map(|i| format!("line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        state.open_log_overlay("Test".to_string(), &content, 1, None);

        // Scroll down
        state.scroll_log_down(5, 20);
        assert_eq!(unwrap_log_overlay(&state).scroll, 5);

        // Scroll up
        state.scroll_log_up(3);
        assert_eq!(unwrap_log_overlay(&state).scroll, 2);

        // Scroll up past top
        state.scroll_log_up(10);
        assert_eq!(unwrap_log_overlay(&state).scroll, 0);

        // Scroll down past max
        state.scroll_log_down(100, 20);
        assert_eq!(unwrap_log_overlay(&state).scroll, 30); // 50 - 20

        // Jump to top
        state.scroll_log_to_top();
        assert_eq!(unwrap_log_overlay(&state).scroll, 0);

        // Jump to bottom
        state.scroll_log_to_bottom(20);
        assert_eq!(unwrap_log_overlay(&state).scroll, 30);
    }
}
