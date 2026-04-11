use vorker_core::{RunSnapshot, SessionRecord, Snapshot, TaskRecord};
use vorker_tui::{NavKey, NavigationState, Pane, apply_navigation_key, reconcile_navigation_state};

fn snapshot() -> Snapshot {
    Snapshot {
        sessions: vec![
            SessionRecord {
                id: "agent-1".to_string(),
                name: "Planner".to_string(),
                model: Some("gpt-5.4".to_string()),
                ..SessionRecord::default()
            },
            SessionRecord {
                id: "agent-2".to_string(),
                name: "Reviewer".to_string(),
                ..SessionRecord::default()
            },
        ],
        runs: vec![
            RunSnapshot {
                id: "run-1".to_string(),
                name: "Bootstrap".to_string(),
                tasks: vec![TaskRecord {
                    id: "task-1".to_string(),
                    run_id: "run-1".to_string(),
                    title: "Wire event bus".to_string(),
                    ..TaskRecord::default()
                }],
                ..RunSnapshot::default()
            },
            RunSnapshot {
                id: "run-2".to_string(),
                name: "Polish".to_string(),
                tasks: vec![TaskRecord {
                    id: "task-2".to_string(),
                    run_id: "run-2".to_string(),
                    title: "Tune theme".to_string(),
                    ..TaskRecord::default()
                }],
                ..RunSnapshot::default()
            },
        ],
        ..Snapshot::default()
    }
}

#[test]
fn reconcile_navigation_state_defaults_to_input_and_active_records() {
    let next = reconcile_navigation_state(&snapshot(), NavigationState::default());

    assert_eq!(next.focused_pane, Pane::Input);
    assert_eq!(next.active_session_id.as_deref(), Some("agent-1"));
    assert_eq!(next.active_run_id.as_deref(), Some("run-1"));
    assert_eq!(next.selected_task_id.as_deref(), Some("task-1"));
}

#[test]
fn tab_cycles_between_input_and_sidebar_sections() {
    let snapshot = snapshot();
    let mut state = reconcile_navigation_state(&snapshot, NavigationState::default());

    state = apply_navigation_key(state, &snapshot, NavKey::Tab);
    assert_eq!(state.focused_pane, Pane::Sessions);

    state = apply_navigation_key(state, &snapshot, NavKey::Tab);
    assert_eq!(state.focused_pane, Pane::Runs);

    state = apply_navigation_key(state, &snapshot, NavKey::Tab);
    assert_eq!(state.focused_pane, Pane::Tasks);

    state = apply_navigation_key(state, &snapshot, NavKey::Tab);
    assert_eq!(state.focused_pane, Pane::Input);
}

#[test]
fn arrow_navigation_moves_inside_sidebar_sections_only() {
    let snapshot = snapshot();
    let mut state = reconcile_navigation_state(
        &snapshot,
        NavigationState {
            focused_pane: Pane::Runs,
            ..NavigationState::default()
        },
    );

    state = apply_navigation_key(state, &snapshot, NavKey::Down);
    assert_eq!(state.active_run_id.as_deref(), Some("run-2"));

    state = apply_navigation_key(state, &snapshot, NavKey::Right);
    assert_eq!(state.focused_pane, Pane::Tasks);

    state = apply_navigation_key(state, &snapshot, NavKey::Left);
    assert_eq!(state.focused_pane, Pane::Runs);
}
