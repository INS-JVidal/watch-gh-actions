#[cfg(feature = "desktop-notify")]
use crate::app::{Conclusion, WorkflowRun};
#[cfg(feature = "desktop-notify")]
use notify_rust::{Notification, Urgency};
#[cfg(feature = "desktop-notify")]
use std::io::Write;

#[cfg(feature = "desktop-notify")]
pub fn send_desktop(run: &WorkflowRun) {
    let (summary, icon, urgency) = match run.conclusion {
        Some(Conclusion::Success) => ("CI Passed", "dialog-information", Urgency::Normal),
        Some(Conclusion::Failure) => ("CI Failed", "dialog-error", Urgency::Critical),
        _ => ("CI Finished", "dialog-information", Urgency::Normal),
    };

    let body = match run.conclusion {
        Some(Conclusion::Success | Conclusion::Failure) | None => run.display_title.clone(),
        Some(ref c) => format!("{} ({c:?})", run.display_title),
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

#[cfg(not(feature = "desktop-notify"))]
pub fn send_desktop(_run: &crate::app::WorkflowRun) {
    // Desktop notifications disabled at compile time
}
