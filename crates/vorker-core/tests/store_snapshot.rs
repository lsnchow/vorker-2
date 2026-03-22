use std::collections::HashSet;

use serde_json::json;
use vorker_core::{SupervisorEvent, SupervisorStore, create_supervisor_event};

#[test]
fn supervisor_store_rebuilds_run_task_and_session_state_from_events() {
    let mut store = SupervisorStore::new();

    store.append(create_supervisor_event(
        "run.created",
        json!({
            "run": {
                "id": "run-1",
                "name": "Bootstrap",
                "goal": "Clone Claude Code ergonomics on top of Copilot",
                "status": "draft",
                "workerAgentIds": [],
                "createdAt": "2026-03-19T00:00:00.000Z",
                "updatedAt": "2026-03-19T00:00:00.000Z"
            }
        }),
    ));

    store.append(create_supervisor_event(
        "session.registered",
        json!({
            "session": {
                "id": "agent-1",
                "name": "Arbitrator",
                "role": "arbitrator",
                "status": "ready",
                "mode": "plan",
                "model": "gpt-5.4",
                "cwd": "/workspace"
            }
        }),
    ));

    store.append(create_supervisor_event(
        "task.created",
        json!({
            "task": {
                "id": "task-1",
                "runId": "run-1",
                "parentTaskId": null,
                "title": "Design the supervisor core",
                "description": "Establish the canonical event model and state reducer.",
                "status": "ready",
                "assignedAgentId": "agent-1",
                "createdAt": "2026-03-19T00:01:00.000Z",
                "updatedAt": "2026-03-19T00:01:00.000Z"
            }
        }),
    ));

    store.append(create_supervisor_event(
        "session.prompt.finished",
        json!({
            "sessionId": "agent-1",
            "message": {
                "role": "assistant",
                "text": "Supervisor core planned."
            }
        }),
    ));

    let snapshot = store.snapshot();
    let run = &snapshot.runs[0];
    let task = &snapshot.tasks[0];
    let session = &snapshot.sessions[0];

    assert_eq!(run.id, "run-1");
    assert_eq!(run.tasks.len(), 1);
    assert_eq!(run.tasks[0].id, "task-1");
    assert_eq!(task.assigned_agent_id.as_deref(), Some("agent-1"));
    assert_eq!(session.transcript.len(), 1);
    assert_eq!(session.transcript[0].role, "assistant");
    assert_eq!(session.transcript[0].text, "Supervisor core planned.");
}

#[test]
fn supervisor_store_applies_task_updates_and_preserves_parent_child_relationships() {
    let mut store = SupervisorStore::new();

    store.append(create_supervisor_event(
        "run.created",
        json!({
            "run": {
                "id": "run-2",
                "name": "Parallel work",
                "goal": "Split orchestration and UI",
                "status": "running",
                "workerAgentIds": ["agent-2"],
                "createdAt": "2026-03-19T00:00:00.000Z",
                "updatedAt": "2026-03-19T00:00:00.000Z"
            }
        }),
    ));

    store.append(create_supervisor_event(
        "task.created",
        json!({
            "task": {
                "id": "task-parent",
                "runId": "run-2",
                "parentTaskId": null,
                "title": "Parent task",
                "description": "Root scope",
                "status": "running",
                "createdAt": "2026-03-19T00:00:00.000Z",
                "updatedAt": "2026-03-19T00:00:00.000Z"
            }
        }),
    ));

    store.append(create_supervisor_event(
        "task.created",
        json!({
            "task": {
                "id": "task-child",
                "runId": "run-2",
                "parentTaskId": "task-parent",
                "title": "Child task",
                "description": "Leaf scope",
                "status": "ready",
                "createdAt": "2026-03-19T00:02:00.000Z",
                "updatedAt": "2026-03-19T00:02:00.000Z"
            }
        }),
    ));

    store.append(create_supervisor_event(
        "task.updated",
        json!({
            "task": {
                "id": "task-child",
                "runId": "run-2",
                "parentTaskId": "task-parent",
                "title": "Child task",
                "description": "Leaf scope",
                "status": "completed",
                "templateAgentId": "template-1",
                "executionAgentId": "exec-1",
                "workspacePath": "/repo/.vorker-2/worktrees/task-child",
                "branchName": "vorker/task-task-child",
                "baseBranch": "main",
                "commitSha": "abc123",
                "changeCount": 1,
                "changedFiles": ["src/index.js"],
                "outputText": "done",
                "createdAt": "2026-03-19T00:02:00.000Z",
                "updatedAt": "2026-03-19T00:03:00.000Z"
            }
        }),
    ));

    let snapshot = store.snapshot();
    let run = &snapshot.runs[0];
    let task_ids: HashSet<_> = run.tasks.iter().map(|task| task.id.as_str()).collect();
    let parent = run
        .tasks
        .iter()
        .find(|task| task.id == "task-parent")
        .expect("parent task");
    let child = run
        .tasks
        .iter()
        .find(|task| task.id == "task-child")
        .expect("child task");

    assert_eq!(task_ids.len(), 2);
    assert!(task_ids.contains("task-parent"));
    assert!(task_ids.contains("task-child"));
    assert!(parent.parent_task_id.is_none());
    assert_eq!(child.parent_task_id.as_deref(), Some("task-parent"));
    assert_eq!(child.status, "completed");
    assert_eq!(child.output_text, "done");
    assert_eq!(child.template_agent_id.as_deref(), Some("template-1"));
    assert_eq!(child.execution_agent_id.as_deref(), Some("exec-1"));
    assert_eq!(
        child.workspace_path.as_deref(),
        Some("/repo/.vorker-2/worktrees/task-child")
    );
    assert_eq!(child.branch_name.as_deref(), Some("vorker/task-task-child"));
    assert_eq!(child.base_branch.as_deref(), Some("main"));
    assert_eq!(child.commit_sha.as_deref(), Some("abc123"));
    assert_eq!(child.change_count, 1);
    assert_eq!(child.changed_files, vec!["src/index.js"]);
}

#[test]
fn supervisor_event_round_trips_with_json_shape_used_by_js_runtime() {
    let event = create_supervisor_event("share.updated", json!({ "share": { "status": "ready" } }));

    let encoded = serde_json::to_string(&event).expect("event serializes");
    let decoded: SupervisorEvent = serde_json::from_str(&encoded).expect("event deserializes");

    assert_eq!(decoded.kind, "share.updated");
    assert_eq!(decoded.payload["share"]["status"], "ready");
}

#[test]
fn supervisor_store_tracks_preflight_state_from_explicit_preflight_events() {
    let mut store = SupervisorStore::new();

    store.append(create_supervisor_event(
        "run.created",
        json!({
            "run": {
                "id": "preflight-1",
                "name": "Preflight octocat/hello-world",
                "goal": "Vet https://github.com/octocat/Hello-World",
                "status": "running",
                "type": "preflight",
                "createdAt": "2026-03-21T00:00:00.000Z",
                "updatedAt": "2026-03-21T00:00:00.000Z"
            },
            "preflight": {
                "runId": "preflight-1",
                "repoInput": "https://github.com/octocat/Hello-World",
                "repoSourceType": "github",
                "stage": "intake",
                "sandboxState": "idle",
                "artifactsDir": "/tmp/preflight-1"
            }
        }),
    ));

    store.append(create_supervisor_event(
        "preflight.classified",
        json!({
            "run": {
                "id": "preflight-1",
                "status": "running",
                "updatedAt": "2026-03-21T00:01:00.000Z"
            },
            "preflight": {
                "runId": "preflight-1",
                "stage": "risk",
                "classification": "web app",
                "classificationConfidence": "0.86",
                "strategy": "node-web",
                "runtimeFamily": "node",
                "packageManager": "pnpm"
            }
        }),
    ));

    store.append(create_supervisor_event(
        "preflight.verified",
        json!({
            "run": {
                "id": "preflight-1",
                "status": "completed",
                "updatedAt": "2026-03-21T00:02:00.000Z"
            },
            "preflight": {
                "runId": "preflight-1",
                "stage": "report",
                "riskLevel": "low",
                "sandboxBackend": "docker",
                "sandboxState": "completed",
                "outcome": "Verified",
                "previewUrl": "http://127.0.0.1:4173",
                "summaryPath": "/tmp/preflight-1/summary.md",
                "reportPath": "/tmp/preflight-1/report.json"
            }
        }),
    ));

    let snapshot = store.snapshot();
    let run = &snapshot.runs[0];
    let preflight = run.preflight.as_ref().expect("preflight metadata");

    assert_eq!(run.run_type.as_deref(), Some("preflight"));
    assert_eq!(
        preflight.repo_input,
        "https://github.com/octocat/Hello-World"
    );
    assert_eq!(preflight.classification.as_deref(), Some("web app"));
    assert_eq!(preflight.classification_confidence.as_deref(), Some("0.86"));
    assert_eq!(preflight.strategy.as_deref(), Some("node-web"));
    assert_eq!(preflight.package_manager.as_deref(), Some("pnpm"));
    assert_eq!(preflight.stage, "report");
    assert_eq!(preflight.risk_level.as_deref(), Some("low"));
    assert_eq!(preflight.outcome.as_deref(), Some("Verified"));
    assert_eq!(
        preflight.preview_url.as_deref(),
        Some("http://127.0.0.1:4173")
    );
}
