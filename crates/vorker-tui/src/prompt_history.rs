use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PromptHistoryEntry {
    pub text: String,
    pub created_at_epoch_seconds: u64,
}

pub struct PromptHistoryStore {
    path: PathBuf,
    entries: Vec<PromptHistoryEntry>,
}

impl PromptHistoryStore {
    pub fn open_at(path: PathBuf) -> io::Result<Self> {
        let entries = if path.exists() {
            fs::read_to_string(&path)?
                .lines()
                .filter_map(|line| serde_json::from_str::<PromptHistoryEntry>(line).ok())
                .collect()
        } else {
            Vec::new()
        };

        Ok(Self { path, entries })
    }

    pub fn append(&mut self, text: impl Into<String>) -> io::Result<()> {
        let text = text.into().trim().to_string();
        if text.is_empty() {
            return Ok(());
        }

        self.entries.retain(|entry| entry.text != text);
        self.entries.push(PromptHistoryEntry {
            text,
            created_at_epoch_seconds: now_epoch_seconds(),
        });
        self.persist()
    }

    #[must_use]
    pub fn recent(&self, limit: usize) -> Vec<PromptHistoryEntry> {
        self.entries.iter().rev().take(limit).cloned().collect()
    }

    fn persist(&self) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = String::new();
        for entry in &self.entries {
            let line = serde_json::to_string(entry).map_err(io::Error::other)?;
            out.push_str(&line);
            out.push('\n');
        }
        fs::write(&self.path, out)
    }
}

fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
