#[cfg(feature = "desktop-notify")]
use crate::app::{Conclusion, WorkflowRun};
#[cfg(feature = "desktop-notify")]
use notify_rust::{Notification, Urgency};

/// Attempt to send a desktop notification for a completed run.
/// Returns `Some(error_message)` on failure, `None` on success.
#[cfg(feature = "desktop-notify")]
pub fn send_desktop(run: &WorkflowRun) -> Option<String> {
    let (summary, icon, urgency) = match run.conclusion {
        Some(Conclusion::Success) => ("CI Passed", "dialog-information", Urgency::Normal),
        Some(Conclusion::Failure) => ("CI Failed", "dialog-error", Urgency::Critical),
        _ => ("CI Finished", "dialog-information", Urgency::Normal),
    };

    let body = match run.conclusion {
        Some(Conclusion::Success | Conclusion::Failure) | None => run.display_title.clone(),
        Some(ref c) => format!("{} ({c:?})", run.display_title),
    };

    match Notification::new()
        .summary(summary)
        .body(&body)
        .icon(icon)
        .urgency(urgency)
        .show()
    {
        Ok(_) => None,
        Err(e) => Some(format!("Desktop notification failed: {e}")),
    }
}

#[cfg(not(feature = "desktop-notify"))]
pub fn send_desktop(_run: &crate::app::WorkflowRun) -> Option<String> {
    // Desktop notifications disabled at compile time
    None
}
