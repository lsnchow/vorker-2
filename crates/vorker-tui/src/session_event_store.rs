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
        row_count: usize,
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
                        row_count: next.rows.len(),
                    },
                });
            }
        }
    }

    events
}

fn row_event(row: TranscriptRow) -> SessionEventKind {
    SessionEventKind::RowAppended {
        row_kind: row.kind,
        text: row.text,
        detail: row.detail,
    }
}

fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
