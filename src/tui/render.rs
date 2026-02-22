use crate::app::AppState;
use crate::tui::{footer, header, tree};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::Frame;

pub fn render(f: &mut Frame, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // header
            Constraint::Min(1),    // tree
            Constraint::Length(2), // footer
        ])
        .split(f.area());

    header::render(f, chunks[0], state);
    tree::render(f, chunks[1], state);
    footer::render(f, chunks[2], state);

    // Error overlay
    if let Some(err) = state.error_message() {
        let area = f.area();
        if area.height > 6 && area.width >= 4 {
            use ratatui::layout::Rect;
            use ratatui::style::{Color, Style};
            use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
            let err_area = Rect {
                x: area.x + 1,
                y: area.y + area.height.saturating_sub(5),
                width: area.width.saturating_sub(2),
                height: 3,
            };
            let err_widget = Paragraph::new(err.to_owned())
                .style(Style::default().fg(Color::Red))
                .block(
                    Block::default()
                        .title(" Error ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Red)),
                )
                .wrap(Wrap { trim: true });
            f.render_widget(err_widget, err_area);
        }
    }

    // Overlay (drawn on top of everything)
    match &state.overlay {
        crate::app::ActiveOverlay::Log(overlay) => {
            crate::tui::log_overlay::render(f, overlay);
        }
        crate::app::ActiveOverlay::Detail(overlay) => {
            crate::tui::detail_overlay::render(f, overlay);
        }
        crate::app::ActiveOverlay::Confirm(ref overlay) => {
            crate::tui::confirm_overlay::render(f, overlay);
        }
        crate::app::ActiveOverlay::None => {}
    }
}
