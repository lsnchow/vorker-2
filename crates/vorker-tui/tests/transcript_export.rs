use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use vorker_tui::{
    ApprovalMode, RowKind, StoredThread, TranscriptRow, render_transcript_markdown,
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

    let path = write_transcript_export(&root, &thread).expect("write export");

    assert!(path.starts_with(&root));
    assert!(path.ends_with("hyperloop-controls-thread-unsafe.md"));
    assert!(
        fs::read_to_string(&path)
            .expect("read export")
            .contains("# Hyperloop: Controls?")
    );

    fs::remove_dir_all(root).ok();
}
