mod cli;
mod executor;
mod parser;

use ciw_core::app;
use ciw_core::diff;
use ciw_core::events;
use ciw_core::input;
use ciw_core::notify;
use ciw_core::platform::PlatformConfig;
use ciw_core::poller::{self, Poller};
use ciw_core::traits::{CiExecutor, CiParser};
use ciw_core::tui;

use app::AppState;
use clap::Parser;
use cli::Cli;
use color_eyre::eyre::{eyre, Result};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen, SetTitle};
use events::{AppEvent, EventHandler};
use executor::GhExecutor;
use input::{Action, InputContext, OverlayMode};
use parser::GhParser;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::future::Future;
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::watch;

const GHW_ART: &[&str] = &[
    r"            __                                    ",
    r"          |  \                                    ",
    r"   ______ | ▓▓____  __   __   __                  ",
    r"  /      \| ▓▓    \|  \ |  \ |  \                 ",
    r" |  ▓▓▓▓▓▓\ ▓▓▓▓▓▓▓\ ▓▓ | ▓▓ | ▓▓               ",
    r" | ▓▓  | ▓▓ ▓▓  | ▓▓ ▓▓ | ▓▓ | ▓▓               ",
    r" | ▓▓__| ▓▓ ▓▓  | ▓▓ ▓▓_/ ▓▓_/ ▓▓               ",
    r"  \▓▓    ▓▓ ▓▓  | ▓▓\▓▓   ▓▓   ▓▓               ",
    r"  _\▓▓▓▓▓▓▓\▓▓   \▓▓ \▓▓▓▓▓\▓▓▓▓                ",
    r" |  \__| ▓▓                                       ",
    r"  \▓▓    ▓▓                                       ",
    r"   \▓▓▓▓▓▓                                        ",
];

static GHW_PLATFORM: PlatformConfig = PlatformConfig {
    name: "workflow",
    full_name: "GitHub Actions",
    cli_tool: "GitHub",
    install_hint: "Install it from https://cli.github.com/",
    ascii_art: GHW_ART,
};

fn setup_verbose_logging() -> Result<()> {
    let state_dir = dirs_next_or_fallback();
    std::fs::create_dir_all(&state_dir)
        .map_err(|e| eyre!("Failed to create log directory {state_dir:?}: {e}"))?;
    let log_path = state_dir.join("debug.log");
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| eyre!("Failed to open log file {log_path:?}: {e}"))?;
    tracing_subscriber::fmt()
        .with_writer(file)
        .with_ansi(false)
        .init();
    tracing::info!(
        "ghw v{} starting with verbose logging",
        env!("CARGO_PKG_VERSION")
    );
    Ok(())
}

fn dirs_next_or_fallback() -> std::path::PathBuf {
    if let Some(state) = std::env::var_os("XDG_STATE_HOME") {
        std::path::PathBuf::from(state).join("ghw")
    } else if let Some(home) = std::env::var_os("HOME") {
        std::path::PathBuf::from(home)
            .join(".local")
            .join("state")
            .join("ghw")
    } else {
        std::path::PathBuf::from("/tmp/ghw")
    }
}

fn spawn_monitored(
    tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    label: &'static str,
    fut: impl Future<Output = ()> + Send + 'static,
) {
    tokio::spawn(async move {
        let handle = tokio::spawn(fut);
        if let Err(join_err) = handle.await {
            let msg = if join_err.is_panic() {
                match join_err.into_panic().downcast::<String>() {
                    Ok(s) => *s,
                    Err(payload) => match payload.downcast::<&str>() {
                        Ok(s) => s.to_string(),
                        Err(_) => "unknown panic".to_string(),
                    },
                }
            } else {
                "task cancelled".to_string()
            };
            tracing::error!("{label} panicked: {msg}");
            if tx
                .send(AppEvent::Error(format!("{label} crashed: {msg}")))
                .is_err()
            {
                tracing::warn!("{label}: channel closed while reporting panic");
            }
        }
    });
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Cli::parse();

    // Setup verbose logging
    if args.verbose {
        setup_verbose_logging()?;
    }

    // Setup terminal with panic hook early, before any data fetching
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        if let Err(e) = terminal::disable_raw_mode() {
            eprintln!("Failed to disable raw mode during panic: {e}");
        }
        if let Err(e) = execute!(io::stdout(), LeaveAlternateScreen, SetTitle("")) {
            eprintln!("Failed to leave alternate screen during panic: {e}");
        }
        original_hook(panic_info);
    }));

    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Create executor and parser for startup (repo not yet known)
    let startup_executor = GhExecutor::new(String::new());
    let gh_parser = GhParser;

    // Run startup phases with animated spinner
    let startup_result = match tui::startup::run_startup(
        &mut terminal,
        &GHW_PLATFORM,
        &startup_executor,
        &gh_parser,
        args.repo.as_deref(),
        args.branch.as_deref(),
        args.limit,
        args.workflow.as_deref(),
        Some(cli::validate_repo_format),
    )
    .await
    {
        Ok(result) => result,
        Err(e) => {
            // Restore terminal before printing error
            terminal::disable_raw_mode()?;
            execute!(terminal.backend_mut(), LeaveAlternateScreen, SetTitle(""))?;
            terminal.show_cursor()?;
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let repo = startup_result.repo;
    execute!(io::stdout(), SetTitle(format!("watching {}", repo)))?;
    let branch = startup_result.branch;

    let version_string = format!(
        "ghw v{}+{}",
        env!("CARGO_PKG_VERSION"),
        env!("BUILD_NUMBER")
    );

    let mut state = AppState::new(repo.clone(), branch, args.limit, args.workflow.clone());
    state.config.version_string = version_string;
    state.poll_interval = args.interval;
    state.desktop_notify = !args.no_notify;
    state.runs = startup_result.runs;
    state.rebuild_tree();
    state.last_poll = Some(Instant::now());

    // Create the real executor (with repo) and parser as Arc trait objects
    let executor: Arc<dyn CiExecutor> = Arc::new(GhExecutor::new(repo.clone()));
    let parser: Arc<dyn CiParser> = Arc::new(GhParser);

    // Event handler
    let events = EventHandler::new(Duration::from_millis(100));
    let tx = events.sender();

    // Adaptive polling interval channel
    let (interval_tx, interval_rx) = watch::channel(args.interval);

    // Start poller
    let poller_tx = tx.clone();
    let poller_executor = executor.clone();
    let poller_parser = parser.clone();
    let poller_workflow = args.workflow.clone();
    let poller_limit = args.limit;
    let poller_handle = tokio::spawn(async move {
        let poller = Poller::new(
            poller_executor,
            poller_parser,
            poller_limit,
            poller_workflow,
            poller_tx,
            interval_rx,
        );
        poller.run().await;
    });

    let result = run_app(
        &mut terminal,
        &mut state,
        events,
        &tx,
        &interval_tx,
        poller_handle,
        executor,
        parser,
    )
    .await;

    // Restore terminal
    terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, SetTitle(""))?;
    terminal.show_cursor()?;

    result
}

/// Visible height of the log overlay area (80% of terminal height minus borders).
/// Must stay in sync with the sizing logic in `tui::log_overlay::render`.
fn log_overlay_height(terminal: &Terminal<CrosstermBackend<io::Stdout>>) -> usize {
    terminal
        .size()
        .map(|s| s.height as usize * 8 / 10)
        .unwrap_or_else(|e| {
            tracing::warn!("terminal size query failed: {e}");
            20
        })
        .saturating_sub(2)
}

#[allow(clippy::too_many_arguments)]
async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
    mut events: EventHandler,
    tx: &tokio::sync::mpsc::UnboundedSender<AppEvent>,
    interval_tx: &watch::Sender<u64>,
    poller_handle: tokio::task::JoinHandle<()>,
    executor: Arc<dyn CiExecutor>,
    parser: Arc<dyn CiParser>,
) -> Result<()> {
    let mut last_tick = Instant::now();
    let mut poll_start = Instant::now();

    loop {
        // Render
        terminal.draw(|f| tui::render::render(f, state))?;

        // Update countdown
        let elapsed = poll_start.elapsed().as_secs();
        state.next_poll_in = state.poll_interval.saturating_sub(elapsed);

        // Prune old notifications, stale errors, and expired log cache entries
        state.prune_notifications();
        state.prune_error();
        state.prune_log_cache();

        // Process events
        if let Some(event) = events.next().await {
            match event {
                AppEvent::Key(key) => {
                    let ctx = InputContext {
                        has_error: state.error.is_some(),
                        is_loading: state.is_loading(),
                        overlay: if state.has_log_overlay() {
                            OverlayMode::Log
                        } else if state.has_detail_overlay() {
                            OverlayMode::Detail
                        } else if state.has_confirm_overlay() {
                            OverlayMode::Confirm
                        } else {
                            OverlayMode::None
                        },
                    };
                    match input::map_key(key, &ctx) {
                        Action::Quit => state.should_quit = true,
                        Action::DismissError => state.clear_error(),
                        Action::MoveUp => state.move_cursor_up(),
                        Action::MoveDown => state.move_cursor_down(),
                        Action::Expand => {
                            if let Some((run_idx, needs_fetch)) = state.expand_current() {
                                if needs_fetch {
                                    if let Some(run) = state.runs.get(run_idx) {
                                        let run_id = run.database_id;
                                        let tx2 = tx.clone();
                                        let executor2 = executor.clone();
                                        let parser2 = parser.clone();
                                        spawn_monitored(tx.clone(), "expand_jobs", async move {
                                            poller::fetch_jobs_for_run(
                                                &*executor2,
                                                &*parser2,
                                                run_id,
                                                &tx2,
                                            )
                                            .await;
                                        });
                                    }
                                }
                            }
                        }
                        Action::Collapse => state.collapse_current(),
                        Action::Toggle => state.toggle_expand(),
                        Action::Refresh => {
                            state.begin_loading();
                            let tx2 = tx.clone();
                            let executor2 = executor.clone();
                            let parser2 = parser.clone();
                            let limit = state.config.limit;
                            let wf = state.config.workflow_filter.clone();
                            spawn_monitored(tx.clone(), "refresh", async move {
                                match executor2.fetch_runs(limit, wf.as_deref()).await {
                                    Ok(json) => match parser2.parse_runs(&json) {
                                        Ok(runs) => {
                                            if tx2
                                                .send(AppEvent::PollResult { runs, manual: true })
                                                .is_err()
                                            {
                                                tracing::warn!("refresh: channel closed");
                                            }
                                        }
                                        Err(e) => {
                                            if tx2.send(AppEvent::Error(format!("{}", e))).is_err()
                                            {
                                                tracing::warn!("refresh: channel closed");
                                            }
                                        }
                                    },
                                    Err(e) => {
                                        if tx2.send(AppEvent::Error(format!("{}", e))).is_err() {
                                            tracing::warn!("refresh: channel closed");
                                        }
                                    }
                                }
                            });
                            poll_start = Instant::now();
                        }
                        Action::RerunFailed => {
                            if let Some(run_id) = state.current_run_id() {
                                let run_conclusion = state
                                    .runs
                                    .iter()
                                    .find(|r| r.database_id == run_id)
                                    .and_then(|r| r.conclusion);
                                if state.current_run_status() != Some(app::RunStatus::Completed) {
                                    state.set_error(
                                        "Cannot rerun: workflow is still in progress".to_string(),
                                    );
                                } else if run_conclusion == Some(app::Conclusion::Success) {
                                    state.set_error(
                                        "Run completed successfully — nothing to rerun".to_string(),
                                    );
                                } else {
                                    let executor2 = executor.clone();
                                    let tx2 = tx.clone();
                                    spawn_monitored(tx.clone(), "rerun", async move {
                                        match executor2.rerun_failed(run_id).await {
                                            Ok(()) => {
                                                if tx2.send(AppEvent::RerunSuccess(run_id)).is_err()
                                                {
                                                    tracing::warn!("rerun: channel closed");
                                                }
                                            }
                                            Err(e) => {
                                                if tx2
                                                    .send(AppEvent::Error(format!("{}", e)))
                                                    .is_err()
                                                {
                                                    tracing::warn!("rerun: channel closed");
                                                }
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        Action::CancelRun => {
                            if let Some(run_id) = state.current_run_id() {
                                if state.current_run_status() != Some(app::RunStatus::InProgress) {
                                    state.set_error(
                                        "Cannot cancel: run is not in progress".to_string(),
                                    );
                                } else {
                                    let title = state
                                        .current_run_display_title()
                                        .unwrap_or_else(|| format!("run {run_id}"));
                                    state.open_confirm_overlay(
                                        "Confirm Cancel".to_string(),
                                        format!("Cancel \"{title}\"?"),
                                        app::ConfirmAction::CancelRun(run_id),
                                    );
                                }
                            }
                        }
                        Action::DeleteRun => {
                            if let Some(run_id) = state.current_run_id() {
                                if state.current_run_status() == Some(app::RunStatus::InProgress) {
                                    state.set_error(
                                        "Cannot delete: run is still in progress".to_string(),
                                    );
                                } else {
                                    let title = state
                                        .current_run_display_title()
                                        .unwrap_or_else(|| format!("run {run_id}"));
                                    state.open_confirm_overlay(
                                        "Confirm Delete".to_string(),
                                        format!("Delete \"{title}\"?"),
                                        app::ConfirmAction::DeleteRun(run_id),
                                    );
                                }
                            }
                        }
                        Action::ConfirmYes => {
                            if let Some(action) = state.confirm_action() {
                                state.close_confirm_overlay();
                                match action {
                                    app::ConfirmAction::CancelRun(run_id) => {
                                        let executor2 = executor.clone();
                                        let tx2 = tx.clone();
                                        spawn_monitored(tx.clone(), "cancel_run", async move {
                                            match executor2.cancel_run(run_id).await {
                                                Ok(()) => {
                                                    if tx2
                                                        .send(AppEvent::CancelSuccess(run_id))
                                                        .is_err()
                                                    {
                                                        tracing::warn!(
                                                            "cancel_run: channel closed"
                                                        );
                                                    }
                                                }
                                                Err(e) => {
                                                    if tx2
                                                        .send(AppEvent::Error(format!("{e}")))
                                                        .is_err()
                                                    {
                                                        tracing::warn!(
                                                            "cancel_run: channel closed"
                                                        );
                                                    }
                                                }
                                            }
                                        });
                                    }
                                    app::ConfirmAction::DeleteRun(run_id) => {
                                        let executor2 = executor.clone();
                                        let tx2 = tx.clone();
                                        spawn_monitored(tx.clone(), "delete_run", async move {
                                            match executor2.delete_run(run_id).await {
                                                Ok(()) => {
                                                    if tx2
                                                        .send(AppEvent::DeleteSuccess(run_id))
                                                        .is_err()
                                                    {
                                                        tracing::warn!(
                                                            "delete_run: channel closed"
                                                        );
                                                    }
                                                }
                                                Err(e) => {
                                                    if tx2
                                                        .send(AppEvent::Error(format!("{e}")))
                                                        .is_err()
                                                    {
                                                        tracing::warn!(
                                                            "delete_run: channel closed"
                                                        );
                                                    }
                                                }
                                            }
                                        });
                                    }
                                }
                            }
                        }
                        Action::OpenBrowser => {
                            if let Some(url) = state.current_run_url() {
                                if let Err(e) = executor.open_in_browser(url) {
                                    state.set_error(format!("{e}"));
                                }
                            }
                        }
                        Action::ViewLogs => {
                            if !state.current_item_is_failed() {
                                state.set_error("No failure logs for this item".to_string());
                            } else if let Some((run_id, job_id)) = state.current_item_ids() {
                                // When on a Job/Step node, job_id must be available
                                let is_job_or_step =
                                    state.tree_items.get(state.cursor).is_some_and(|item| {
                                        matches!(
                                            item.level,
                                            app::TreeLevel::Job | app::TreeLevel::Step
                                        )
                                    });
                                // Defense-in-depth: rebuild_tree filters None database_id jobs,
                                // so this is currently unreachable but guards against future changes.
                                if is_job_or_step && job_id.is_none() {
                                    state.set_error(
                                        "Job ID unavailable, cannot fetch logs".to_string(),
                                    );
                                } else {
                                    let cache_key = (run_id, job_id);
                                    let cached =
                                        state.log_cache.get(&cache_key).and_then(|entry| {
                                            if entry.fetched_at.elapsed().as_secs()
                                                < app::LOG_CACHE_TTL_SECS
                                            {
                                                Some(entry.content.clone())
                                            } else {
                                                None
                                            }
                                        });
                                    if let Some(content) = cached {
                                        let title = build_log_title(state, run_id, job_id);
                                        state.open_log_overlay(title, &content, run_id, job_id);
                                    } else {
                                        let title = build_log_title(state, run_id, job_id);
                                        fetch_logs_async(
                                            &executor, &parser, run_id, job_id, &title, tx,
                                        );
                                    }
                                }
                            }
                        }
                        Action::CloseOverlay => {
                            state.close_overlay();
                        }
                        Action::ScrollUp => state.scroll_log_up(1),
                        Action::ScrollDown => {
                            state.scroll_log_down(1, log_overlay_height(terminal));
                        }
                        Action::PageUp => state.scroll_log_up(20),
                        Action::PageDown => {
                            state.scroll_log_down(20, log_overlay_height(terminal));
                        }
                        Action::ScrollToTop => state.scroll_log_to_top(),
                        Action::ScrollToBottom => {
                            state.scroll_log_to_bottom(log_overlay_height(terminal));
                        }
                        Action::CopyToClipboard => {
                            if let Some(text) = state.log_overlay_text() {
                                let executor2 = executor.clone();
                                let tx2 = tx.clone();
                                spawn_monitored(tx.clone(), "clipboard", async move {
                                    let result = executor2
                                        .copy_to_clipboard(&text)
                                        .await
                                        .map_err(|e| format!("{e}"));
                                    if tx2.send(AppEvent::ClipboardResult(result)).is_err() {
                                        tracing::warn!("clipboard: channel closed");
                                    }
                                });
                            }
                        }
                        Action::ShowDetails => {
                            if let Some(item) = state.tree_items.get(state.cursor).cloned() {
                                if let Some(resolved) = state.resolve_item(&item) {
                                    let (title, lines) =
                                        build_detail_lines(&resolved, state, item.run_idx);
                                    state.open_detail_overlay(title, lines);
                                }
                            }
                        }
                        Action::CycleFilter => state.cycle_filter(),
                        Action::FilterBranch => {
                            state.filter = app::FilterMode::CurrentBranch;
                            state.rebuild_tree();
                        }
                        Action::QuickSelect(n) => state.quick_select(n),
                        Action::None => {}
                    }
                }
                AppEvent::Tick => {
                    if last_tick.elapsed() >= Duration::from_millis(100) {
                        state.advance_spinner();
                        last_tick = Instant::now();
                    }
                    // Check if the poller task has died unexpectedly
                    if poller_handle.is_finished() {
                        state.set_error(
                            "Poller stopped unexpectedly. Press r to refresh manually.".to_string(),
                        );
                    }
                    // Adaptive polling: adjust interval and notify poller
                    let new_interval = if state.has_active_runs() {
                        app::POLL_INTERVAL_ACTIVE
                    } else if state
                        .last_poll
                        .is_some_and(|t| t.elapsed().as_secs() < app::POLL_RECENT_THRESHOLD_SECS)
                    {
                        app::POLL_INTERVAL_RECENT
                    } else {
                        app::POLL_INTERVAL_IDLE
                    };
                    if new_interval != state.poll_interval {
                        state.poll_interval = new_interval;
                        if interval_tx.send(new_interval).is_err() {
                            tracing::warn!("interval: poller channel closed");
                        }
                    }
                }
                AppEvent::PollResult {
                    runs: new_runs,
                    manual,
                } => {
                    if manual {
                        state.end_loading();
                    }
                    state.clear_error();
                    state.run_errors.clear();

                    let old_snapshot = if state.desktop_notify {
                        Some(state.previous_snapshot.clone())
                    } else {
                        None
                    };

                    diff::detect_changes(state, &new_runs);

                    if let Some(old_snapshot) = old_snapshot {
                        for run in &new_runs {
                            if run.status == app::RunStatus::Completed {
                                if let Some(entry) = old_snapshot.get(&run.database_id) {
                                    if entry.status != app::RunStatus::Completed {
                                        let run_clone = run.clone();
                                        let tx2 = tx.clone();
                                        tokio::task::spawn_blocking(move || {
                                            let result = std::panic::catch_unwind(
                                                std::panic::AssertUnwindSafe(|| {
                                                    notify::send_desktop(&run_clone)
                                                }),
                                            );
                                            match result {
                                                Ok(Some(err)) => {
                                                    if tx2.send(AppEvent::Error(err)).is_err() {
                                                        tracing::warn!("notify: channel closed");
                                                    }
                                                }
                                                Err(panic_payload) => {
                                                    let msg = panic_payload
                                                        .downcast::<String>()
                                                        .map(|s| *s)
                                                        .unwrap_or_else(|p| {
                                                            p.downcast::<&str>()
                                                                .map(|s| s.to_string())
                                                                .unwrap_or_else(|_| {
                                                                    "unknown panic".to_string()
                                                                })
                                                        });
                                                    tracing::error!("notify panicked: {msg}");
                                                    if tx2
                                                        .send(AppEvent::Error(format!(
                                                            "Notification crashed: {msg}"
                                                        )))
                                                        .is_err()
                                                    {
                                                        tracing::warn!("notify: channel closed");
                                                    }
                                                }
                                                Ok(None) => {} // success
                                            }
                                        });
                                    }
                                }
                            }
                        }
                    }

                    // Preserve jobs data for runs that haven't changed
                    let mut runs = new_runs;
                    let mut refetch_run_ids = Vec::new();
                    for run in &mut runs {
                        if let Some(old) =
                            state.runs.iter().find(|r| r.database_id == run.database_id)
                        {
                            if old.jobs.is_some() {
                                if old.updated_at == run.updated_at {
                                    run.jobs = old.jobs.clone();
                                } else if state.expanded_runs.contains(&run.database_id) {
                                    // Run changed and is expanded — re-fetch jobs
                                    refetch_run_ids.push(run.database_id);
                                }
                            }
                        }
                    }
                    state.runs = runs;
                    state.rebuild_tree();
                    state.last_poll = Some(Instant::now());
                    poll_start = Instant::now();

                    // Close log overlay if its run no longer exists
                    if let Some(overlay) = state.log_overlay_ref() {
                        let overlay_run_id = overlay.run_id;
                        if !state.runs.iter().any(|r| r.database_id == overlay_run_id) {
                            state.close_overlay();
                        }
                    }

                    // Prune expanded_runs and expanded_jobs for runs no longer present
                    let run_ids: std::collections::HashSet<u64> =
                        state.runs.iter().map(|r| r.database_id).collect();
                    state.expanded_runs.retain(|id| run_ids.contains(id));
                    state
                        .expanded_jobs
                        .retain(|(run_id, _)| run_ids.contains(run_id));

                    // Re-fetch jobs for expanded runs whose data has changed
                    for run_id in refetch_run_ids {
                        let tx2 = tx.clone();
                        let executor2 = executor.clone();
                        let parser2 = parser.clone();
                        spawn_monitored(tx.clone(), "refetch_jobs", async move {
                            poller::fetch_jobs_for_run(&*executor2, &*parser2, run_id, &tx2).await;
                        });
                    }
                }
                AppEvent::JobsResult { run_id, jobs } => {
                    // Find run by ID (index may have changed)
                    if let Some(run) = state.runs.iter_mut().find(|r| r.database_id == run_id) {
                        run.jobs = Some(jobs);
                    }
                    state.rebuild_tree();
                }
                AppEvent::FailedLogResult {
                    run_id,
                    job_id,
                    title,
                    content,
                } => {
                    state.open_log_overlay(title, &content, run_id, job_id);
                    state.log_cache.insert(
                        (run_id, job_id),
                        app::FailedLog {
                            content,
                            fetched_at: Instant::now(),
                        },
                    );
                }
                AppEvent::ClipboardResult(result) => match result {
                    Ok(()) => {
                        state.add_notification(0, "Copied to clipboard".to_string());
                    }
                    Err(e) => {
                        state.set_error(e);
                    }
                },
                AppEvent::CancelSuccess(run_id) => {
                    state.add_notification(run_id, "Run cancelled".to_string());
                    // Trigger a refresh to pick up the new status
                    poll_start = Instant::now()
                        .checked_sub(Duration::from_secs(state.poll_interval))
                        .unwrap_or(poll_start);
                }
                AppEvent::DeleteSuccess(run_id) => {
                    state.remove_run(run_id);
                    state.add_notification(run_id, "Run deleted".to_string());
                }
                AppEvent::RerunSuccess(run_id) => {
                    state.log_cache.retain(|(r, _), _| *r != run_id);
                    state.add_notification(run_id, "Rerun triggered".to_string());
                }
                AppEvent::RunError { run_id, error } => {
                    state.run_errors.insert(run_id, error);
                    state.rebuild_tree();
                }
                AppEvent::Error(e) => {
                    state.end_loading();
                    state.set_error(e);
                }
            }
        }

        if state.should_quit {
            return Ok(());
        }
    }
}

fn build_log_title(state: &AppState, run_id: u64, job_id: Option<u64>) -> String {
    let run = state.runs.iter().find(|r| r.database_id == run_id);
    let run_name = run.map_or("Unknown", |r| r.display_title.as_str());

    if let Some(jid) = job_id {
        let job_name = run
            .and_then(|r| r.jobs.as_ref()?.iter().find(|j| j.database_id == Some(jid)))
            .map_or("Unknown job", |j| j.name.as_str());
        format!("{run_name} > {job_name}")
    } else {
        run_name.to_string()
    }
}

fn build_detail_lines(
    resolved: &app::ResolvedItem<'_>,
    state: &AppState,
    run_idx: usize,
) -> (String, Vec<(String, String)>) {
    match resolved {
        app::ResolvedItem::Run(run) => {
            let title = format!("Run #{}", run.number);
            let conclusion_str = run.conclusion.map_or("-".into(), |c| format!("{c:?}"));
            let end = if run.status == app::RunStatus::Completed {
                Some(run.updated_at)
            } else {
                None
            };
            let duration = app::compute_duration(Some(run.created_at), end);
            let lines = vec![
                ("Title".into(), run.display_title.clone()),
                ("Workflow".into(), run.name.clone()),
                ("Branch".into(), run.head_branch.clone()),
                ("Event".into(), run.event.clone()),
                ("Status".into(), format!("{:?}", run.status)),
                ("Conclusion".into(), conclusion_str),
                ("Duration".into(), duration),
                (
                    "Created".into(),
                    run.created_at.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                ),
                (
                    "Updated".into(),
                    run.updated_at.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                ),
                ("URL".into(), run.url.clone()),
            ];
            (title, lines)
        }
        app::ResolvedItem::Job(job) => {
            let title = format!("Job: {}", job.name);
            let conclusion_str = job.conclusion.map_or("-".into(), |c| format!("{c:?}"));
            let mut lines = vec![
                ("Name".into(), job.name.clone()),
                ("Status".into(), format!("{:?}", job.status)),
                ("Conclusion".into(), conclusion_str),
            ];
            if let Some(started) = job.started_at {
                lines.push((
                    "Started".into(),
                    started.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                ));
            }
            if let Some(completed) = job.completed_at {
                lines.push((
                    "Completed".into(),
                    completed.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                ));
            }
            let dur = app::compute_duration(job.started_at, job.completed_at);
            if !dur.is_empty() {
                let label = if job.completed_at.is_none() {
                    format!("{dur} (running)")
                } else {
                    dur
                };
                lines.push(("Duration".into(), label));
            }
            lines.push(("URL".into(), job.url.clone()));
            // Show parent run info
            if let Some(run) = state.runs.get(run_idx) {
                lines.push((
                    "Run".into(),
                    format!("#{} {}", run.number, run.display_title),
                ));
            }
            (title, lines)
        }
        app::ResolvedItem::Step(step) => {
            let title = format!("Step #{}: {}", step.number, step.name);
            let conclusion_str = step.conclusion.map_or("-".into(), |c| format!("{c:?}"));
            let mut lines = vec![
                ("Name".into(), step.name.clone()),
                ("Number".into(), step.number.to_string()),
                ("Status".into(), format!("{:?}", step.status)),
                ("Conclusion".into(), conclusion_str),
            ];
            if let Some(started) = step.started_at {
                lines.push((
                    "Started".into(),
                    started.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                ));
            }
            if let Some(completed) = step.completed_at {
                lines.push((
                    "Completed".into(),
                    completed.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                ));
            }
            let dur = app::compute_duration(step.started_at, step.completed_at);
            if !dur.is_empty() {
                let label = if step.completed_at.is_none() {
                    format!("{dur} (running)")
                } else {
                    dur
                };
                lines.push(("Duration".into(), label));
            }
            (title, lines)
        }
    }
}

fn fetch_logs_async(
    executor: &Arc<dyn CiExecutor>,
    parser: &Arc<dyn CiParser>,
    run_id: u64,
    job_id: Option<u64>,
    title: &str,
    tx: &tokio::sync::mpsc::UnboundedSender<AppEvent>,
) {
    let executor = executor.clone();
    let parser = parser.clone();
    let title = title.to_string();
    let tx2 = tx.clone();
    spawn_monitored(tx.clone(), "fetch_logs", async move {
        let result = if let Some(jid) = job_id {
            executor.fetch_failed_logs_for_job(run_id, jid).await
        } else {
            executor.fetch_failed_logs(run_id).await
        };
        match result {
            Ok(raw) => {
                let (content, _truncated) = parser.process_log_output(&raw, app::LOG_MAX_LINES);
                let content = if content.trim().is_empty() {
                    "(no failed step logs available)".to_string()
                } else {
                    content
                };
                if tx2
                    .send(AppEvent::FailedLogResult {
                        run_id,
                        job_id,
                        title,
                        content,
                    })
                    .is_err()
                {
                    tracing::warn!("fetch_logs: channel closed");
                }
            }
            Err(e) => {
                if tx2.send(AppEvent::Error(format!("{}", e))).is_err() {
                    tracing::warn!("fetch_logs: channel closed");
                }
            }
        }
    });
}
