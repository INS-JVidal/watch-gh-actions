use crate::app::DetailOverlay;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

pub fn render(f: &mut Frame, overlay: &DetailOverlay) {
    let area = f.area();

    // Size: ~60% width, height based on content
    // +2 border +1 bottom hint; cap before casting to u16 to avoid wrapping
    let content_height = (overlay.lines.len().min(u16::MAX as usize - 3) as u16).saturating_add(3);
    let width = (area.width * 6 / 10).max(30).min(area.width);
    let height = content_height.max(5).min(area.height);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let overlay_area = Rect::new(x, y, width, height);

    f.render_widget(Clear, overlay_area);

    let title = format!(" {} ", overlay.title);
    let hints = " d/q/Esc close ";

    let block = Block::default()
        .title(title)
        .title_bottom(Line::from(hints).centered())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(Color::Black));

    let inner_width = width.saturating_sub(2) as usize;
    let label_width = overlay
        .lines
        .iter()
        .map(|(l, _)| UnicodeWidthStr::width(l.as_str()))
        .max()
        .unwrap_or(0);

    let lines: Vec<Line> = overlay
        .lines
        .iter()
        .map(|(label, value)| {
            let label_col_width = label_width.saturating_add(2);
            let value_max = inner_width.saturating_sub(label_col_width);
            Line::from(vec![
                Span::styled(
                    format!("{label:>label_width$}  "),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    truncate_value(value, value_max),
                    Style::default().fg(Color::White),
                ),
            ])
        })
        .collect();

    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, overlay_area);
}

fn truncate_value(s: &str, max: usize) -> String {
    let w = UnicodeWidthStr::width(s);
    if w <= max {
        s.to_string()
    } else if max == 0 {
        String::new()
    } else {
        let mut result = String::new();
        let mut width = 0;
        for c in s.chars() {
            let cw = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
            if width + cw + 1 > max {
                result.push('…');
                break;
            }
            result.push(c);
            width += cw;
        }
        result
    }
}
