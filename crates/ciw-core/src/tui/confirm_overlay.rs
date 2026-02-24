use crate::app::ConfirmOverlay;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

pub fn render(f: &mut Frame, overlay: &ConfirmOverlay) {
    let area = f.area();

    let width = 40u16.min(area.width);
    let height = 7u16.min(area.height);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let overlay_area = Rect::new(x, y, width, height);

    f.render_widget(Clear, overlay_area);

    let title = format!(" {} ", overlay.title);
    let hints = Line::from(vec![
        Span::styled(
            "y",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" confirm   ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "n",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" cancel ", Style::default().fg(Color::DarkGray)),
    ]);

    let block = Block::default()
        .title(title)
        .title_bottom(hints.centered())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .style(Style::default().bg(Color::Black));

    let message = Line::from(Span::styled(
        &overlay.message,
        Style::default().fg(Color::White),
    ));

    let paragraph = Paragraph::new(vec![Line::from(""), message, Line::from("")])
        .block(block)
        .centered();
    f.render_widget(paragraph, overlay_area);
}
