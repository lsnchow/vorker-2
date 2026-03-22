use vorker_core::{RunSnapshot, SessionRecord, Snapshot, TaskRecord};
use vorker_tui::{
    ACTION_ITEMS, NavKey, NavigationState, apply_navigation_key, reconcile_navigation_state,
};

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
                tasks: vec![
                    TaskRecord {
                        id: "task-1".to_string(),
                        run_id: "run-1".to_string(),
                        title: "Wire event bus".to_string(),
                        ..TaskRecord::default()
                    },
                    TaskRecord {
                        id: "task-2".to_string(),
                        run_id: "run-1".to_string(),
                        title: "Build renderer".to_string(),
                        ..TaskRecord::default()
                    },
                ],
                ..RunSnapshot::default()
            },
            RunSnapshot {
                id: "run-2".to_string(),
                name: "Polish".to_string(),
                tasks: vec![TaskRecord {
                    id: "task-3".to_string(),
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
fn reconcile_navigation_state_picks_defaults_for_empty_selection_state() {
    let next = reconcile_navigation_state(&snapshot(), NavigationState::default());

    assert_eq!(next.focused_pane.to_string(), "actions");
    assert_eq!(next.selected_action_id.to_string(), "new-agent");
    assert_eq!(next.active_session_id.as_deref(), Some("agent-1"));
    assert_eq!(next.active_run_id.as_deref(), Some("run-1"));
    assert_eq!(next.selected_task_id.as_deref(), Some("task-1"));
    assert_eq!(next.selected_model_id.as_deref(), Some("gpt-5.4"));
    assert_eq!(next.model_choices, vec!["gpt-5.4", "gpt-5", "gpt-4.1"]);
    assert!(!next.model_picker_open);
    assert!(next.command_buffer.is_empty());
}

#[test]
fn apply_navigation_key_moves_across_the_action_rail_and_pane_selections() {
    let snapshot = snapshot();
    let mut state = reconcile_navigation_state(&snapshot, NavigationState::default());

    state = apply_navigation_key(state, &snapshot, NavKey::Right);
    assert_eq!(state.selected_action_id.to_string(), "swarm");

    state = apply_navigation_key(state, &snapshot, NavKey::Left);
    assert_eq!(state.selected_action_id.to_string(), "new-agent");

    state = apply_navigation_key(state, &snapshot, NavKey::Down);
    assert_eq!(state.focused_pane.to_string(), "sessions");

    state = apply_navigation_key(state, &snapshot, NavKey::Up);
    assert_eq!(state.focused_pane.to_string(), "actions");

    state = apply_navigation_key(state, &snapshot, NavKey::Down);
    state = apply_navigation_key(state, &snapshot, NavKey::Right);
    assert_eq!(state.focused_pane.to_string(), "runs");

    state = apply_navigation_key(state, &snapshot, NavKey::Down);
    assert_eq!(state.active_run_id.as_deref(), Some("run-2"));
    assert_eq!(state.selected_task_id.as_deref(), Some("task-3"));

    state = apply_navigation_key(state, &snapshot, NavKey::Right);
    assert_eq!(state.focused_pane.to_string(), "tasks");

    state = apply_navigation_key(state, &snapshot, NavKey::Left);
    state = apply_navigation_key(state, &snapshot, NavKey::Up);
    assert_eq!(state.active_run_id.as_deref(), Some("run-1"));
    assert_eq!(state.selected_task_id.as_deref(), Some("task-1"));

    state = apply_navigation_key(state, &snapshot, NavKey::Left);
    assert_eq!(state.focused_pane.to_string(), "sessions");
}

#[test]
fn apply_navigation_key_cycles_models_while_the_model_picker_is_open() {
    let state = reconcile_navigation_state(
        &snapshot(),
        NavigationState {
            selected_action_id: ACTION_ITEMS[0],
            model_picker_open: true,
            ..NavigationState::default()
        },
    );

    let next = apply_navigation_key(state, &snapshot(), NavKey::Right);
    assert_eq!(next.selected_model_id.as_deref(), Some("gpt-5"));

    let last = apply_navigation_key(next, &snapshot(), NavKey::Left);
    assert_eq!(last.selected_model_id.as_deref(), Some("gpt-5.4"));
}

#[test]
fn apply_navigation_key_can_move_down_into_the_input_pane() {
    let snapshot = snapshot();
    let mut state = reconcile_navigation_state(&snapshot, NavigationState::default());

    state = apply_navigation_key(state, &snapshot, NavKey::Down);
    state = apply_navigation_key(state, &snapshot, NavKey::Right);
    state = apply_navigation_key(state, &snapshot, NavKey::Right);
    state = apply_navigation_key(state, &snapshot, NavKey::Right);

    assert_eq!(state.focused_pane.to_string(), "events");

    state = apply_navigation_key(state, &snapshot, NavKey::Down);
    assert_eq!(state.focused_pane.to_string(), "input");

    state = apply_navigation_key(state, &snapshot, NavKey::Up);
    assert_eq!(state.focused_pane.to_string(), "events");
}
