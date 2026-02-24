//! Terminal input thread and application event channel.
//!
//! [`EventHandler`] spawns an OS thread (not tokio task) because `crossterm::event::poll()`
//! blocks and would starve the async runtime. Drop signals shutdown without joining
//! to avoid deadlocking if `poll` blocks during panic unwinding.

use crate::app::Job;
use crate::app::WorkflowRun;
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug)]
pub enum AppEvent {
    Key(KeyEvent),
    Tick,
    PollResult {
        runs: Vec<WorkflowRun>,
        /// `true` when user pressed 'r' (shows immediate feedback vs background refresh).
        manual: bool,
    },
    JobsResult {
        run_id: u64,
        jobs: Vec<Job>,
    },
    FailedLogResult {
        run_id: u64,
        job_id: Option<u64>,
        title: String,
        content: String,
    },
    ClipboardResult(Result<(), String>),
    RerunSuccess(u64),
    CancelSuccess(u64),
    DeleteSuccess(u64),
    /// Per-run ⚠ icon. Persists until run is refreshed. Use for job-fetch failures etc.
    RunError {
        run_id: u64,
        error: String,
    },
    /// Global toast, auto-dismisses after `ERROR_TTL_SECS`. Use `RunError` for per-run.
    Error(String),
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<AppEvent>,
    tx: mpsc::UnboundedSender<AppEvent>,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let eventtx = tx.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_flag = shutdown.clone();

        let thread = std::thread::spawn(move || {
            while !shutdown_flag.load(Ordering::Relaxed) {
                match event::poll(tick_rate) {
                    Err(e) => {
                        let _ = eventtx.send(AppEvent::Error(format!("Terminal poll error: {e}")));
                        break;
                    }
                    Ok(false) => {
                        if eventtx.send(AppEvent::Tick).is_err() {
                            break;
                        }
                        continue;
                    }
                    Ok(true) => {}
                }
                match event::read() {
                    Ok(CrosstermEvent::Key(key)) => {
                        if eventtx.send(AppEvent::Key(key)).is_err() {
                            break;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                        // EINTR — retry silently
                    }
                    Err(e) => {
                        let _ = eventtx.send(AppEvent::Error(format!("Terminal read error: {e}")));
                        break;
                    }
                    _ => {} // Non-key events (mouse, resize, etc.)
                }
            }
        });

        Self {
            rx,
            tx,
            shutdown,
            thread: Some(thread),
        }
    }

    pub fn sender(&self) -> mpsc::UnboundedSender<AppEvent> {
        self.tx.clone()
    }

    pub async fn next(&mut self) -> Option<AppEvent> {
        self.rx.recv().await
    }

    pub fn stop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.thread.take() {
            if let Err(panic_payload) = handle.join() {
                let msg = panic_payload.downcast::<String>().map_or_else(
                    |p| {
                        p.downcast::<&str>()
                            .map_or_else(|_| "unknown panic".to_string(), |s| s.to_string())
                    },
                    |s| *s,
                );
                tracing::error!("event thread panicked: {msg}");
            }
        }
    }
}

impl Drop for EventHandler {
    fn drop(&mut self) {
        // Only signal shutdown; don't join the thread in Drop to avoid deadlocking
        // if crossterm::event::poll is blocking (e.g. during panic unwinding).
        // The thread will exit on its next poll tick when it checks the shutdown flag.
        self.shutdown.store(true, Ordering::Relaxed);
    }
}
