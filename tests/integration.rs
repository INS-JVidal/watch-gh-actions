mod fixtures;

use fixtures::*;
use ghw::app::{AppState, Conclusion, FilterMode, ResolvedItem, RunStatus, TreeLevel};
use ghw::diff;
use ghw::gh::parser;
use ghw::input::{self, Action, InputContext, OverlayMode};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

fn press(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

// ========== Data flow tests (always run) ==========

#[test]
fn full_flow_json_to_parse_to_state_to_tree_to_resolve() {
    // Step 1: JSON string (as gh CLI would return)
    let json = r#"[
        {
            "databaseId": 100,
            "displayTitle": "Integration Test Run",
            "name": "CI",
            "headBranch": "main",
            "status": "completed",
            "conclusion": "success",
            "createdAt": "2024-06-01T10:00:00Z",
            "updatedAt": "2024-06-01T10:05:00Z",
            "event": "push",
            "number": 50,
            "url": "https://github.com/test/repo/actions/runs/100"
        },
        {
            "databaseId": 101,
            "displayTitle": "Feature Branch Build",
            "name": "CI",
            "headBranch": "feature-x",
            "status": "in_progress",
            "conclusion": null,
            "createdAt": "2024-06-01T11:00:00Z",
            "updatedAt": "2024-06-01T11:02:00Z",
            "event": "push",
            "number": 51,
            "url": "https://github.com/test/repo/actions/runs/101"
        }
    ]"#;

    // Step 2: Parse
    let runs = parser::parse_runs(json).expect("parse should succeed");
    assert_eq!(runs.len(), 2);

    // Step 3: Build AppState
    let mut state = AppState::new("test/repo".to_string(), Some("main".to_string()), 20, None);
    state.runs = runs;
    state.rebuild_tree();
    assert_eq!(state.tree_items.len(), 2);

    // Step 4: Apply filter
    state.filter = FilterMode::CurrentBranch;
    state.rebuild_tree();
    assert_eq!(state.tree_items.len(), 1);

    // Step 5: Resolve
    let item = &state.tree_items[0];
    match state.resolve_item(item) {
        Some(ResolvedItem::Run(run)) => {
            assert_eq!(run.display_title, "Integration Test Run");
            assert_eq!(run.head_branch, "main");
        }
        _ => panic!("Expected Run"),
    }
}

#[test]
fn full_flow_with_jobs_expansion() {
    // Parse runs
    let runs_json = r#"[{
        "databaseId": 200,
        "displayTitle": "Full Pipeline",
        "name": "CI",
        "headBranch": "main",
        "status": "completed",
        "conclusion": "success",
        "createdAt": "2024-06-01T10:00:00Z",
        "updatedAt": "2024-06-01T10:10:00Z",
        "event": "push",
        "number": 60,
        "url": "https://github.com/test/repo/actions/runs/200"
    }]"#;
    let mut runs = parser::parse_runs(runs_json).unwrap();

    // Parse jobs (simulating a separate fetch)
    let jobs_json = r#"{"jobs":[{
        "databaseId": 301,
        "name": "build",
        "status": "completed",
        "conclusion": "success",
        "startedAt": "2024-06-01T10:00:00Z",
        "completedAt": "2024-06-01T10:05:00Z",
        "url": "https://github.com/test/repo/actions/runs/200/jobs/1",
        "steps": [
            {"name": "Checkout", "status": "completed", "conclusion": "success", "number": 1},
            {"name": "Build", "status": "completed", "conclusion": "success", "number": 2},
            {"name": "Test", "status": "completed", "conclusion": "failure", "number": 3}
        ]
    }]}"#;
    let jobs = parser::parse_jobs(jobs_json).unwrap();

    // Attach jobs to run
    runs[0].jobs = Some(jobs);

    // Build state and expand
    let mut state = make_state_with_runs(runs);
    state.expanded_runs.insert(200);
    state.expanded_jobs.insert((200, 301));
    state.rebuild_tree();

    // Verify tree depth: run + job + 3 steps = 5 items
    assert_eq!(state.tree_items.len(), 5);
    assert_eq!(state.tree_items[0].level, TreeLevel::Run);
    assert_eq!(state.tree_items[1].level, TreeLevel::Job);
    assert_eq!(state.tree_items[2].level, TreeLevel::Step);
    assert_eq!(state.tree_items[3].level, TreeLevel::Step);
    assert_eq!(state.tree_items[4].level, TreeLevel::Step);

    // Resolve step
    match state.resolve_item(&state.tree_items[4]) {
        Some(ResolvedItem::Step(step)) => {
            assert_eq!(step.name, "Test");
            assert_eq!(step.conclusion, Some(Conclusion::Failure));
        }
        _ => panic!("Expected Step"),
    }
}

#[test]
fn change_detection_across_poll_cycles() {
    let mut state = AppState::new("test/repo".to_string(), Some("main".to_string()), 20, None);

    // Cycle 1: First poll, no notifications
    let runs1 = vec![run_in_progress(1)];
    diff::detect_changes(&mut state, &runs1);
    assert!(
        state.notifications.is_empty(),
        "First poll should produce no notifications"
    );

    // Cycle 2: Status change -> notification
    let mut run_completed = run_with_id(1);
    run_completed.status = RunStatus::Completed;
    run_completed.conclusion = Some(Conclusion::Success);
    let runs2 = vec![run_completed];
    diff::detect_changes(&mut state, &runs2);
    assert_eq!(
        state.notifications.len(),
        1,
        "Status change should produce notification"
    );
    assert!(state.notifications[0]
        .message
        .contains("completed successfully"));

    // Cycle 3: Same status -> no new notification
    let mut run_still_completed = run_with_id(1);
    run_still_completed.status = RunStatus::Completed;
    run_still_completed.conclusion = Some(Conclusion::Success);
    let runs3 = vec![run_still_completed];
    diff::detect_changes(&mut state, &runs3);
    assert_eq!(
        state.notifications.len(),
        1,
        "Same status should not add notification"
    );
}

#[test]
fn input_to_state_action_flow() {
    let mut state = make_state_with_runs(vec![run_with_id(1), run_with_id(2), run_with_id(3)]);

    let ctx = InputContext::default();

    // Map key 'j' -> MoveDown
    let action = input::map_key(press(KeyCode::Char('j')), &ctx);
    assert_eq!(action, Action::MoveDown);
    state.move_cursor_down();
    assert_eq!(state.cursor, 1);

    // Map key 'k' -> MoveUp
    let action = input::map_key(press(KeyCode::Char('k')), &ctx);
    assert_eq!(action, Action::MoveUp);
    state.move_cursor_up();
    assert_eq!(state.cursor, 0);

    // Map key 'f' -> CycleFilter
    let action = input::map_key(press(KeyCode::Char('f')), &ctx);
    assert_eq!(action, Action::CycleFilter);
    state.cycle_filter();
    assert_eq!(state.filter, FilterMode::ActiveOnly);
    // All runs are completed, so tree should be empty
    assert!(state.tree_items.is_empty());

    // Cycle again to get back to All
    state.cycle_filter();
    state.cycle_filter();
    assert_eq!(state.filter, FilterMode::All);
    assert_eq!(state.tree_items.len(), 3);
}

#[test]
fn log_overlay_lifecycle() {
    use ghw::app::{Conclusion, Job, RunStatus, Step};

    // Create a failed run with a failed job
    let mut run = run_failed(1);
    let job = Job {
        database_id: Some(10),
        name: "build".to_string(),
        status: RunStatus::Completed,
        conclusion: Some(Conclusion::Failure),
        started_at: None,
        completed_at: None,
        url: "https://example.com".to_string(),
        steps: vec![Step {
            name: "Test".to_string(),
            status: RunStatus::Completed,
            conclusion: Some(Conclusion::Failure),
            number: 1,
            started_at: None,
            completed_at: None,
        }],
    };
    run.jobs = Some(vec![job]);

    let mut state = make_state_with_runs(vec![run]);

    // Cursor on the failed run
    assert!(state.current_item_is_failed());
    assert_eq!(state.current_item_ids(), Some((1, None)));

    // Open overlay
    let content = "error: test failed\nassert_eq failed at line 42".to_string();
    state.open_log_overlay("CI Build #1".to_string(), &content, 1, None);
    assert!(state.has_log_overlay());
    assert_eq!(state.log_overlay_text(), Some(content));

    // Scroll
    state.scroll_log_down(1, 10);
    assert_eq!(state.log_overlay_ref().unwrap().scroll, 0); // only 2 lines, can't scroll

    // Close
    state.close_log_overlay();
    assert!(!state.has_log_overlay());

    // Expand to job level
    state.expanded_runs.insert(1);
    state.rebuild_tree();
    state.cursor = 1; // on the job
    assert!(state.current_item_is_failed());
    assert_eq!(state.current_item_ids(), Some((1, Some(10))));

    // ViewLogs action maps from 'e' key
    let action = input::map_key(press(KeyCode::Char('e')), &InputContext::default());
    assert_eq!(action, Action::ViewLogs);

    // Overlay mode: 'e' closes
    state.open_log_overlay("test".to_string(), "log", 1, Some(10));
    let log_ctx = InputContext {
        overlay: OverlayMode::Log,
        ..Default::default()
    };
    let action = input::map_key(press(KeyCode::Char('e')), &log_ctx);
    assert_eq!(action, Action::CloseOverlay);
}

// ========== TUI snapshot tests ==========

#[test]
fn tui_header_contains_repo_name() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let state = make_state_with_runs(vec![run_with_id(1)]);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            ghw::tui::render::render(f, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let text: String = (0..buffer.area.width)
        .map(|x| buffer.cell((x, 0)).unwrap().symbol().to_string())
        .collect();
    assert!(
        text.contains("test/repo"),
        "Header should contain repo name, got: {text}"
    );
}

#[test]
fn tui_footer_contains_key_hints() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let state = make_state_with_runs(vec![run_with_id(1)]);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            ghw::tui::render::render(f, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    // Footer is at the last 2 rows; key hints are on the last content row (row 23)
    let footer_row = buffer.area.height - 1;
    let text: String = (0..buffer.area.width)
        .map(|x| buffer.cell((x, footer_row)).unwrap().symbol().to_string())
        .collect();
    assert!(
        text.contains("navigate"),
        "Footer should contain 'navigate' hint, got: {text}"
    );
}

#[test]
fn tui_tree_renders_run_titles() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let state = make_state_with_runs(vec![run_with_id(1), run_with_id(2)]);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            ghw::tui::render::render(f, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    // Tree area starts at row 2 (after 2-row header)
    let mut tree_text = String::new();
    for y in 2..buffer.area.height.saturating_sub(2) {
        for x in 0..buffer.area.width {
            tree_text.push_str(buffer.cell((x, y)).unwrap().symbol());
        }
        tree_text.push('\n');
    }
    assert!(
        tree_text.contains("CI Build #1"),
        "Tree should contain run title 'CI Build #1', got: {tree_text}"
    );
    assert!(
        tree_text.contains("CI Build #2"),
        "Tree should contain run title 'CI Build #2', got: {tree_text}"
    );
}

#[test]
fn tui_empty_state_shows_no_runs_message() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let state = make_state_with_runs(vec![]);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            ghw::tui::render::render(f, &state);
        })
        .unwrap();

    let buffer = terminal.backend().buffer().clone();
    let mut all_text = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            all_text.push_str(buffer.cell((x, y)).unwrap().symbol());
        }
    }
    assert!(
        all_text.contains("No workflow runs found"),
        "Empty state should show 'No workflow runs found', got: {all_text}"
    );
}

// ========== Live gh CLI tests (ignored by default) ==========

#[tokio::test]
#[ignore]
async fn gh_check_available() {
    ghw::gh::executor::check_gh_available()
        .await
        .expect("gh CLI should be authenticated");
}

#[tokio::test]
#[ignore]
async fn gh_fetch_runs_from_public_repo() {
    let json = ghw::gh::executor::fetch_runs("cli/cli", 5, None)
        .await
        .expect("should fetch runs from cli/cli");
    let runs = parser::parse_runs(&json).expect("should parse runs");
    assert!(!runs.is_empty(), "cli/cli should have runs");
    // Verify fields are populated
    let run = &runs[0];
    assert!(run.database_id > 0);
    assert!(!run.display_title.is_empty());
    assert!(!run.head_branch.is_empty());
    assert!(!run.url.is_empty());
}

#[tokio::test]
#[ignore]
async fn gh_fetch_jobs_from_public_repo() {
    let json = ghw::gh::executor::fetch_runs("cli/cli", 1, None)
        .await
        .expect("should fetch runs");
    let runs = parser::parse_runs(&json).expect("should parse");
    assert!(!runs.is_empty());

    let run_id = runs[0].database_id;
    let jobs_json = ghw::gh::executor::fetch_jobs("cli/cli", run_id)
        .await
        .expect("should fetch jobs");
    let jobs = parser::parse_jobs(&jobs_json).expect("should parse jobs");
    // Jobs may be empty for some run types, but parsing should succeed
    // Parsing succeeded - jobs may be empty for some run types
    let _ = jobs;
}

#[tokio::test]
#[ignore]
async fn gh_full_pipeline_fetch_parse_state() {
    let json = ghw::gh::executor::fetch_runs("cli/cli", 5, None)
        .await
        .expect("fetch runs");
    let runs = parser::parse_runs(&json).expect("parse runs");

    let mut state = AppState::new("cli/cli".to_string(), None, 5, None);
    state.runs = runs;
    state.rebuild_tree();

    assert!(!state.tree_items.is_empty());

    // Resolve all items
    for item in &state.tree_items {
        let resolved = state.resolve_item(item);
        assert!(resolved.is_some(), "Every tree item should resolve");
    }
}
