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
    ClipboardResult(bool),
    RerunSuccess(u64),
    RunError {
        run_id: u64,
        error: String,
    },
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
                if let Ok(CrosstermEvent::Key(key)) = event::read() {
                    if eventtx.send(AppEvent::Key(key)).is_err() {
                        break;
                    }
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
            let _ = handle.join();
        }
    }
}

impl Drop for EventHandler {
    fn drop(&mut self) {
        self.stop();
    }
}
