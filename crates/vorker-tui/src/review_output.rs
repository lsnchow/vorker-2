use crate::render::{RowKind, TranscriptRow};

pub fn parse_review_markdown(markdown: &str) -> Vec<TranscriptRow> {
    let mut rows = Vec::new();
    let mut current: Option<TranscriptRow> = None;
    let mut in_code_block = false;
    let mut code_lines = Vec::new();

    let flush_current = |rows: &mut Vec<TranscriptRow>, current: &mut Option<TranscriptRow>| {
        if let Some(row) = current.take() {
            rows.push(row);
        }
    };

    for raw_line in markdown.lines() {
        let line = raw_line.trim_end();
        if line.starts_with("```") {
            if in_code_block {
                if let Some(row) = current.as_mut() {
                    let snippet = code_lines
                        .iter()
                        .map(|line| format!("    {line}"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    row.detail = Some(match row.detail.take() {
                        Some(existing) if !existing.is_empty() => {
                            format!("{existing}\n\n{snippet}")
                        }
                        _ => snippet,
                    });
                }
                code_lines.clear();
                in_code_block = false;
            } else {
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            code_lines.push(line.to_string());
            continue;
        }

        if let Some(text) = line.strip_prefix("# ") {
            flush_current(&mut rows, &mut current);
            current = Some(TranscriptRow {
                kind: RowKind::System,
                text: text.to_string(),
                detail: None,
            });
            continue;
        }

        if let Some(text) = line.strip_prefix("## ") {
            flush_current(&mut rows, &mut current);
            current = Some(TranscriptRow {
                kind: RowKind::System,
                text: text.to_string(),
                detail: None,
            });
            continue;
        }

        if let Some(text) = line.strip_prefix("### ") {
            flush_current(&mut rows, &mut current);
            current = Some(TranscriptRow {
                kind: RowKind::Tool,
                text: text.to_string(),
                detail: None,
            });
            continue;
        }

        if line.starts_with("- ") {
            if let Some(row) = current.as_mut() {
                let bullet = line.trim_start_matches("- ").to_string();
                if bullet.starts_with("Confidence:") {
                    continue;
                }
                row.detail = Some(match row.detail.take() {
                    Some(existing) if !existing.is_empty() => format!("{existing}\n{bullet}"),
                    _ => bullet,
                });
            } else {
                rows.push(TranscriptRow {
                    kind: RowKind::System,
                    text: line.trim_start_matches("- ").to_string(),
                    detail: None,
                });
            }
            continue;
        }

        if line.is_empty() {
            flush_current(&mut rows, &mut current);
            continue;
        }

        let text = line
            .trim_start_matches("**")
            .trim_end_matches("**")
            .to_string();

        if let Some(row) = current.as_mut() {
            row.detail = Some(match row.detail.take() {
                Some(existing) if !existing.is_empty() => format!("{existing}\n{text}"),
                _ => text,
            });
        } else {
            current = Some(TranscriptRow {
                kind: RowKind::Assistant,
                text,
                detail: None,
            });
        }
    }

    if in_code_block
        && !code_lines.is_empty()
        && let Some(row) = current.as_mut()
    {
        row.detail = Some(code_lines.join("\n"));
    }
    flush_current(&mut rows, &mut current);
    rows
}
