use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use vorker_tui::{
    ApprovalMode, RowKind, SessionEventKind, SessionEventStore, StoredThread, TranscriptRow,
    apply_events_to_thread, derive_thread_events, render_session_event_timeline,
    render_session_event_timeline_with_mode,
};

fn unique_temp_dir(name: &str) -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("vorker-events-{name}-{suffix}"))
}

#[test]
fn derive_thread_events_emits_creation_and_row_appends() {
    let mut thread = StoredThread::ephemeral("/workspace/pod");
    thread.name = "Hyperloop controls".to_string();
    thread.rows.push(TranscriptRow {
        kind: RowKind::User,
        text: "build controller".to_string(),
        detail: None,
    });

    let events = derive_thread_events(None, &thread);

    assert!(matches!(
        events.first().map(|event| &event.kind),
        Some(SessionEventKind::ThreadCreated { .. })
    ));
    assert!(events.iter().any(|event| matches!(
        event.kind,
        SessionEventKind::RowAppended {
            row_kind: RowKind::User,
            ..
        }
    )));
}

#[test]
fn derive_thread_events_emits_metadata_changes_and_transcript_replace() {
    let mut previous = StoredThread::ephemeral("/workspace/pod");
    previous.name = "Thread 1".to_string();
    previous.rows.push(TranscriptRow {
        kind: RowKind::User,
        text: "old".to_string(),
        detail: None,
    });

    let mut next = previous.clone();
    next.name = "Renamed thread".to_string();
    next.model = Some("gpt-5.4".to_string());
    next.approval_mode = ApprovalMode::Auto;
    next.rows = vec![TranscriptRow {
        kind: RowKind::System,
        text: "Conversation compacted.".to_string(),
        detail: Some("summary".to_string()),
    }];

    let events = derive_thread_events(Some(&previous), &next);

    assert!(
        events
            .iter()
            .any(|event| matches!(event.kind, SessionEventKind::ThreadRenamed { .. }))
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event.kind, SessionEventKind::ModelChanged { .. }))
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event.kind, SessionEventKind::ApprovalModeChanged { .. }))
    );
    assert!(events.iter().any(|event| matches!(
        event.kind,
        SessionEventKind::TranscriptReplaced { ref rows } if rows.len() == 1
    )));
}

#[test]
fn session_event_store_appends_and_reads_events() {
    let root = unique_temp_dir("store");
    fs::create_dir_all(&root).expect("root");
    let store = SessionEventStore::open_at(root.clone()).expect("store");
    let mut thread = StoredThread::ephemeral("/workspace/pod");
    thread.rows.push(TranscriptRow {
        kind: RowKind::User,
        text: "hello".to_string(),
        detail: None,
    });
    let events = derive_thread_events(None, &thread);

    store.append(&thread.id, &events).expect("append");
    let loaded = store.events(&thread.id).expect("load");

    assert_eq!(loaded.len(), events.len());
    assert!(store.path_for(&thread.id).starts_with(&root));

    fs::remove_dir_all(root).ok();
}

#[test]
fn render_session_event_timeline_summarizes_events() {
    let mut thread = StoredThread::ephemeral("/workspace/pod");
    thread.name = "Hyperloop controls".to_string();
    thread.rows.push(TranscriptRow {
        kind: RowKind::User,
        text: "build controller".to_string(),
        detail: None,
    });
    let events = derive_thread_events(None, &thread);

    let timeline = render_session_event_timeline(&thread.name, &events);

    assert!(timeline.contains("## Timeline"));
    assert!(timeline.contains("events:"));
    assert!(timeline.contains("[thread] created"));
    assert!(timeline.contains("[user] build controller"));
}

#[test]
fn render_session_event_timeline_recent_limits_output() {
    let mut thread = StoredThread::ephemeral("/workspace/pod");
    thread.name = "Hyperloop controls".to_string();
    for index in 0..12 {
        thread.rows.push(TranscriptRow {
            kind: RowKind::User,
            text: format!("row {index}"),
            detail: None,
        });
    }
    let events = derive_thread_events(None, &thread);

    let timeline = render_session_event_timeline_with_mode(&thread.name, &events, "recent", None);

    assert!(timeline.contains("- mode: recent"));
    assert!(!timeline.contains("1. [thread] created"));
    assert!(timeline.contains("10."));
}

#[test]
fn render_session_event_timeline_filter_limits_to_matching_kind() {
    let mut thread = StoredThread::ephemeral("/workspace/pod");
    thread.name = "Hyperloop controls".to_string();
    thread.model = Some("gpt-5.4".to_string());
    thread.rows.push(TranscriptRow {
        kind: RowKind::User,
        text: "build controller".to_string(),
        detail: None,
    });
    let events = derive_thread_events(None, &thread);

    let timeline =
        render_session_event_timeline_with_mode(&thread.name, &events, "filter", Some("model"));

    assert!(timeline.contains("- mode: filter"));
    assert!(timeline.contains("[model]"));
    assert!(!timeline.contains("[user]"));
}

#[test]
fn apply_events_to_thread_rebuilds_latest_thread_state() {
    let base = StoredThread::ephemeral("/workspace/pod");
    let mut next = base.clone();
    next.name = "Renamed thread".to_string();
    next.model = Some("gpt-5.4".to_string());
    next.approval_mode = ApprovalMode::Auto;
    next.rows.push(TranscriptRow {
        kind: RowKind::Assistant,
        text: "done".to_string(),
        detail: None,
    });
    let events = derive_thread_events(None, &next);

    let rebuilt = apply_events_to_thread(&base, &events);

    assert_eq!(rebuilt.name, "Renamed thread");
    assert_eq!(rebuilt.model.as_deref(), Some("gpt-5.4"));
    assert_eq!(rebuilt.approval_mode, ApprovalMode::Auto);
    assert_eq!(rebuilt.rows.len(), 1);
    assert_eq!(rebuilt.rows[0].text, "done");
}
