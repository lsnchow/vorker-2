use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{ApprovalMode, RowKind, StoredThread, TranscriptRow};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEventKind {
    ThreadCreated {
        thread_name: String,
        cwd: String,
    },
    ThreadRenamed {
        from: String,
        to: String,
    },
    ModelChanged {
        from: Option<String>,
        to: Option<String>,
    },
    ApprovalModeChanged {
        from: ApprovalMode,
        to: ApprovalMode,
    },
    CwdChanged {
        from: String,
        to: String,
    },
    RowAppended {
        row_kind: RowKind,
        text: String,
        detail: Option<String>,
    },
    TranscriptReplaced {
        rows: Vec<TranscriptRow>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionEvent {
    pub timestamp_epoch_seconds: u64,
    pub thread_id: String,
    #[serde(flatten)]
    pub kind: SessionEventKind,
}

pub struct SessionEventStore {
    root: PathBuf,
}

impl SessionEventStore {
    pub fn open_at(root: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    #[must_use]
    pub fn path_for(&self, thread_id: &str) -> PathBuf {
        self.root.join(format!("{thread_id}.jsonl"))
    }

    pub fn append(&self, thread_id: &str, events: &[SessionEvent]) -> io::Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let path = self.path_for(thread_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        for event in events {
            let line = serde_json::to_string(event).map_err(io::Error::other)?;
            file.write_all(line.as_bytes())?;
            file.write_all(b"\n")?;
        }
        Ok(())
    }

    pub fn events(&self, thread_id: &str) -> io::Result<Vec<SessionEvent>> {
        let path = self.path_for(thread_id);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let mut events = Vec::new();
        for (index, line) in fs::read_to_string(&path)?.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let event = serde_json::from_str::<SessionEvent>(line).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "failed to parse {} line {}: {error}",
                        path.display(),
                        index + 1
                    ),
                )
            })?;
            events.push(event);
        }
        Ok(events)
    }
}

#[must_use]
pub fn derive_thread_events(
    previous: Option<&StoredThread>,
    next: &StoredThread,
) -> Vec<SessionEvent> {
    let timestamp = now_epoch_seconds();
    let mut events = Vec::new();

    match previous {
        None => {
            events.push(SessionEvent {
                timestamp_epoch_seconds: timestamp,
                thread_id: next.id.clone(),
                kind: SessionEventKind::ThreadCreated {
                    thread_name: next.name.clone(),
                    cwd: next.cwd.clone(),
                },
            });
            if next.model.is_some() {
                events.push(SessionEvent {
                    timestamp_epoch_seconds: timestamp,
                    thread_id: next.id.clone(),
                    kind: SessionEventKind::ModelChanged {
                        from: None,
                        to: next.model.clone(),
                    },
                });
            }
            if next.approval_mode != ApprovalMode::Manual {
                events.push(SessionEvent {
                    timestamp_epoch_seconds: timestamp,
                    thread_id: next.id.clone(),
                    kind: SessionEventKind::ApprovalModeChanged {
                        from: ApprovalMode::Manual,
                        to: next.approval_mode,
                    },
                });
            }
            events.extend(next.rows.iter().cloned().map(|row| SessionEvent {
                timestamp_epoch_seconds: timestamp,
                thread_id: next.id.clone(),
                kind: row_event(row),
            }));
        }
        Some(previous) => {
            if previous.name != next.name {
                events.push(SessionEvent {
                    timestamp_epoch_seconds: timestamp,
                    thread_id: next.id.clone(),
                    kind: SessionEventKind::ThreadRenamed {
                        from: previous.name.clone(),
                        to: next.name.clone(),
                    },
                });
            }

            if previous.cwd != next.cwd {
                events.push(SessionEvent {
                    timestamp_epoch_seconds: timestamp,
                    thread_id: next.id.clone(),
                    kind: SessionEventKind::CwdChanged {
                        from: previous.cwd.clone(),
                        to: next.cwd.clone(),
                    },
                });
            }

            if previous.model != next.model {
                events.push(SessionEvent {
                    timestamp_epoch_seconds: timestamp,
                    thread_id: next.id.clone(),
                    kind: SessionEventKind::ModelChanged {
                        from: previous.model.clone(),
                        to: next.model.clone(),
                    },
                });
            }

            if previous.approval_mode != next.approval_mode {
                events.push(SessionEvent {
                    timestamp_epoch_seconds: timestamp,
                    thread_id: next.id.clone(),
                    kind: SessionEventKind::ApprovalModeChanged {
                        from: previous.approval_mode,
                        to: next.approval_mode,
                    },
                });
            }

            if next.rows.starts_with(&previous.rows) {
                events.extend(next.rows[previous.rows.len()..].iter().cloned().map(|row| {
                    SessionEvent {
                        timestamp_epoch_seconds: timestamp,
                        thread_id: next.id.clone(),
                        kind: row_event(row),
                    }
                }));
            } else if previous.rows != next.rows {
                events.push(SessionEvent {
                    timestamp_epoch_seconds: timestamp,
                    thread_id: next.id.clone(),
                    kind: SessionEventKind::TranscriptReplaced {
                        rows: next.rows.clone(),
                    },
                });
            }
        }
    }

    events
}

#[must_use]
pub fn apply_events_to_thread(base: &StoredThread, events: &[SessionEvent]) -> StoredThread {
    let mut thread = base.clone();
    thread.rows.clear();
    for event in events {
        match &event.kind {
            SessionEventKind::ThreadCreated { thread_name, cwd } => {
                thread.name = thread_name.clone();
                thread.cwd = cwd.clone();
            }
            SessionEventKind::ThreadRenamed { to, .. } => {
                thread.name = to.clone();
            }
            SessionEventKind::ModelChanged { to, .. } => {
                thread.model = to.clone();
            }
            SessionEventKind::ApprovalModeChanged { to, .. } => {
                thread.approval_mode = *to;
            }
            SessionEventKind::CwdChanged { to, .. } => {
                thread.cwd = to.clone();
            }
            SessionEventKind::RowAppended {
                row_kind,
                text,
                detail,
            } => thread.rows.push(TranscriptRow {
                kind: row_kind.clone(),
                text: text.clone(),
                detail: detail.clone(),
            }),
            SessionEventKind::TranscriptReplaced { rows } => {
                thread.rows = rows.clone();
            }
        }
    }
    thread
}

#[must_use]
pub fn render_session_event_timeline(thread_name: &str, events: &[SessionEvent]) -> String {
    render_session_event_timeline_with_mode(thread_name, events, "full", None, None)
}

#[must_use]
pub fn render_session_event_timeline_with_mode(
    thread_name: &str,
    events: &[SessionEvent],
    mode: &str,
    filter: Option<&str>,
    limit: Option<usize>,
) -> String {
    if events.is_empty() {
        return "Timeline is empty.".to_string();
    }

    let filtered = filter
        .map(|filter| {
            events
                .iter()
                .filter(|event| event_matches_filter(&event.kind, filter))
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| events.to_vec());

    let visible = if mode.eq_ignore_ascii_case("recent") {
        let window = limit.unwrap_or(10);
        let start = filtered.len().saturating_sub(window);
        filtered[start..].to_vec()
    } else {
        filtered
    };

    if visible.is_empty() {
        return "Timeline is empty.".to_string();
    }

    let mut lines = vec![format!(
        "## Timeline\n- thread: {}\n- events: {}\n- mode: {}",
        thread_name,
        visible.len(),
        mode
    )];

    for (index, event) in visible.iter().enumerate() {
        lines.push(format!(
            "{}. {}",
            index + 1,
            summarize_event_kind(&event.kind)
        ));
    }

    lines.join("\n")
}

fn row_event(row: TranscriptRow) -> SessionEventKind {
    SessionEventKind::RowAppended {
        row_kind: row.kind,
        text: row.text,
        detail: row.detail,
    }
}

fn summarize_event_kind(kind: &SessionEventKind) -> String {
    match kind {
        SessionEventKind::ThreadCreated { thread_name, cwd } => {
            format!("[thread] created '{thread_name}' in {cwd}")
        }
        SessionEventKind::ThreadRenamed { from, to } => {
            format!("[thread] renamed from '{from}' to '{to}'")
        }
        SessionEventKind::ModelChanged { from, to } => {
            format!(
                "[model] {} -> {}",
                from.as_deref().unwrap_or("unset"),
                to.as_deref().unwrap_or("unset")
            )
        }
        SessionEventKind::ApprovalModeChanged { from, to } => {
            format!("[approvals] {} -> {}", from.label(), to.label())
        }
        SessionEventKind::CwdChanged { from, to } => {
            format!("[cwd] {} -> {}", from, to)
        }
        SessionEventKind::RowAppended { row_kind, text, .. } => {
            let kind = match row_kind {
                RowKind::System => "system",
                RowKind::User => "user",
                RowKind::Assistant => "assistant",
                RowKind::Tool => "tool",
            };
            let summary = text
                .lines()
                .next()
                .unwrap_or_default()
                .trim()
                .chars()
                .take(100)
                .collect::<String>();
            format!("[{kind}] {summary}")
        }
        SessionEventKind::TranscriptReplaced { rows } => {
            format!("[transcript] replaced with {} row(s)", rows.len())
        }
    }
}

fn event_matches_filter(kind: &SessionEventKind, filter: &str) -> bool {
    match filter.to_ascii_lowercase().as_str() {
        "thread" => matches!(
            kind,
            SessionEventKind::ThreadCreated { .. } | SessionEventKind::ThreadRenamed { .. }
        ),
        "model" => matches!(kind, SessionEventKind::ModelChanged { .. }),
        "approvals" => matches!(kind, SessionEventKind::ApprovalModeChanged { .. }),
        "workspace" | "cwd" => matches!(kind, SessionEventKind::CwdChanged { .. }),
        "user" => matches!(
            kind,
            SessionEventKind::RowAppended {
                row_kind: RowKind::User,
                ..
            }
        ),
        "assistant" => matches!(
            kind,
            SessionEventKind::RowAppended {
                row_kind: RowKind::Assistant,
                ..
            }
        ),
        "tool" => matches!(
            kind,
            SessionEventKind::RowAppended {
                row_kind: RowKind::Tool,
                ..
            }
        ),
        "system" => matches!(
            kind,
            SessionEventKind::RowAppended {
                row_kind: RowKind::System,
                ..
            }
        ),
        "transcript" => matches!(kind, SessionEventKind::TranscriptReplaced { .. }),
        _ => false,
    }
}

fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
