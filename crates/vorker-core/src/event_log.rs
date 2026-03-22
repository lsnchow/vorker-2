use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::events::SupervisorEvent;
use crate::models::Snapshot;
use crate::store::SupervisorStore;

type CoreResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Debug, Clone)]
pub struct EventLog {
    root_dir: PathBuf,
    file_path: PathBuf,
}

impl EventLog {
    #[must_use]
    pub fn new(root_dir: impl AsRef<Path>, file_path: Option<PathBuf>) -> Self {
        let root_dir = root_dir.as_ref().to_path_buf();
        let file_path = file_path.unwrap_or_else(|| root_dir.join("events.ndjson"));
        Self {
            root_dir,
            file_path,
        }
    }

    #[must_use]
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    pub fn append(&self, event: &SupervisorEvent) -> CoreResult<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)?;
        } else {
            fs::create_dir_all(&self.root_dir)?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)?;
        writeln!(file, "{}", serde_json::to_string(event)?)?;
        Ok(())
    }

    pub fn read_all(&self) -> CoreResult<Vec<SupervisorEvent>> {
        match fs::read_to_string(&self.file_path) {
            Ok(raw) => raw
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(|line| Ok(serde_json::from_str::<SupervisorEvent>(line)?))
                .collect(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(Box::new(error)),
        }
    }
}

pub fn restore_durable_supervisor_state(event_log: &EventLog) -> CoreResult<Snapshot> {
    let mut store = SupervisorStore::new();

    for event in event_log.read_all()? {
        if is_durable_event(&event.kind) {
            store.append(event);
        }
    }

    Ok(store.snapshot())
}

fn is_durable_event(kind: &str) -> bool {
    matches!(
        kind,
        "run.created" | "run.updated" | "task.created" | "task.updated"
    ) || kind.starts_with("preflight.")
}
