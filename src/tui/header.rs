use crate::app::AppState;
use crate::tui::spinner;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn render(f: &mut Frame, area: Rect, state: &AppState) {
    let mut spans = vec![
        Span::styled(
            format!(
                " ghw v{}+{} ",
                env!("CARGO_PKG_VERSION"),
                env!("BUILD_NUMBER")
            ),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("│ "),
        Span::styled(
            &state.config.repo,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    if let Some(branch) = &state.config.branch {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("[{branch}]"),
            Style::default().fg(Color::Yellow),
        ));
    }

    // Filter indicator
    let filter_text = match state.filter {
        crate::app::FilterMode::All => "",
        crate::app::FilterMode::ActiveOnly => " [active]",
        crate::app::FilterMode::CurrentBranch => " [branch]",
    };
    if !filter_text.is_empty() {
        spans.push(Span::styled(
            filter_text,
            Style::default().fg(Color::Magenta),
        ));
    }

    // Loading spinner or poll countdown
    if state.is_loading {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("{}", spinner::frame(state.spinner_frame)),
            Style::default().fg(Color::Yellow),
        ));
    } else if state.next_poll_in > 0 {
        // Right-align the countdown
        let countdown = format!(" {}s", state.next_poll_in);
        spans.push(Span::styled(
            countdown,
            Style::default().fg(Color::DarkGray),
        ));
    }

    // Error indicator
    if state.error_message().is_some() {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            "!",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ));
    }

    let header = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    f.render_widget(header, area);
}
