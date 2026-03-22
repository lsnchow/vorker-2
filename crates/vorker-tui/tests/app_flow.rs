use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use vorker_tui::{App, Pane};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn app_starts_with_the_composer_focused() {
    let app = App::new(vorker_core::Snapshot::default());
    assert_eq!(app.navigation.focused_pane, Pane::Input);
}

#[test]
fn slash_new_opens_the_create_agent_overlay() {
    let mut app = App::new(vorker_core::Snapshot::default());

    for ch in "/new".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    let output = app.render(120, false);
    assert!(
        output.contains("CREATE AGENT"),
        "missing create-agent overlay:\n{output}"
    );
}

#[test]
fn slash_model_opens_the_model_picker() {
    let mut app = App::new(vorker_core::Snapshot::default());

    for ch in "/model".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    let output = app.render(120, false);
    assert!(
        output.contains("MODEL PICKER"),
        "missing model picker:\n{output}"
    );
}

#[test]
fn typing_a_prompt_and_pressing_enter_appends_transcript_turns() {
    let mut app = App::new(vorker_core::Snapshot::default());

    for ch in "/new".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));
    assert!(app.handle_key(key(KeyCode::Enter)));

    for ch in "plan the work".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert_eq!(app.navigation.focused_pane, Pane::Input);
    assert!(app.handle_key(key(KeyCode::Enter)));

    let transcript = &app.snapshot.sessions[0].transcript;
    assert_eq!(transcript.len(), 2);
    assert_eq!(transcript[0].role, "user");
    assert_eq!(transcript[0].text, "plan the work");
    assert_eq!(transcript[1].role, "assistant");
    assert!(transcript[1].text.contains("plan the work"));
}

#[test]
fn slash_runs_moves_focus_to_the_sidebar_runs_section() {
    let mut app = App::new(vorker_core::Snapshot {
        runs: vec![vorker_core::RunSnapshot {
            id: "run-1".to_string(),
            name: "Bootstrap".to_string(),
            ..vorker_core::RunSnapshot::default()
        }],
        ..vorker_core::Snapshot::default()
    });

    for ch in "/runs".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(app.navigation.focused_pane, Pane::Runs);
}
