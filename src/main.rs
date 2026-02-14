use ghw::app;
use ghw::cli;
use ghw::diff;
use ghw::events;
use ghw::gh;
use ghw::input;
use ghw::notify;
use ghw::tui;

use app::AppState;
use clap::Parser;
use cli::Cli;
use color_eyre::eyre::Result;
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use events::{AppEvent, EventHandler};
use gh::poller::{self, Poller};
use input::Action;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::time::{Duration, Instant};
use tokio::sync::watch;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Cli::parse();

    // Startup validation
    if let Err(e) = gh::executor::check_gh_available().await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    // Detect repo
    let repo = match args.repo {
        Some(r) => r,
        None => match gh::executor::detect_repo().await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
    };

    // Detect branch
    let branch = match args.branch {
        Some(b) => Some(b),
        None => gh::executor::detect_branch().await.ok(),
    };

    let mut state = AppState::new(repo.clone(), branch, args.limit, args.workflow.clone());
    state.poll_interval = args.interval;
    state.is_loading = true;
    state.desktop_notify = !args.no_notify;

    // Setup terminal with panic hook
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Event handler
    let events = EventHandler::new(Duration::from_millis(100));
    let tx = events.sender();

    // Adaptive polling interval channel
    let (interval_tx, interval_rx) = watch::channel(args.interval);

    // Start poller
    let poller_tx = tx.clone();
    let poller_repo = repo.clone();
    let poller_workflow = args.workflow.clone();
    let poller_limit = args.limit;
    tokio::spawn(async move {
        let poller = Poller::new(
            poller_repo,
            poller_limit,
            poller_workflow,
            poller_tx,
            interval_rx,
        );
        poller.run().await;
    });

    let result = run_app(&mut terminal, &mut state, events, &tx, &repo, &interval_tx).await;

    // Restore terminal
    terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
    mut events: EventHandler,
    tx: &tokio::sync::mpsc::UnboundedSender<AppEvent>,
    repo: &str,
    interval_tx: &watch::Sender<u64>,
) -> Result<()> {
    let mut last_tick = Instant::now();
    let mut poll_start = Instant::now();

    loop {
        // Render
        terminal.draw(|f| tui::render::render(f, state))?;

        // Update countdown
        let elapsed = poll_start.elapsed().as_secs();
        state.next_poll_in = state.poll_interval.saturating_sub(elapsed);

        // Prune old notifications and stale errors
        state.prune_notifications();
        state.prune_error();

        // Process events
        if let Some(event) = events.next().await {
            match event {
                AppEvent::Key(key) => {
                    match input::map_key(key, state.error.is_some(), state.is_loading, state.has_log_overlay()) {
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
                                        let repo2 = repo.to_string();
                                        tokio::spawn(async move {
                                            poller::fetch_jobs_for_run(&repo2, run_id, &tx2)
                                                .await;
                                        });
                                    }
                                }
                            }
                        }
                        Action::Collapse => state.collapse_current(),
                        Action::Toggle => state.toggle_expand(),
                        Action::Refresh => {
                            state.is_loading = true;
                            let tx2 = tx.clone();
                            let repo2 = repo.to_string();
                            let limit = state.config.limit;
                            let wf = state.config.workflow_filter.clone();
                            tokio::spawn(async move {
                                match gh::executor::fetch_runs(&repo2, limit, wf.as_deref())
                                    .await
                                {
                                    Ok(json) => match gh::parser::parse_runs(&json) {
                                        Ok(runs) => {
                                            let _ = tx2.send(AppEvent::PollResult(runs));
                                        }
                                        Err(e) => {
                                            let _ =
                                                tx2.send(AppEvent::Error(format!("{}", e)));
                                        }
                                    },
                                    Err(e) => {
                                        let _ = tx2.send(AppEvent::Error(format!("{}", e)));
                                    }
                                }
                            });
                            poll_start = Instant::now();
                        }
                        Action::RerunFailed => {
                            if let Some(run_id) = state.current_run_id() {
                                // Clear log cache for this run (logs will change after rerun)
                                state.log_cache.retain(|(r, _), _| *r != run_id);
                                let repo2 = repo.to_string();
                                let tx2 = tx.clone();
                                tokio::spawn(async move {
                                    if let Err(e) =
                                        gh::executor::rerun_failed(&repo2, run_id).await
                                    {
                                        let _ = tx2.send(AppEvent::Error(format!("{}", e)));
                                    }
                                });
                            }
                        }
                        Action::OpenBrowser => {
                            if let Some(url) = state.current_run_url() {
                                let url = url.to_string();
                                tokio::spawn(async move {
                                    let _ = gh::executor::open_in_browser(&url).await;
                                });
                            }
                        }
                        Action::ViewLogs => {
                            if !state.current_item_is_failed() {
                                state.set_error("No failure logs for this item".to_string());
                            } else if let Some((run_id, job_id)) = state.current_item_ids() {
                                let cache_key = (run_id, job_id);
                                let cached = state.log_cache.get(&cache_key).and_then(|entry| {
                                    if entry.fetched_at.elapsed().as_secs() < app::LOG_CACHE_TTL_SECS {
                                        Some(entry.content.clone())
                                    } else {
                                        None
                                    }
                                });
                                if let Some(content) = cached {
                                    let title = build_log_title(state, run_id, job_id);
                                    state.open_log_overlay(title, content, run_id, job_id);
                                } else {
                                    let title = build_log_title(state, run_id, job_id);
                                    fetch_logs_async(repo, run_id, job_id, &title, tx);
                                }
                            }
                        }
                        Action::CloseOverlay => state.close_log_overlay(),
                        Action::ScrollUp => state.scroll_log_up(1),
                        Action::ScrollDown => {
                            let h = terminal.size().map(|s| s.height as usize * 8 / 10).unwrap_or(20).saturating_sub(2);
                            state.scroll_log_down(1, h);
                        }
                        Action::PageUp => state.scroll_log_up(20),
                        Action::PageDown => {
                            let h = terminal.size().map(|s| s.height as usize * 8 / 10).unwrap_or(20).saturating_sub(2);
                            state.scroll_log_down(20, h);
                        }
                        Action::ScrollToTop => state.scroll_log_to_top(),
                        Action::ScrollToBottom => {
                            let h = terminal.size().map(|s| s.height as usize * 8 / 10).unwrap_or(20).saturating_sub(2);
                            state.scroll_log_to_bottom(h);
                        }
                        Action::CopyToClipboard => {
                            if let Some(text) = state.log_overlay_text() {
                                let tx2 = tx.clone();
                                tokio::spawn(async move {
                                    let ok = gh::executor::copy_to_clipboard(&text).await.is_ok();
                                    let _ = tx2.send(AppEvent::ClipboardResult(ok));
                                });
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
                        let _ = interval_tx.send(new_interval);
                    }
                }
                AppEvent::PollResult(new_runs) => {
                    state.is_loading = false;
                    state.clear_error();

                    let old_snapshot = if state.desktop_notify {
                        Some(state.previous_snapshot.clone())
                    } else {
                        None
                    };

                    diff::detect_changes(state, &new_runs);

                    if let Some(old_snapshot) = old_snapshot {
                        for run in &new_runs {
                            if run.status == app::RunStatus::Completed {
                                if let Some(&(old_status, _)) = old_snapshot.get(&run.database_id) {
                                    if old_status != app::RunStatus::Completed {
                                        notify::send_desktop(run);
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
                            if old.jobs_fetched {
                                if old.updated_at == run.updated_at {
                                    run.jobs = old.jobs.clone();
                                    run.jobs_fetched = true;
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

                    // Re-fetch jobs for expanded runs whose data has changed
                    for run_id in refetch_run_ids {
                        let tx2 = tx.clone();
                        let repo2 = repo.to_string();
                        tokio::spawn(async move {
                            poller::fetch_jobs_for_run(&repo2, run_id, &tx2).await;
                        });
                    }
                }
                AppEvent::JobsResult { run_id, jobs } => {
                    // Find run by ID (index may have changed)
                    if let Some(run) = state.runs.iter_mut().find(|r| r.database_id == run_id) {
                        run.jobs = jobs;
                        run.jobs_fetched = true;
                    }
                    state.rebuild_tree();
                }
                AppEvent::FailedLogResult { run_id, job_id, title, content } => {
                    // Cache the result
                    state.log_cache.insert(
                        (run_id, job_id),
                        app::FailedLog {
                            content: content.clone(),
                            fetched_at: Instant::now(),
                        },
                    );
                    state.open_log_overlay(title, content, run_id, job_id);
                }
                AppEvent::ClipboardResult(ok) => {
                    if ok {
                        state.notifications.push(app::Notification {
                            run_id: 0,
                            message: "Copied to clipboard".to_string(),
                            timestamp: Instant::now(),
                        });
                    } else {
                        state.set_error("Failed to copy to clipboard".to_string());
                    }
                }
                AppEvent::Error(e) => {
                    state.is_loading = false;
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
    let run_name = state
        .runs
        .iter()
        .find(|r| r.database_id == run_id)
        .map(|r| r.display_title.as_str())
        .unwrap_or("Unknown");

    if let Some(jid) = job_id {
        let job_name = state
            .runs
            .iter()
            .find(|r| r.database_id == run_id)
            .and_then(|r| r.jobs.iter().find(|j| j.database_id == Some(jid)))
            .map(|j| j.name.as_str())
            .unwrap_or("Unknown job");
        format!("{} > {}", run_name, job_name)
    } else {
        run_name.to_string()
    }
}

fn fetch_logs_async(
    repo: &str,
    run_id: u64,
    job_id: Option<u64>,
    title: &str,
    tx: &tokio::sync::mpsc::UnboundedSender<AppEvent>,
) {
    let repo = repo.to_string();
    let title = title.to_string();
    let tx = tx.clone();
    tokio::spawn(async move {
        let result = if let Some(jid) = job_id {
            gh::executor::fetch_failed_logs_for_job(&repo, run_id, jid).await
        } else {
            gh::executor::fetch_failed_logs(&repo, run_id).await
        };
        match result {
            Ok(raw) => {
                let (content, _truncated) =
                    gh::parser::process_log_output(&raw, app::LOG_MAX_LINES);
                let content = if content.trim().is_empty() {
                    "(no failed step logs available)".to_string()
                } else {
                    content
                };
                let _ = tx.send(AppEvent::FailedLogResult {
                    run_id,
                    job_id,
                    title,
                    content,
                });
            }
            Err(e) => {
                let _ = tx.send(AppEvent::Error(format!("{}", e)));
            }
        }
    });
}
