use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::{RowKind, SessionEvent, SessionEventKind, StoredThread, TranscriptRow};

#[must_use]
pub fn render_transcript_markdown(thread: &StoredThread) -> String {
    let mut lines = vec![
        format!("# {}", thread.name),
        String::new(),
        format!("- thread: {}", thread.id),
        format!("- cwd: {}", thread.cwd),
        format!("- model: {}", thread.model.as_deref().unwrap_or("unknown")),
        format!("- duration: {}s", thread.total_active_seconds),
        String::new(),
    ];

    for row in &thread.rows {
        append_row(&mut lines, row);
    }

    lines.join("\n")
}

#[must_use]
pub fn render_transcript_markdown_from_events(
    thread: &StoredThread,
    events: &[SessionEvent],
) -> String {
    let mut lines = vec![
        format!("# {}", thread.name),
        String::new(),
        format!("- thread: {}", thread.id),
        format!("- cwd: {}", thread.cwd),
        format!("- model: {}", thread.model.as_deref().unwrap_or("unknown")),
        format!("- duration: {}s", thread.total_active_seconds),
        format!("- events: {}", events.len()),
        String::new(),
    ];

    for event in events {
        append_event(lines.as_mut(), event);
    }

    lines.join("\n")
}

pub fn write_transcript_export(
    root: &Path,
    thread: &StoredThread,
    events: Option<&[SessionEvent]>,
) -> io::Result<PathBuf> {
    fs::create_dir_all(root)?;
    let filename = format!("{}-{}.md", slugify(&thread.name), slugify(&thread.id));
    let path = root.join(filename);
    let markdown = match events.filter(|events| !events.is_empty()) {
        Some(events) => render_transcript_markdown_from_events(thread, events),
        None => render_transcript_markdown(thread),
    };
    fs::write(&path, markdown)?;
    Ok(path)
}

fn append_row(lines: &mut Vec<String>, row: &TranscriptRow) {
    lines.push(format!("## {}", row_heading(row.kind.clone())));
    lines.push(String::new());
    lines.push(row.text.clone());
    if let Some(detail) = &row.detail
        && !detail.trim().is_empty()
    {
        lines.push(String::new());
        lines.push("```text".to_string());
        lines.push(detail.clone());
        lines.push("```".to_string());
    }
    lines.push(String::new());
}

fn append_event(lines: &mut Vec<String>, event: &SessionEvent) {
    lines.push(format!("## {}", event_heading(&event.kind)));
    lines.push(String::new());
    match &event.kind {
        SessionEventKind::ThreadCreated { thread_name, cwd } => {
            lines.push(format!("Created thread `{thread_name}` in `{cwd}`."));
        }
        SessionEventKind::ThreadRenamed { from, to } => {
            lines.push(format!("Renamed thread from `{from}` to `{to}`."));
        }
        SessionEventKind::ModelChanged { from, to } => {
            lines.push(format!(
                "Model changed from `{}` to `{}`.",
                from.as_deref().unwrap_or("unset"),
                to.as_deref().unwrap_or("unset")
            ));
        }
        SessionEventKind::ApprovalModeChanged { from, to } => {
            lines.push(format!(
                "Approval mode changed from `{}` to `{}`.",
                from.label(),
                to.label()
            ));
        }
        SessionEventKind::CwdChanged { from, to } => {
            lines.push(format!(
                "Working directory changed from `{from}` to `{to}`."
            ));
        }
        SessionEventKind::RowAppended {
            row_kind,
            text,
            detail,
        } => {
            lines.push(format!("{}: {}", row_heading(row_kind.clone()), text));
            if let Some(detail) = detail
                && !detail.trim().is_empty()
            {
                lines.push(String::new());
                lines.push("```text".to_string());
                lines.push(detail.clone());
                lines.push("```".to_string());
            }
        }
        SessionEventKind::TranscriptReplaced { rows } => {
            lines.push(format!("Transcript replaced with {} row(s).", rows.len()));
        }
    }
    lines.push(String::new());
}

fn row_heading(kind: RowKind) -> &'static str {
    match kind {
        RowKind::System => "System",
        RowKind::User => "User",
        RowKind::Assistant => "Assistant",
        RowKind::Tool => "Tool",
    }
}

fn event_heading(kind: &SessionEventKind) -> &'static str {
    match kind {
        SessionEventKind::ThreadCreated { .. } => "Thread",
        SessionEventKind::ThreadRenamed { .. } => "Thread",
        SessionEventKind::ModelChanged { .. } => "Model",
        SessionEventKind::ApprovalModeChanged { .. } => "Approvals",
        SessionEventKind::CwdChanged { .. } => "Workspace",
        SessionEventKind::RowAppended { row_kind, .. } => row_heading(row_kind.clone()),
        SessionEventKind::TranscriptReplaced { .. } => "Transcript",
    }
}

fn slugify(input: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for ch in input.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            previous_dash = false;
            ch.to_ascii_lowercase()
        } else if !previous_dash {
            previous_dash = true;
            '-'
        } else {
            continue;
        };
        slug.push(mapped);
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "thread".to_string()
    } else {
        slug
    }
}
