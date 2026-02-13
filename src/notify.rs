use crate::app::{Conclusion, WorkflowRun};
use notify_rust::{Notification, Urgency};

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

    let _ = Notification::new()
        .summary(summary)
        .body(&body)
        .icon(icon)
        .urgency(urgency)
        .show();
}
