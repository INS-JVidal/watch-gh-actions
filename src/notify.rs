use crate::app::{Conclusion, WorkflowRun};
use notify_rust::{Notification, Urgency};
use std::io::Write;

pub fn send_desktop(run: &WorkflowRun) {
    let (summary, icon, urgency) = match run.conclusion {
        Some(Conclusion::Success) => ("CI Passed", "dialog-information", Urgency::Normal),
        Some(Conclusion::Failure) => ("CI Failed", "dialog-error", Urgency::Critical),
        _ => ("CI Finished", "dialog-information", Urgency::Normal),
    };

    let body = match run.conclusion {
        Some(Conclusion::Success) | Some(Conclusion::Failure) => run.display_title.clone(),
        Some(ref c) => format!("{} ({:?})", run.display_title, c),
        None => run.display_title.clone(),
    };

    let result = Notification::new()
        .summary(summary)
        .body(&body)
        .icon(icon)
        .urgency(urgency)
        .show();

    if result.is_err() {
        let _ = std::io::stdout().write_all(b"\x07");
        let _ = std::io::stdout().flush();
    }
}
