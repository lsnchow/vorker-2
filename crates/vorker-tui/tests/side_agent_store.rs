use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use vorker_tui::{SideAgentStatus, SideAgentStore, summarize_side_agent_events};

fn unique_temp_dir(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("vorker-side-agent-{name}-{suffix}"))
}

#[test]
fn side_agent_store_persists_jobs_and_sorts_recent_first() {
    let root = unique_temp_dir("store");
    fs::create_dir_all(&root).expect("create root");
    let path = root.join("agents.json");

    let mut store = SideAgentStore::open_at(path.clone()).expect("open store");
    let first = store
        .create_job(
            "/workspace/a",
            "inspect auth",
            "gpt-5.3-codex",
            "/tmp/agent-a.md",
            "/tmp/agent-a.stderr",
        )
        .expect("create first");
    let second = store
        .create_job(
            "/workspace/a",
            "review api",
            "gpt-5.4",
            "/tmp/agent-b.md",
            "/tmp/agent-b.stderr",
        )
        .expect("create second");

    store
        .mark_finished(&first.id, SideAgentStatus::Completed)
        .expect("mark first");

    let store = SideAgentStore::open_at(path).expect("reload store");
    let listed = store.list_jobs();

    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].id, second.id);
    assert_eq!(listed[0].status, SideAgentStatus::Running);
    assert_eq!(listed[1].id, first.id);
    assert_eq!(listed[1].status, SideAgentStatus::Completed);
    assert!(listed[1].finished_at_epoch_seconds.is_some());

    fs::remove_dir_all(root).ok();
}

#[test]
fn side_agent_store_can_reload_a_single_job() {
    let root = unique_temp_dir("single");
    fs::create_dir_all(&root).expect("create root");
    let path = root.join("agents.json");

    let mut store = SideAgentStore::open_at(path.clone()).expect("open store");
    let job = store
        .create_job(
            "/workspace/b",
            "summarize tests",
            "gpt-5.3-codex",
            "/tmp/agent-c.md",
            "/tmp/agent-c.stderr",
        )
        .expect("create job");

    let store = SideAgentStore::open_at(path).expect("reload store");
    let loaded = store.job(&job.id).expect("load job");

    assert_eq!(loaded.prompt, "summarize tests");
    assert_eq!(loaded.cwd, "/workspace/b");
    assert_eq!(loaded.output_path, "/tmp/agent-c.md");

    fs::remove_dir_all(root).ok();
}

#[test]
fn side_agent_store_rejects_corrupt_json_instead_of_erasing_it() {
    let root = unique_temp_dir("corrupt");
    fs::create_dir_all(&root).expect("create root");
    let path = root.join("agents.json");
    fs::write(&path, "{not-json").expect("write corrupt store");

    let error = match SideAgentStore::open_at(path) {
        Ok(_) => panic!("corrupt store should fail"),
        Err(error) => error,
    };

    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    fs::remove_dir_all(root).ok();
}

#[test]
fn side_agent_store_can_allocate_project_local_output_paths() {
    let root = unique_temp_dir("paths");
    fs::create_dir_all(&root).expect("create root");

    let mut store = SideAgentStore::open_at(root.join("agents.json")).expect("open store");
    let job = store
        .create_job_in_dir(
            "/workspace/c",
            "check routing",
            "gpt-5.3-codex",
            root.join("side-agents"),
        )
        .expect("create job");

    assert!(job.output_path.contains("/side-agents/"));
    assert!(job.output_path.ends_with("/last-message.md"));
    assert!(job.stderr_path.ends_with("/stderr.log"));
    assert!(job.events_path.ends_with("/events.jsonl"));
    assert!(std::path::Path::new(&job.output_path).exists());
    assert!(std::path::Path::new(&job.stderr_path).exists());
    assert!(std::path::Path::new(&job.events_path).exists());
    assert!(
        std::path::Path::new(&job.output_path)
            .parent()
            .unwrap()
            .exists()
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn side_agent_status_has_stable_display_labels() {
    assert_eq!(SideAgentStatus::Running.label(), "running");
    assert_eq!(SideAgentStatus::Completed.label(), "completed");
    assert_eq!(SideAgentStatus::Stopped.label(), "stopped");
    assert_eq!(SideAgentStatus::Failed.label(), "failed");
}

#[test]
fn side_agent_event_summary_extracts_codex_jsonl_events() {
    let root = unique_temp_dir("events");
    fs::create_dir_all(&root).expect("create root");
    let events_path = root.join("events.jsonl");
    fs::write(
        &events_path,
        [
            r#"{"type":"item.started","item":{"type":"command_execution","command":"cargo test"}}"#,
            r#"{"type":"item.completed","item":{"type":"agent_message","text":"Looks good."}}"#,
            r#"{"type":"turn.completed"}"#,
        ]
        .join("\n"),
    )
    .expect("write events");

    let summary = summarize_side_agent_events(&events_path, 10).expect("summary");

    assert_eq!(
        summary,
        vec![
            "command started: cargo test".to_string(),
            "assistant response captured".to_string(),
            "turn completed".to_string(),
        ]
    );

    fs::remove_dir_all(root).ok();
}
