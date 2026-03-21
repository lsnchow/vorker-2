use std::fs;

use serde_json::json;
use tempfile::tempdir;
use vorker_core::{EventLog, create_supervisor_event, restore_durable_supervisor_state};

#[test]
fn event_log_appends_ndjson_events_and_replays_them_in_order() {
    let tempdir = tempdir().expect("tempdir");
    let event_log = EventLog::new(tempdir.path(), None);

    let events = vec![
        create_supervisor_event(
            "run.created",
            json!({ "run": { "id": "run-1", "name": "Bootstrap" } }),
        ),
        create_supervisor_event(
            "task.created",
            json!({ "task": { "id": "task-1", "runId": "run-1", "title": "First task" } }),
        ),
    ];

    for event in &events {
        event_log.append(event).expect("append event");
    }

    let raw = fs::read_to_string(event_log.file_path()).expect("raw file");
    let replayed = event_log.read_all().expect("replayed events");

    assert_eq!(raw.trim().lines().count(), 2);
    assert_eq!(replayed.len(), 2);
    assert_eq!(replayed[0].kind, "run.created");
    assert_eq!(replayed[1].payload["task"]["id"], "task-1");
}

#[test]
fn restore_durable_supervisor_state_rebuilds_runs_and_tasks_without_reviving_sessions() {
    let tempdir = tempdir().expect("tempdir");
    let event_log = EventLog::new(
        tempdir.path(),
        Some(tempdir.path().join("supervisor.ndjson")),
    );

    event_log
        .append(&create_supervisor_event(
            "run.created",
            json!({
                "run": {
                    "id": "run-1",
                    "name": "Bootstrap",
                    "goal": "Persist runs",
                    "status": "running",
                    "workerAgentIds": [],
                    "createdAt": "2026-03-19T00:00:00.000Z",
                    "updatedAt": "2026-03-19T00:00:00.000Z"
                }
            }),
        ))
        .expect("append run");

    event_log
        .append(&create_supervisor_event(
            "task.created",
            json!({
                "task": {
                    "id": "task-1",
                    "runId": "run-1",
                    "title": "Persist task",
                    "description": "Keep it after restart",
                    "status": "completed",
                    "branchName": "vorker/task-task-1",
                    "workspacePath": "/repo/.vorker-2/worktrees/task-1",
                    "createdAt": "2026-03-19T00:01:00.000Z",
                    "updatedAt": "2026-03-19T00:01:00.000Z"
                }
            }),
        ))
        .expect("append task");

    event_log
        .append(&create_supervisor_event(
            "session.registered",
            json!({
                "session": {
                    "id": "agent-1",
                    "name": "Old agent"
                }
            }),
        ))
        .expect("append session");

    let snapshot = restore_durable_supervisor_state(&event_log).expect("snapshot restored");

    assert_eq!(snapshot.runs.len(), 1);
    assert_eq!(snapshot.runs[0].tasks.len(), 1);
    assert!(snapshot.sessions.is_empty());
    assert_eq!(
        snapshot.runs[0].tasks[0].branch_name.as_deref(),
        Some("vorker/task-task-1")
    );
}
