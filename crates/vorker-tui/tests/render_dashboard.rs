use vorker_core::{RunSnapshot, SessionRecord, Snapshot, TaskRecord, TranscriptEntry};
use vorker_tui::{DashboardOptions, Pane, render_dashboard};

fn sample_snapshot() -> Snapshot {
    Snapshot {
        sessions: vec![SessionRecord {
            id: "agent-1".to_string(),
            name: "Planner".to_string(),
            role: "worker".to_string(),
            status: "ready".to_string(),
            model: Some("gpt-5.4".to_string()),
            cwd: "/workspace".to_string(),
            transcript: vec![
                TranscriptEntry {
                    role: "user".to_string(),
                    text: "Plan the work".to_string(),
                },
                TranscriptEntry {
                    role: "assistant".to_string(),
                    text: "Plan ready".to_string(),
                },
            ],
            ..SessionRecord::default()
        }],
        runs: vec![RunSnapshot {
            id: "run-1".to_string(),
            name: "Bootstrap".to_string(),
            goal: "Build the supervisor core".to_string(),
            status: "running".to_string(),
            tasks: vec![TaskRecord {
                id: "task-1".to_string(),
                title: "Wire event bus".to_string(),
                run_id: "run-1".to_string(),
                status: "completed".to_string(),
                ..TaskRecord::default()
            }],
            ..RunSnapshot::default()
        }],
        events: vec![vorker_core::create_supervisor_event(
            "task.updated",
            serde_json::json!({ "task": { "title": "Wire event bus", "status": "completed" } }),
        )],
        ..Snapshot::default()
    }
}

#[test]
fn render_dashboard_uses_a_transcript_first_shell() {
    let output = render_dashboard(
        &sample_snapshot(),
        DashboardOptions {
            width: 120,
            provider_id: "copilot".to_string(),
            workspace_path: "/workspace".to_string(),
            active_session_id: Some("agent-1".to_string()),
            focused_pane: Pane::Input,
            ..DashboardOptions::default()
        },
    );

    for needle in [
        "[vorker]",
        "provider copilot",
        "model gpt-5.4",
        "cwd /workspace",
        "target agent Planner",
        "Chats",
        "Agents",
        "Runs",
        "Tasks",
        "Conversation",
        "user      Plan the work",
        "assistant Plan ready",
        "tool      task Wire event bus -> completed",
    ] {
        assert!(
            output.contains(needle),
            "missing {needle} in output:\n{output}"
        );
    }

    for removed in ["ACTIONS", "RUN OVERVIEW", "TASK INSPECTOR", "ACTIVITY"] {
        assert!(
            !output.contains(removed),
            "dashboard furniture should be gone ({removed}):\n{output}"
        );
    }
}

#[test]
fn render_dashboard_shows_slash_commands_below_the_composer() {
    let output = render_dashboard(
        &Snapshot::default(),
        DashboardOptions {
            width: 120,
            provider_id: "copilot".to_string(),
            workspace_path: "/workspace".to_string(),
            focused_pane: Pane::Input,
            command_buffer: "/m".to_string(),
            slash_menu_selected_index: 0,
            ..DashboardOptions::default()
        },
    );

    assert!(
        output.contains("Commands"),
        "missing slash command list:\n{output}"
    );
    assert!(
        output.contains("/model"),
        "missing filtered /model command:\n{output}"
    );
    assert!(
        !output.contains("/share"),
        "slash list should filter live by prefix:\n{output}"
    );
    assert!(
        output.contains("> /model"),
        "selected slash command should be highlighted inline:\n{output}"
    );
}

#[test]
fn render_dashboard_uses_a_prompt_first_empty_state() {
    let output = render_dashboard(
        &Snapshot::default(),
        DashboardOptions {
            width: 120,
            provider_id: "copilot".to_string(),
            workspace_path: "/workspace".to_string(),
            focused_pane: Pane::Input,
            ..DashboardOptions::default()
        },
    );

    for needle in [
        "Start typing to talk to an agent.",
        "Try /new to create an agent.",
        "Try /help to see available commands.",
        "> Type a prompt or /command",
    ] {
        assert!(
            output.contains(needle),
            "missing {needle} in output:\n{output}"
        );
    }
}

#[test]
fn render_dashboard_hides_the_sidebar_on_narrow_terminals() {
    let output = render_dashboard(
        &sample_snapshot(),
        DashboardOptions {
            width: 70,
            provider_id: "copilot".to_string(),
            workspace_path: "/workspace".to_string(),
            focused_pane: Pane::Input,
            ..DashboardOptions::default()
        },
    );

    assert!(
        !output.contains("Chats"),
        "narrow terminals should hide the sidebar by default:\n{output}"
    );
    assert!(
        output.contains("Conversation"),
        "main transcript should remain visible on narrow terminals:\n{output}"
    );
}

#[test]
fn render_dashboard_uses_focus_styling_for_the_composer_when_color_is_enabled() {
    let output = render_dashboard(
        &Snapshot::default(),
        DashboardOptions {
            width: 120,
            color: true,
            provider_id: "copilot".to_string(),
            workspace_path: "/workspace".to_string(),
            focused_pane: Pane::Input,
            ..DashboardOptions::default()
        },
    );

    assert!(
        output.contains("\u{1b}["),
        "color-enabled render should emit ansi styling:\n{output}"
    );
}

#[test]
fn render_dashboard_stays_ascii_safe_and_under_terminal_width() {
    let width = 120;
    let output = render_dashboard(
        &sample_snapshot(),
        DashboardOptions {
            width,
            provider_id: "copilot".to_string(),
            workspace_path: "/workspace".to_string(),
            focused_pane: Pane::Input,
            ..DashboardOptions::default()
        },
    );

    assert!(
        output.is_ascii(),
        "output should stay ASCII-only:\n{output}"
    );
    assert!(
        output.lines().all(|line| line.chars().count() <= width - 2),
        "renderer should stay comfortably within the terminal width:\n{output}"
    );
}
