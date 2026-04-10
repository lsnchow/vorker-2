use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::render::TranscriptRow;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    #[default]
    Manual,
    Auto,
}

impl ApprovalMode {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Manual => "manual approvals",
            Self::Auto => "auto approvals",
        }
    }

    #[must_use]
    pub fn toggled(self) -> Self {
        match self {
            Self::Manual => Self::Auto,
            Self::Auto => Self::Manual,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredThread {
    pub id: String,
    pub name: String,
    pub cwd: String,
    pub rows: Vec<TranscriptRow>,
    pub model: Option<String>,
    pub approval_mode: ApprovalMode,
    pub created_at_epoch_seconds: u64,
    pub updated_at_epoch_seconds: u64,
    pub total_active_seconds: u64,
}

impl StoredThread {
    #[must_use]
    pub fn ephemeral(cwd: impl Into<String>) -> Self {
        let now = now_epoch_seconds();
        Self {
            id: generate_thread_id(),
            name: "Thread 1".to_string(),
            cwd: cwd.into(),
            rows: Vec::new(),
            model: None,
            approval_mode: ApprovalMode::Manual,
            created_at_epoch_seconds: now,
            updated_at_epoch_seconds: now,
            total_active_seconds: 0,
        }
    }

    pub fn touch(&mut self) {
        self.updated_at_epoch_seconds = now_epoch_seconds();
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct ThreadStorePayload {
    threads: Vec<StoredThread>,
}

pub struct ThreadStore {
    path: PathBuf,
    threads: Vec<StoredThread>,
}

impl ThreadStore {
    pub fn open_default() -> io::Result<Self> {
        Self::open_at(default_store_path())
    }

    pub fn open_at(path: PathBuf) -> io::Result<Self> {
        let threads = if path.exists() {
            let raw = fs::read_to_string(&path)?;
            if raw.trim().is_empty() {
                Vec::new()
            } else {
                serde_json::from_str::<ThreadStorePayload>(&raw)
                    .map(|payload| payload.threads)
                    .unwrap_or_default()
            }
        } else {
            Vec::new()
        };

        Ok(Self { path, threads })
    }

    #[must_use]
    pub fn create_thread(&self, cwd: impl AsRef<Path>) -> StoredThread {
        let cwd = cwd.as_ref().display().to_string();
        let count = self
            .threads
            .iter()
            .filter(|thread| thread.cwd == cwd)
            .count()
            + 1;
        let now = now_epoch_seconds();
        StoredThread {
            id: generate_thread_id(),
            name: format!("Thread {count}"),
            cwd,
            rows: Vec::new(),
            model: None,
            approval_mode: ApprovalMode::Manual,
            created_at_epoch_seconds: now,
            updated_at_epoch_seconds: now,
            total_active_seconds: 0,
        }
    }

    pub fn upsert(&mut self, mut thread: StoredThread) -> io::Result<()> {
        thread.touch();
        if let Some(existing) = self.threads.iter_mut().find(|entry| entry.id == thread.id) {
            *existing = thread;
        } else {
            self.threads.push(thread);
        }
        self.persist()
    }

    #[must_use]
    pub fn thread(&self, thread_id: &str) -> Option<StoredThread> {
        self.threads
            .iter()
            .find(|entry| entry.id == thread_id)
            .cloned()
    }

    #[must_use]
    pub fn list_threads(&self) -> Vec<StoredThread> {
        let mut threads = self.threads.clone();
        threads.sort_by(|left, right| {
            right
                .updated_at_epoch_seconds
                .cmp(&left.updated_at_epoch_seconds)
                .then_with(|| left.name.cmp(&right.name))
        });
        threads
    }

    #[must_use]
    pub fn latest_for_cwd(&self, cwd: impl AsRef<Path>) -> Option<StoredThread> {
        let cwd = cwd.as_ref().display().to_string();
        self.list_threads()
            .into_iter()
            .find(|thread| thread.cwd == cwd)
    }

    fn persist(&self) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let payload = ThreadStorePayload {
            threads: self.list_threads(),
        };
        let data = serde_json::to_string_pretty(&payload).map_err(io::Error::other)?;
        fs::write(&self.path, data)
    }
}

fn default_store_path() -> PathBuf {
    if let Some(path) = std::env::var_os("VORKER_THREAD_STORE_PATH") {
        return PathBuf::from(path);
    }

    let base = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join(".vorker-2").join("threads.json")
}

fn generate_thread_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("thread-{now}-{}", std::process::id())
}

fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
