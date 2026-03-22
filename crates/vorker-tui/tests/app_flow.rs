use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use vorker_tui::{ActionItem, App, InputMode, Pane};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn enter_on_new_agent_opens_overlay_then_creates_a_session_on_the_selected_model() {
    let mut app = App::new(vorker_core::Snapshot::default());

    assert!(app.handle_key(key(KeyCode::Enter)));
    assert_eq!(app.snapshot.sessions.len(), 0);
    assert!(app.status_line.contains("Create agent"));

    assert!(app.handle_key(key(KeyCode::Right)));
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(app.snapshot.sessions.len(), 1);
    assert_eq!(app.snapshot.sessions[0].model.as_deref(), Some("gpt-5.4"));
    assert_eq!(app.snapshot.sessions[0].role, "planner");
    assert!(app.status_line.contains("Created planner"));
}

#[test]
fn swarm_goal_flow_uses_an_overlay_then_creates_a_run_and_task_lanes() {
    let mut app = App::new(vorker_core::Snapshot::default());

    app.navigation.selected_action_id = ActionItem::Swarm;
    assert!(app.handle_key(key(KeyCode::Enter)));
    assert_eq!(app.snapshot.runs.len(), 0);
    assert_eq!(app.input_mode, InputMode::SwarmGoal);
    assert!(app.status_line.contains("Swarm launch"));

    for ch in "ship the runtime".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(app.snapshot.runs.len(), 1);
    assert_eq!(app.snapshot.runs[0].tasks.len(), 2);
    assert_eq!(app.snapshot.runs[0].goal, "ship the runtime");
    assert_eq!(app.input_mode, InputMode::Prompt);
    assert!(app.status_line.contains("Swarm launched"));
}

#[test]
fn prompt_flow_appends_transcript_events_for_the_active_agent() {
    let mut app = App::new(vorker_core::Snapshot::default());

    assert!(app.handle_key(key(KeyCode::Enter)));
    assert!(app.handle_key(key(KeyCode::Enter)));
    for ch in "plan the work".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert_eq!(app.navigation.focused_pane, Pane::Input);
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(app.snapshot.sessions.len(), 1);
    let transcript = &app.snapshot.sessions[0].transcript;
    assert_eq!(transcript.len(), 2);
    assert_eq!(transcript[0].role, "user");
    assert_eq!(transcript[0].text, "plan the work");
    assert_eq!(transcript[1].role, "assistant");
    assert!(transcript[1].text.contains("plan the work"));
}

#[test]
fn enter_outside_input_does_not_submit_the_composer_buffer() {
    let mut app = App::new(vorker_core::Snapshot::default());

    assert!(app.handle_key(key(KeyCode::Enter)));
    assert!(app.handle_key(key(KeyCode::Enter)));
    for ch in "hello".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    app.navigation.focused_pane = Pane::Runs;

    assert!(app.handle_key(key(KeyCode::Enter)));

    assert!(app.snapshot.sessions[0].transcript.is_empty());
    assert_eq!(app.navigation.command_buffer, "hello");
}
