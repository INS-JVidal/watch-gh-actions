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

/// Which overlay (if any) is currently displayed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverlayMode {
    #[default]
    None,
    Log,
    Detail,
}

/// Captures the UI state needed to interpret a key press.
#[derive(Debug, Clone, Default)]
pub struct InputContext {
    pub has_error: bool,
    pub is_loading: bool,
    pub overlay: OverlayMode,
}

pub fn map_key(key: KeyEvent, ctx: &InputContext) -> Action {
    let has_log_overlay = ctx.overlay == OverlayMode::Log;
    let has_detail_overlay = ctx.overlay == OverlayMode::Detail;
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
            if ctx.has_error {
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
        KeyCode::Char('r') if !ctx.is_loading => Action::Refresh,
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

    fn ctx() -> InputContext {
        InputContext::default()
    }

    fn ctx_error() -> InputContext {
        InputContext { has_error: true, ..Default::default() }
    }

    fn ctx_loading() -> InputContext {
        InputContext { is_loading: true, ..Default::default() }
    }

    fn ctx_log() -> InputContext {
        InputContext { overlay: OverlayMode::Log, ..Default::default() }
    }

    fn ctx_detail() -> InputContext {
        InputContext { overlay: OverlayMode::Detail, ..Default::default() }
    }

    #[test]
    fn quit_on_q() {
        assert_eq!(map_key(press(KeyCode::Char('q')), &ctx()), Action::Quit);
    }

    #[test]
    fn esc_quits_without_error() {
        assert_eq!(map_key(press(KeyCode::Esc), &ctx()), Action::Quit);
    }

    #[test]
    fn esc_dismisses_error_when_present() {
        assert_eq!(map_key(press(KeyCode::Esc), &ctx_error()), Action::DismissError);
    }

    #[test]
    fn ctrl_c_quits() {
        assert_eq!(
            map_key(press_with(KeyCode::Char('c'), KeyModifiers::CONTROL), &ctx()),
            Action::Quit
        );
    }

    #[test]
    fn move_up_arrow() {
        assert_eq!(map_key(press(KeyCode::Up), &ctx()), Action::MoveUp);
    }

    #[test]
    fn move_up_k() {
        assert_eq!(map_key(press(KeyCode::Char('k')), &ctx()), Action::MoveUp);
    }

    #[test]
    fn move_down_arrow() {
        assert_eq!(map_key(press(KeyCode::Down), &ctx()), Action::MoveDown);
    }

    #[test]
    fn move_down_j() {
        assert_eq!(map_key(press(KeyCode::Char('j')), &ctx()), Action::MoveDown);
    }

    #[test]
    fn expand_right_arrow() {
        assert_eq!(map_key(press(KeyCode::Right), &ctx()), Action::Expand);
    }

    #[test]
    fn expand_l() {
        assert_eq!(map_key(press(KeyCode::Char('l')), &ctx()), Action::Expand);
    }

    #[test]
    fn expand_enter() {
        assert_eq!(map_key(press(KeyCode::Enter), &ctx()), Action::Expand);
    }

    #[test]
    fn collapse_left_arrow() {
        assert_eq!(map_key(press(KeyCode::Left), &ctx()), Action::Collapse);
    }

    #[test]
    fn collapse_h() {
        assert_eq!(map_key(press(KeyCode::Char('h')), &ctx()), Action::Collapse);
    }

    #[test]
    fn toggle_space() {
        assert_eq!(map_key(press(KeyCode::Char(' ')), &ctx()), Action::Toggle);
    }

    #[test]
    fn refresh_r() {
        assert_eq!(map_key(press(KeyCode::Char('r')), &ctx()), Action::Refresh);
    }

    #[test]
    fn refresh_blocked_while_loading() {
        assert_eq!(map_key(press(KeyCode::Char('r')), &ctx_loading()), Action::None);
    }

    #[test]
    fn rerun_failed_capital_r() {
        assert_eq!(map_key(press(KeyCode::Char('R')), &ctx()), Action::RerunFailed);
    }

    #[test]
    fn open_browser_o() {
        assert_eq!(map_key(press(KeyCode::Char('o')), &ctx()), Action::OpenBrowser);
    }

    #[test]
    fn cycle_filter_f() {
        assert_eq!(map_key(press(KeyCode::Char('f')), &ctx()), Action::CycleFilter);
    }

    #[test]
    fn filter_branch_b() {
        assert_eq!(map_key(press(KeyCode::Char('b')), &ctx()), Action::FilterBranch);
    }

    #[test]
    fn quick_select_digits_1_to_9() {
        for d in 1..=9u8 {
            let c = (b'0' + d) as char;
            assert_eq!(
                map_key(press(KeyCode::Char(c)), &ctx()),
                Action::QuickSelect(d as usize)
            );
        }
    }

    #[test]
    fn digit_zero_returns_none() {
        assert_eq!(map_key(press(KeyCode::Char('0')), &ctx()), Action::None);
    }

    #[test]
    fn unbound_key_returns_none() {
        assert_eq!(map_key(press(KeyCode::Char('z')), &ctx()), Action::None);
    }

    #[test]
    fn non_press_event_filtered() {
        assert_eq!(map_key(release(KeyCode::Char('q')), &ctx()), Action::None);
    }

    #[test]
    fn view_logs_e() {
        assert_eq!(map_key(press(KeyCode::Char('e')), &ctx()), Action::ViewLogs);
    }

    // --- Overlay mode tests ---

    #[test]
    fn overlay_scroll_down_j() {
        assert_eq!(map_key(press(KeyCode::Char('j')), &ctx_log()), Action::ScrollDown);
    }

    #[test]
    fn overlay_scroll_up_k() {
        assert_eq!(map_key(press(KeyCode::Char('k')), &ctx_log()), Action::ScrollUp);
    }

    #[test]
    fn overlay_page_down() {
        assert_eq!(map_key(press(KeyCode::PageDown), &ctx_log()), Action::PageDown);
    }

    #[test]
    fn overlay_page_up() {
        assert_eq!(map_key(press(KeyCode::PageUp), &ctx_log()), Action::PageUp);
    }

    #[test]
    fn overlay_scroll_to_top_g() {
        assert_eq!(map_key(press(KeyCode::Char('g')), &ctx_log()), Action::ScrollToTop);
    }

    #[test]
    #[allow(non_snake_case)]
    fn overlay_scroll_to_bottom_G() {
        assert_eq!(map_key(press(KeyCode::Char('G')), &ctx_log()), Action::ScrollToBottom);
    }

    #[test]
    fn overlay_copy_y() {
        assert_eq!(map_key(press(KeyCode::Char('y')), &ctx_log()), Action::CopyToClipboard);
    }

    #[test]
    fn overlay_close_q() {
        assert_eq!(map_key(press(KeyCode::Char('q')), &ctx_log()), Action::CloseOverlay);
    }

    #[test]
    fn overlay_close_esc() {
        assert_eq!(map_key(press(KeyCode::Esc), &ctx_log()), Action::CloseOverlay);
    }

    #[test]
    fn overlay_close_e() {
        assert_eq!(map_key(press(KeyCode::Char('e')), &ctx_log()), Action::CloseOverlay);
    }

    #[test]
    fn overlay_ctrl_c_quits() {
        assert_eq!(
            map_key(press_with(KeyCode::Char('c'), KeyModifiers::CONTROL), &ctx_log()),
            Action::Quit
        );
    }

    // --- Detail overlay mode tests ---

    #[test]
    fn show_details_d() {
        assert_eq!(map_key(press(KeyCode::Char('d')), &ctx()), Action::ShowDetails);
    }

    #[test]
    fn detail_overlay_close_d() {
        assert_eq!(map_key(press(KeyCode::Char('d')), &ctx_detail()), Action::CloseOverlay);
    }

    #[test]
    fn detail_overlay_close_q() {
        assert_eq!(map_key(press(KeyCode::Char('q')), &ctx_detail()), Action::CloseOverlay);
    }

    #[test]
    fn detail_overlay_close_esc() {
        assert_eq!(map_key(press(KeyCode::Esc), &ctx_detail()), Action::CloseOverlay);
    }

    #[test]
    fn detail_overlay_ctrl_c_quits() {
        assert_eq!(
            map_key(press_with(KeyCode::Char('c'), KeyModifiers::CONTROL), &ctx_detail()),
            Action::Quit
        );
    }
}
