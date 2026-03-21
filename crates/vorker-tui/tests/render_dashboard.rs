use vorker_core::{RunSnapshot, SessionRecord, Snapshot, TaskRecord, TranscriptEntry};
use vorker_tui::{DashboardOptions, render_dashboard};

#[test]
fn render_dashboard_prints_sessions_runs_tasks_and_tunnel_status() {
    let output = render_dashboard(
        &Snapshot {
            sessions: vec![SessionRecord {
                id: "agent-1".to_string(),
                name: "Planner".to_string(),
                role: "arbitrator".to_string(),
                status: "ready".to_string(),
                model: Some("gpt-5.4".to_string()),
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
                    assigned_agent_id: Some("agent-1".to_string()),
                    execution_agent_id: Some("exec-1".to_string()),
                    branch_name: Some("vorker/task-task-1-wire-event-bus".to_string()),
                    workspace_path: Some("/repo/.vorker-2/worktrees/task-1".to_string()),
                    commit_sha: Some("abc123def456".to_string()),
                    change_count: 1,
                    ..TaskRecord::default()
                }],
                ..RunSnapshot::default()
            }],
            share: Some(serde_json::json!({
                "state": "ready",
                "publicUrl": "https://example.trycloudflare.com?transport=poll"
            })),
            events: vec![
                vorker_core::create_supervisor_event(
                    "task.updated",
                    serde_json::json!({ "task": { "title": "Wire event bus", "status": "completed" } }),
                ),
                vorker_core::create_supervisor_event(
                    "run.updated",
                    serde_json::json!({ "run": { "name": "Bootstrap", "status": "running" } }),
                ),
            ],
            ..Snapshot::default()
        },
        DashboardOptions {
            selected_action_id: "swarm".parse().expect("action"),
            selected_model_id: Some("gpt-5.4".to_string()),
            model_choices: vec!["gpt-5.4".to_string(), "gpt-5".to_string()],
            active_session_id: Some("agent-1".to_string()),
            active_run_id: Some("run-1".to_string()),
            width: 100,
            status_line: "Ready for commands.".to_string(),
            ..DashboardOptions::default()
        },
    );

    for needle in [
        "VORKER-2",
        "LAUNCH RAIL",
        "NEW AGENT",
        "SWARM",
        "gpt-5.4",
        "ACTIVE AGENTS",
        "AGENT DETAIL",
        "RUN BOARD",
        "EVENT FEED",
        "Planner",
        "Bootstrap",
        "Wire event bus",
        "vorker/task-task-1-wire-event-bus",
        "exec-1",
        "abc123def456",
        "ready",
        "Plan ready",
        "Ready for commands",
    ] {
        assert!(
            output.contains(needle),
            "missing {needle} in output:\n{output}"
        );
    }
}
