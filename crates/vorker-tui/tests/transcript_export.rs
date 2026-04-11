use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use vorker_tui::{
    ApprovalMode, RowKind, SessionEvent, SessionEventKind, StoredThread, TranscriptRow,
    render_transcript_markdown, render_transcript_markdown_from_events,
    render_transcript_markdown_from_events_with_options, render_transcript_markdown_with_options,
    write_transcript_export,
};

fn unique_temp_dir(name: &str) -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("vorker-export-{name}-{suffix}"))
}

#[test]
fn transcript_export_renders_thread_as_markdown() {
    let thread = StoredThread {
        id: "thread-1".to_string(),
        name: "Hyperloop controls".to_string(),
        cwd: "/workspace/pod".to_string(),
        rows: vec![
            TranscriptRow {
                kind: RowKind::User,
                text: "build the controller".to_string(),
                detail: None,
            },
            TranscriptRow {
                kind: RowKind::Tool,
                text: "Explored".to_string(),
                detail: Some("Read src/controller.rs".to_string()),
            },
            TranscriptRow {
                kind: RowKind::Assistant,
                text: "Implemented PID loop.".to_string(),
                detail: None,
            },
        ],
        model: Some("gpt-5.4".to_string()),
        approval_mode: ApprovalMode::Manual,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 2,
        total_active_seconds: 42,
    };

    let markdown = render_transcript_markdown(&thread);

    assert!(markdown.contains("# Hyperloop controls"));
    assert!(markdown.contains("- model: gpt-5.4"));
    assert!(markdown.contains("## User"));
    assert!(markdown.contains("build the controller"));
    assert!(markdown.contains("## Tool"));
    assert!(markdown.contains("Read src/controller.rs"));
    assert!(markdown.contains("## Assistant"));
}

#[test]
fn transcript_export_writes_a_safe_filename() {
    let root = unique_temp_dir("write");
    fs::create_dir_all(&root).expect("create root");
    let mut thread = StoredThread::ephemeral("/workspace/pod");
    thread.id = "thread/unsafe".to_string();
    thread.name = "Hyperloop: Controls?".to_string();

    let path = write_transcript_export(&root, &thread, None, "auto").expect("write export");

    assert!(path.starts_with(&root));
    assert!(path.ends_with("hyperloop-controls-thread-unsafe.md"));
    assert!(
        fs::read_to_string(&path)
            .expect("read export")
            .contains("# Hyperloop: Controls?")
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn transcript_export_can_render_from_events() {
    let mut thread = StoredThread::ephemeral("/workspace/pod");
    thread.name = "Hyperloop controls".to_string();
    let events = vec![
        SessionEvent {
            timestamp_epoch_seconds: 1,
            thread_id: thread.id.clone(),
            kind: SessionEventKind::ThreadCreated {
                thread_name: thread.name.clone(),
                cwd: thread.cwd.clone(),
            },
        },
        SessionEvent {
            timestamp_epoch_seconds: 2,
            thread_id: thread.id.clone(),
            kind: SessionEventKind::RowAppended {
                row_kind: RowKind::User,
                text: "build the controller".to_string(),
                detail: None,
            },
        },
    ];

    let markdown = render_transcript_markdown_from_events(&thread, &events);

    assert!(markdown.contains("- events: 2"));
    assert!(markdown.contains("## Thread"));
    assert!(markdown.contains("Created thread"));
    assert!(markdown.contains("## User"));
    assert!(markdown.contains("build the controller"));
}

#[test]
fn transcript_export_brief_mode_omits_metadata_and_details() {
    let thread = StoredThread {
        id: "thread-1".to_string(),
        name: "Hyperloop controls".to_string(),
        cwd: "/workspace/pod".to_string(),
        rows: vec![TranscriptRow {
            kind: RowKind::Tool,
            text: "Explored".to_string(),
            detail: Some("Read src/controller.rs".to_string()),
        }],
        model: Some("gpt-5.4".to_string()),
        approval_mode: ApprovalMode::Manual,
        created_at_epoch_seconds: 1,
        updated_at_epoch_seconds: 2,
        total_active_seconds: 42,
    };

    let rows_markdown = render_transcript_markdown_with_options(&thread, false, false);
    assert!(!rows_markdown.contains("- model:"));
    assert!(!rows_markdown.contains("```text"));

    let events = vec![SessionEvent {
        timestamp_epoch_seconds: 2,
        thread_id: thread.id.clone(),
        kind: SessionEventKind::RowAppended {
            row_kind: RowKind::Tool,
            text: "Explored".to_string(),
            detail: Some("Read src/controller.rs".to_string()),
        },
    }];
    let events_markdown =
        render_transcript_markdown_from_events_with_options(&thread, &events, false, false);
    assert!(!events_markdown.contains("- events:"));
    assert!(!events_markdown.contains("```text"));
}
