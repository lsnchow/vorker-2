use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::{RowKind, StoredThread, TranscriptRow};

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

pub fn write_transcript_export(root: &Path, thread: &StoredThread) -> io::Result<PathBuf> {
    fs::create_dir_all(root)?;
    let filename = format!("{}-{}.md", slugify(&thread.name), slugify(&thread.id));
    let path = root.join(filename);
    fs::write(&path, render_transcript_markdown(thread))?;
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

fn row_heading(kind: RowKind) -> &'static str {
    match kind {
        RowKind::System => "System",
        RowKind::User => "User",
        RowKind::Assistant => "Assistant",
        RowKind::Tool => "Tool",
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
