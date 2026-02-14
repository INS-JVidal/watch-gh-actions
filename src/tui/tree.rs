use crate::app::{AppState, Conclusion, ResolvedItem, RunStatus, TreeLevel};
use chrono::Utc;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let narrow = area.width < crate::app::NARROW_WIDTH_THRESHOLD;
    let inner_width = area.width.saturating_sub(2) as usize;

    if state.tree_items.is_empty() && !state.is_loading {
        let msg = match state.filter {
            crate::app::FilterMode::ActiveOnly => "No active runs",
            crate::app::FilterMode::CurrentBranch => "No runs for current branch",
            crate::app::FilterMode::All => "No workflow runs found",
        };
        let para = Paragraph::new(msg)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::NONE));
        f.render_widget(para, area);
        return;
    }

    // Calculate visible window (scroll)
    let visible_height = area.height as usize;
    let scroll_offset = if state.cursor >= visible_height {
        state.cursor - visible_height + 1
    } else {
        0
    };

    // Count which visual run index each run_idx corresponds to
    // (for quick-select labels)
    let mut run_visual_idx: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();
    let mut visual = 0;
    for item in &state.tree_items {
        if item.level == TreeLevel::Run {
            run_visual_idx.insert(item.run_idx, visual);
            visual += 1;
        }
    }

    let mut lines: Vec<Line> = Vec::new();

    for (i, item) in state
        .tree_items
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_height)
    {
        let is_selected = i == state.cursor;
        let has_notification = state.notifications.iter().any(|n| {
            state
                .runs
                .get(item.run_idx)
                .is_some_and(|r| r.database_id == n.run_id)
        });

        let line = if item.level == TreeLevel::Loading {
            render_loading_line(state.spinner_frame, is_selected)
        } else {
            match state.resolve_item(item) {
                Some(ResolvedItem::Run(run)) => {
                    let vis_idx = run_visual_idx.get(&item.run_idx).copied().unwrap_or(0);
                    render_run_line(
                        run,
                        vis_idx,
                        is_selected,
                        has_notification,
                        narrow,
                        inner_width,
                        item.expanded,
                    )
                }
                Some(ResolvedItem::Job(job)) => {
                    render_job_line(job, is_selected, narrow, inner_width, item.expanded)
                }
                Some(ResolvedItem::Step(step)) => render_step_line(step, is_selected, inner_width),
                None => Line::raw(""),
            }
        };
        lines.push(line);
    }

    let tree = Paragraph::new(lines).block(Block::default().borders(Borders::NONE));
    f.render_widget(tree, area);
}

fn status_icon(status: RunStatus, conclusion: Option<Conclusion>) -> (&'static str, Color) {
    match (status, conclusion) {
        (RunStatus::Completed, Some(Conclusion::Success)) => ("✓", Color::Green),
        (RunStatus::Completed, Some(Conclusion::Failure)) => ("✗", Color::Red),
        (RunStatus::Completed, Some(Conclusion::Cancelled)) => ("⊘", Color::Yellow),
        (RunStatus::Completed, Some(Conclusion::Skipped)) => ("⊘", Color::DarkGray),
        (RunStatus::Completed, Some(Conclusion::TimedOut)) => ("✗", Color::Red),
        (RunStatus::Completed, _) => ("·", Color::DarkGray),
        (RunStatus::InProgress, _) => ("⟳", Color::Yellow),
        (RunStatus::Queued | RunStatus::Waiting | RunStatus::Pending | RunStatus::Requested, _)
        | (RunStatus::Unknown, _) => ("·", Color::DarkGray),
    }
}

fn format_duration(secs: i64) -> String {
    let secs = secs.max(0);
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

fn truncate(s: &str, max_width: usize) -> String {
    if UnicodeWidthStr::width(s) <= max_width {
        s.to_string()
    } else {
        let mut result = String::new();
        let mut width = 0;
        for c in s.chars() {
            let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
            if width + cw + 1 > max_width {
                result.push('…');
                break;
            }
            result.push(c);
            width += cw;
        }
        result
    }
}

fn render_run_line(
    run: &crate::app::WorkflowRun,
    visual_idx: usize,
    is_selected: bool,
    has_notification: bool,
    narrow: bool,
    max_width: usize,
    expanded: bool,
) -> Line<'static> {
    let (icon, icon_color) = status_icon(run.status, run.conclusion);
    let arrow = if expanded { "▼" } else { "▶" };

    let number = format!("#{}", run.number);
    let duration = {
        let elapsed = Utc::now().signed_duration_since(run.created_at);
        format_duration(elapsed.num_seconds())
    };

    let icon_display_width = UnicodeWidthStr::width(icon);
    let arrow_display_width = UnicodeWidthStr::width(arrow);
    let prefix_width = 1 + arrow_display_width + 1 + icon_display_width + 1 + number.len() + 1;
    let suffix_width = if narrow { 0 } else { duration.len() + 1 };
    let title_max = max_width.saturating_sub(prefix_width + suffix_width + 2);
    let title = truncate(&run.display_title, title_max);

    let select_style = if is_selected {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };

    let notif_marker = if has_notification { "★ " } else { "" };
    let idx_label = if visual_idx < crate::app::QUICK_SELECT_MAX {
        format!("{}", visual_idx + 1)
    } else {
        " ".to_string()
    };

    let mut spans = vec![
        Span::styled(
            format!("{}{} {} ", idx_label, arrow, icon),
            Style::default().fg(icon_color),
        ),
        Span::styled(format!("{} ", number), Style::default().fg(Color::DarkGray)),
        Span::styled(notif_marker.to_string(), Style::default().fg(Color::Yellow)),
        Span::styled(title, select_style),
    ];

    if !narrow {
        spans.push(Span::styled(
            format!(" {}", duration),
            Style::default().fg(Color::DarkGray),
        ));
    }

    if !narrow {
        spans.push(Span::styled(
            format!(" {}", run.head_branch),
            Style::default().fg(Color::Blue),
        ));
    }

    Line::from(spans)
}

fn render_job_line(
    job: &crate::app::Job,
    is_selected: bool,
    _narrow: bool,
    max_width: usize,
    expanded: bool,
) -> Line<'static> {
    let (icon, icon_color) = status_icon(job.status, job.conclusion);
    let arrow = if expanded { "▼" } else { "▶" };

    let duration = match (job.started_at, job.completed_at) {
        (Some(start), Some(end)) => {
            let d = end.signed_duration_since(start);
            format_duration(d.num_seconds())
        }
        (Some(start), None) => {
            let d = Utc::now().signed_duration_since(start);
            format_duration(d.num_seconds())
        }
        _ => String::new(),
    };

    let prefix = format!("    {} {} ", arrow, icon);
    let prefix_display_width = UnicodeWidthStr::width(prefix.as_str());
    let suffix_width = if duration.is_empty() {
        0
    } else {
        duration.len() + 1
    };
    let name_max = max_width.saturating_sub(prefix_display_width + suffix_width);
    let name = truncate(&job.name, name_max);

    let select_style = if is_selected {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };

    let mut spans = vec![
        Span::styled(prefix, Style::default().fg(icon_color)),
        Span::styled(name, select_style),
    ];

    if !duration.is_empty() {
        spans.push(Span::styled(
            format!(" {}", duration),
            Style::default().fg(Color::DarkGray),
        ));
    }

    Line::from(spans)
}

fn render_step_line(step: &crate::app::Step, is_selected: bool, max_width: usize) -> Line<'static> {
    let (icon, icon_color) = status_icon(step.status, step.conclusion);

    let prefix = format!("        {} ", icon);
    let prefix_display_width = UnicodeWidthStr::width(prefix.as_str());
    let name_max = max_width.saturating_sub(prefix_display_width);
    let name = truncate(&step.name, name_max);

    let select_style = if is_selected {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };

    Line::from(vec![
        Span::styled(prefix, Style::default().fg(icon_color)),
        Span::styled(name, select_style),
    ])
}

fn render_loading_line(spinner_frame: usize, is_selected: bool) -> Line<'static> {
    let spinner_char = crate::tui::spinner::frame(spinner_frame);
    let select_style = if is_selected {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };
    Line::from(vec![
        Span::styled(
            format!("    {} ", spinner_char),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled("Loading…", select_style.fg(Color::DarkGray)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- format_duration ---

    #[test]
    fn duration_zero() {
        assert_eq!(format_duration(0), "0s");
    }

    #[test]
    fn duration_negative() {
        assert_eq!(format_duration(-10), "0s");
    }

    #[test]
    fn duration_seconds() {
        assert_eq!(format_duration(45), "45s");
    }

    #[test]
    fn duration_one_minute() {
        assert_eq!(format_duration(60), "1m 0s");
    }

    #[test]
    fn duration_minutes_and_seconds() {
        assert_eq!(format_duration(125), "2m 5s");
    }

    #[test]
    fn duration_one_hour() {
        assert_eq!(format_duration(3600), "1h 0m");
    }

    #[test]
    fn duration_hours_and_minutes() {
        assert_eq!(format_duration(3725), "1h 2m");
    }

    #[test]
    fn duration_large() {
        assert_eq!(format_duration(86400), "24h 0m");
    }

    // --- truncate ---

    #[test]
    fn truncate_short_unchanged() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_exact_length_unchanged() {
        assert_eq!(truncate("hello", 5), "hello");
    }

    #[test]
    fn truncate_long_adds_ellipsis() {
        let result = truncate("hello world", 6);
        assert!(result.contains('…'));
        assert!(result.len() <= 10); // byte len, not char width
    }

    #[test]
    fn truncate_empty_string() {
        assert_eq!(truncate("", 10), "");
    }

    #[test]
    fn truncate_zero_width() {
        let result = truncate("hello", 0);
        assert_eq!(result, "…");
    }

    #[test]
    fn truncate_cjk_characters() {
        // CJK chars are 2-width each
        let result = truncate("你好世界test", 6);
        assert!(result.contains('…'));
    }

    #[test]
    fn truncate_max_width_one() {
        let result = truncate("hello", 1);
        assert_eq!(result, "…");
    }

    // --- status_icon ---

    #[test]
    fn icon_completed_success() {
        let (icon, color) = status_icon(RunStatus::Completed, Some(Conclusion::Success));
        assert_eq!(icon, "✓");
        assert_eq!(color, Color::Green);
    }

    #[test]
    fn icon_completed_failure() {
        let (icon, color) = status_icon(RunStatus::Completed, Some(Conclusion::Failure));
        assert_eq!(icon, "✗");
        assert_eq!(color, Color::Red);
    }

    #[test]
    fn icon_completed_cancelled() {
        let (icon, color) = status_icon(RunStatus::Completed, Some(Conclusion::Cancelled));
        assert_eq!(icon, "⊘");
        assert_eq!(color, Color::Yellow);
    }

    #[test]
    fn icon_completed_skipped() {
        let (icon, color) = status_icon(RunStatus::Completed, Some(Conclusion::Skipped));
        assert_eq!(icon, "⊘");
        assert_eq!(color, Color::DarkGray);
    }

    #[test]
    fn icon_completed_timed_out() {
        let (icon, color) = status_icon(RunStatus::Completed, Some(Conclusion::TimedOut));
        assert_eq!(icon, "✗");
        assert_eq!(color, Color::Red);
    }

    #[test]
    fn icon_in_progress() {
        let (icon, color) = status_icon(RunStatus::InProgress, None);
        assert_eq!(icon, "⟳");
        assert_eq!(color, Color::Yellow);
    }

    #[test]
    fn icon_queued_and_unknown() {
        for status in [RunStatus::Queued, RunStatus::Waiting, RunStatus::Pending, RunStatus::Unknown] {
            let (icon, color) = status_icon(status, None);
            assert_eq!(icon, "·");
            assert_eq!(color, Color::DarkGray);
        }
    }
}
