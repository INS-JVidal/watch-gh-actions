use crate::app::LogOverlay;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

pub fn render(f: &mut Frame, overlay: &LogOverlay) {
    let area = f.area();

    // ~90% width, ~80% height, centered
    let width = (area.width * 9 / 10).max(area.width.min(20)).min(area.width);
    let height = (area.height * 8 / 10).max(6).min(area.height);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let overlay_area = Rect::new(x, y, width, height);

    // Clear the area behind the overlay
    f.render_widget(Clear, overlay_area);

    // Inner height (minus border top/bottom)
    let inner_height = height.saturating_sub(2) as usize;

    // Scroll position info
    let total = overlay.lines.len();
    let scroll_info = if total > inner_height {
        format!(
            " [{}-{}/{}] ",
            overlay.scroll + 1,
            (overlay.scroll + inner_height).min(total),
            total,
        )
    } else {
        String::new()
    };

    let title = format!(" {} {}", overlay.title, scroll_info);
    let hints = " j/k scroll | y copy | q close ";

    let block = Block::default()
        .title(title)
        .title_bottom(Line::from(hints).centered())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .style(Style::default().bg(Color::Black));

    // Build visible lines
    let visible_lines: Vec<Line> = overlay
        .lines
        .iter()
        .skip(overlay.scroll)
        .take(inner_height)
        .map(|l| Line::from(Span::raw(l.as_str())))
        .collect();

    let paragraph = Paragraph::new(visible_lines)
        .block(block)
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, overlay_area);
}
