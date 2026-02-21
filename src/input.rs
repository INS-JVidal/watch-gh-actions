use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Quit,
    DismissError,
    MoveUp,
    MoveDown,
    Expand,
    Collapse,
    Toggle,
    Refresh,
    RerunFailed,
    OpenBrowser,
    CycleFilter,
    FilterBranch,
    QuickSelect(usize),
    ViewLogs,
    CopyToClipboard,
    CloseOverlay,
    ScrollUp,
    ScrollDown,
    PageUp,
    PageDown,
    ScrollToTop,
    ScrollToBottom,
    ShowDetails,
    None,
}

pub fn map_key(
    key: KeyEvent,
    has_error: bool,
    is_loading: bool,
    has_log_overlay: bool,
    has_detail_overlay: bool,
) -> Action {
    if key.kind != KeyEventKind::Press {
        return Action::None;
    }

    // Ctrl+C always quits
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Action::Quit;
    }

    // Log overlay mode
    if has_log_overlay {
        return match key.code {
            KeyCode::Char('j') | KeyCode::Down => Action::ScrollDown,
            KeyCode::Char('k') | KeyCode::Up => Action::ScrollUp,
            KeyCode::PageDown => Action::PageDown,
            KeyCode::PageUp => Action::PageUp,
            KeyCode::Char('g') => Action::ScrollToTop,
            KeyCode::Char('G') => Action::ScrollToBottom,
            KeyCode::Char('y') => Action::CopyToClipboard,
            KeyCode::Char('q' | 'e') | KeyCode::Esc => Action::CloseOverlay,
            _ => Action::None,
        };
    }

    // Detail overlay mode
    if has_detail_overlay {
        return match key.code {
            KeyCode::Char('q' | 'd') | KeyCode::Esc => Action::CloseOverlay,
            _ => Action::None,
        };
    }

    match key.code {
        KeyCode::Char('q') => Action::Quit,
        KeyCode::Esc => {
            if has_error {
                Action::DismissError
            } else {
                Action::Quit
            }
        }
        KeyCode::Up | KeyCode::Char('k') => Action::MoveUp,
        KeyCode::Down | KeyCode::Char('j') => Action::MoveDown,
        KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter => Action::Expand,
        KeyCode::Left | KeyCode::Char('h') => Action::Collapse,
        KeyCode::Char(' ') => Action::Toggle,
        KeyCode::Char('r') if !is_loading => Action::Refresh,
        KeyCode::Char('R') => Action::RerunFailed,
        KeyCode::Char('o') => Action::OpenBrowser,
        KeyCode::Char('e') => Action::ViewLogs,
        KeyCode::Char('f') => Action::CycleFilter,
        KeyCode::Char('b') => Action::FilterBranch,
        KeyCode::Char('d') => Action::ShowDetails,
        KeyCode::Char(c) if c.is_ascii_digit() && c != '0' => {
            Action::QuickSelect((c as u8 - b'0') as usize)
        }
        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn press_with(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn release(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn quit_on_q() {
        assert_eq!(
            map_key(press(KeyCode::Char('q')), false, false, false, false),
            Action::Quit
        );
    }

    #[test]
    fn esc_quits_without_error() {
        assert_eq!(
            map_key(press(KeyCode::Esc), false, false, false, false),
            Action::Quit
        );
    }

    #[test]
    fn esc_dismisses_error_when_present() {
        assert_eq!(
            map_key(press(KeyCode::Esc), true, false, false, false),
            Action::DismissError
        );
    }

    #[test]
    fn ctrl_c_quits() {
        assert_eq!(
            map_key(
                press_with(KeyCode::Char('c'), KeyModifiers::CONTROL),
                false,
                false,
                false,
                false,
            ),
            Action::Quit
        );
    }

    #[test]
    fn move_up_arrow() {
        assert_eq!(
            map_key(press(KeyCode::Up), false, false, false, false),
            Action::MoveUp
        );
    }

    #[test]
    fn move_up_k() {
        assert_eq!(
            map_key(press(KeyCode::Char('k')), false, false, false, false),
            Action::MoveUp
        );
    }

    #[test]
    fn move_down_arrow() {
        assert_eq!(
            map_key(press(KeyCode::Down), false, false, false, false),
            Action::MoveDown
        );
    }

    #[test]
    fn move_down_j() {
        assert_eq!(
            map_key(press(KeyCode::Char('j')), false, false, false, false),
            Action::MoveDown
        );
    }

    #[test]
    fn expand_right_arrow() {
        assert_eq!(
            map_key(press(KeyCode::Right), false, false, false, false),
            Action::Expand
        );
    }

    #[test]
    fn expand_l() {
        assert_eq!(
            map_key(press(KeyCode::Char('l')), false, false, false, false),
            Action::Expand
        );
    }

    #[test]
    fn expand_enter() {
        assert_eq!(
            map_key(press(KeyCode::Enter), false, false, false, false),
            Action::Expand
        );
    }

    #[test]
    fn collapse_left_arrow() {
        assert_eq!(
            map_key(press(KeyCode::Left), false, false, false, false),
            Action::Collapse
        );
    }

    #[test]
    fn collapse_h() {
        assert_eq!(
            map_key(press(KeyCode::Char('h')), false, false, false, false),
            Action::Collapse
        );
    }

    #[test]
    fn toggle_space() {
        assert_eq!(
            map_key(press(KeyCode::Char(' ')), false, false, false, false),
            Action::Toggle
        );
    }

    #[test]
    fn refresh_r() {
        assert_eq!(
            map_key(press(KeyCode::Char('r')), false, false, false, false),
            Action::Refresh
        );
    }

    #[test]
    fn refresh_blocked_while_loading() {
        assert_eq!(
            map_key(press(KeyCode::Char('r')), false, true, false, false),
            Action::None
        );
    }

    #[test]
    fn rerun_failed_capital_r() {
        assert_eq!(
            map_key(press(KeyCode::Char('R')), false, false, false, false),
            Action::RerunFailed
        );
    }

    #[test]
    fn open_browser_o() {
        assert_eq!(
            map_key(press(KeyCode::Char('o')), false, false, false, false),
            Action::OpenBrowser
        );
    }

    #[test]
    fn cycle_filter_f() {
        assert_eq!(
            map_key(press(KeyCode::Char('f')), false, false, false, false),
            Action::CycleFilter
        );
    }

    #[test]
    fn filter_branch_b() {
        assert_eq!(
            map_key(press(KeyCode::Char('b')), false, false, false, false),
            Action::FilterBranch
        );
    }

    #[test]
    fn quick_select_digits_1_to_9() {
        for d in 1..=9u8 {
            let c = (b'0' + d) as char;
            assert_eq!(
                map_key(press(KeyCode::Char(c)), false, false, false, false),
                Action::QuickSelect(d as usize)
            );
        }
    }

    #[test]
    fn digit_zero_returns_none() {
        assert_eq!(
            map_key(press(KeyCode::Char('0')), false, false, false, false),
            Action::None
        );
    }

    #[test]
    fn unbound_key_returns_none() {
        assert_eq!(
            map_key(press(KeyCode::Char('z')), false, false, false, false),
            Action::None
        );
    }

    #[test]
    fn non_press_event_filtered() {
        assert_eq!(
            map_key(release(KeyCode::Char('q')), false, false, false, false),
            Action::None
        );
    }

    #[test]
    fn view_logs_e() {
        assert_eq!(
            map_key(press(KeyCode::Char('e')), false, false, false, false),
            Action::ViewLogs
        );
    }

    // --- Overlay mode tests ---

    #[test]
    fn overlay_scroll_down_j() {
        assert_eq!(
            map_key(press(KeyCode::Char('j')), false, false, true, false),
            Action::ScrollDown
        );
    }

    #[test]
    fn overlay_scroll_up_k() {
        assert_eq!(
            map_key(press(KeyCode::Char('k')), false, false, true, false),
            Action::ScrollUp
        );
    }

    #[test]
    fn overlay_page_down() {
        assert_eq!(
            map_key(press(KeyCode::PageDown), false, false, true, false),
            Action::PageDown
        );
    }

    #[test]
    fn overlay_page_up() {
        assert_eq!(
            map_key(press(KeyCode::PageUp), false, false, true, false),
            Action::PageUp
        );
    }

    #[test]
    fn overlay_scroll_to_top_g() {
        assert_eq!(
            map_key(press(KeyCode::Char('g')), false, false, true, false),
            Action::ScrollToTop
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn overlay_scroll_to_bottom_G() {
        assert_eq!(
            map_key(press(KeyCode::Char('G')), false, false, true, false),
            Action::ScrollToBottom
        );
    }

    #[test]
    fn overlay_copy_y() {
        assert_eq!(
            map_key(press(KeyCode::Char('y')), false, false, true, false),
            Action::CopyToClipboard
        );
    }

    #[test]
    fn overlay_close_q() {
        assert_eq!(
            map_key(press(KeyCode::Char('q')), false, false, true, false),
            Action::CloseOverlay
        );
    }

    #[test]
    fn overlay_close_esc() {
        assert_eq!(
            map_key(press(KeyCode::Esc), false, false, true, false),
            Action::CloseOverlay
        );
    }

    #[test]
    fn overlay_close_e() {
        assert_eq!(
            map_key(press(KeyCode::Char('e')), false, false, true, false),
            Action::CloseOverlay
        );
    }

    #[test]
    fn overlay_ctrl_c_quits() {
        assert_eq!(
            map_key(
                press_with(KeyCode::Char('c'), KeyModifiers::CONTROL),
                false,
                false,
                true,
                false,
            ),
            Action::Quit
        );
    }

    // --- Detail overlay mode tests ---

    #[test]
    fn show_details_d() {
        assert_eq!(
            map_key(press(KeyCode::Char('d')), false, false, false, false),
            Action::ShowDetails
        );
    }

    #[test]
    fn detail_overlay_close_d() {
        assert_eq!(
            map_key(press(KeyCode::Char('d')), false, false, false, true),
            Action::CloseOverlay
        );
    }

    #[test]
    fn detail_overlay_close_q() {
        assert_eq!(
            map_key(press(KeyCode::Char('q')), false, false, false, true),
            Action::CloseOverlay
        );
    }

    #[test]
    fn detail_overlay_close_esc() {
        assert_eq!(
            map_key(press(KeyCode::Esc), false, false, false, true),
            Action::CloseOverlay
        );
    }

    #[test]
    fn detail_overlay_ctrl_c_quits() {
        assert_eq!(
            map_key(
                press_with(KeyCode::Char('c'), KeyModifiers::CONTROL),
                false,
                false,
                false,
                true,
            ),
            Action::Quit
        );
    }
}
