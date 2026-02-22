use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::AppState;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let narrow = area.width < crate::app::NARROW_WIDTH_THRESHOLD;

    let hints: &[(&str, &str)] = if state.has_log_overlay() {
        &[("j/k", "scroll"), ("y", "copy"), ("q", "close")]
    } else if state.has_detail_overlay() {
        &[("d/q", "close")]
    } else if state.has_confirm_overlay() {
        &[("y", "confirm"), ("n", "cancel")]
    } else if narrow {
        &[
            ("j/k", "nav"),
            ("l/h", "exp/col"),
            ("d", "detail"),
            ("e", "err"),
            ("o", "open"),
            ("c", "cancel"),
            ("x", "delete"),
            ("r", "refresh"),
            ("q", "quit"),
        ]
    } else {
        &[
            ("↑↓/jk", "navigate"),
            ("→/l/Enter", "expand"),
            ("←/h", "collapse"),
            ("d", "details"),
            ("e", "errors"),
            ("o", "open"),
            ("c", "cancel"),
            ("x", "delete"),
            ("r", "refresh"),
            ("R", "rerun"),
            ("f", "filter"),
            ("q", "quit"),
        ]
    };

    // Notification display
    let line = if let Some(notif) = state.notifications.last() {
        Line::from(vec![
            Span::styled("★ ", Style::default().fg(Color::Yellow)),
            Span::styled(&notif.message, Style::default().fg(Color::Yellow)),
        ])
    } else {
        let mut spans: Vec<Span> = Vec::new();
        for (i, (key, desc)) in hints.iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw(" "));
            }
            spans.push(Span::styled(*key, Style::default().fg(Color::Cyan)));
            spans.push(Span::styled(
                format!(" {desc}"),
                Style::default().fg(Color::DarkGray),
            ));
        }
        Line::from(spans)
    };

    let footer = Paragraph::new(line).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(footer, area);
}
