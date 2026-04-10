use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SideAgentStatus {
    Running,
    Completed,
    Stopped,
    Failed,
}

impl SideAgentStatus {
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredSideAgentJob {
    pub id: String,
    pub prompt: String,
    pub cwd: String,
    pub model: String,
    pub status: SideAgentStatus,
    pub output_path: String,
    pub stderr_path: String,
    #[serde(default)]
    pub events_path: String,
    pub created_at_epoch_seconds: u64,
    pub finished_at_epoch_seconds: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct SideAgentStorePayload {
    jobs: Vec<StoredSideAgentJob>,
}

pub struct SideAgentStore {
    path: PathBuf,
    jobs: Vec<StoredSideAgentJob>,
}

impl SideAgentStore {
    pub fn open_at(path: PathBuf) -> io::Result<Self> {
        let jobs = if path.exists() {
            let raw = fs::read_to_string(&path)?;
            if raw.trim().is_empty() {
                Vec::new()
            } else {
                serde_json::from_str::<SideAgentStorePayload>(&raw)
                    .map(|payload| payload.jobs)
                    .map_err(|error| invalid_data_error(&path, error))?
            }
        } else {
            Vec::new()
        };

        Ok(Self { path, jobs })
    }

    pub fn create_job(
        &mut self,
        cwd: impl AsRef<Path>,
        prompt: impl Into<String>,
        model: impl Into<String>,
        output_path: impl AsRef<Path>,
        stderr_path: impl AsRef<Path>,
    ) -> io::Result<StoredSideAgentJob> {
        let now = now_epoch_seconds();
        let job = StoredSideAgentJob {
            id: generate_agent_id(),
            prompt: prompt.into(),
            cwd: cwd.as_ref().display().to_string(),
            model: model.into(),
            status: SideAgentStatus::Running,
            output_path: output_path.as_ref().display().to_string(),
            stderr_path: stderr_path.as_ref().display().to_string(),
            events_path: String::new(),
            created_at_epoch_seconds: now,
            finished_at_epoch_seconds: None,
        };
        self.jobs.push(job.clone());
        self.persist()?;
        Ok(job)
    }

    pub fn create_job_in_dir(
        &mut self,
        cwd: impl AsRef<Path>,
        prompt: impl Into<String>,
        model: impl Into<String>,
        agents_dir: impl AsRef<Path>,
    ) -> io::Result<StoredSideAgentJob> {
        let id = generate_agent_id();
        let job_dir = agents_dir.as_ref().join(&id);
        fs::create_dir_all(&job_dir)?;
        self.insert_job(StoredSideAgentJob {
            id,
            prompt: prompt.into(),
            cwd: cwd.as_ref().display().to_string(),
            model: model.into(),
            status: SideAgentStatus::Running,
            output_path: job_dir.join("last-message.md").display().to_string(),
            stderr_path: job_dir.join("stderr.log").display().to_string(),
            events_path: job_dir.join("events.jsonl").display().to_string(),
            created_at_epoch_seconds: now_epoch_seconds(),
            finished_at_epoch_seconds: None,
        })
    }

    fn insert_job(&mut self, job: StoredSideAgentJob) -> io::Result<StoredSideAgentJob> {
        self.jobs.push(job.clone());
        self.persist()?;
        Ok(job)
    }

    pub fn mark_finished(&mut self, id: &str, status: SideAgentStatus) -> io::Result<()> {
        if let Some(job) = self.jobs.iter_mut().find(|job| job.id == id) {
            job.status = status;
            job.finished_at_epoch_seconds = Some(now_epoch_seconds());
            self.persist()?;
        }
        Ok(())
    }

    #[must_use]
    pub fn job(&self, id: &str) -> Option<StoredSideAgentJob> {
        self.jobs.iter().find(|job| job.id == id).cloned()
    }

    #[must_use]
    pub fn list_jobs(&self) -> Vec<StoredSideAgentJob> {
        let mut jobs = self.jobs.clone();
        jobs.sort_by(|left, right| {
            right
                .created_at_epoch_seconds
                .cmp(&left.created_at_epoch_seconds)
                .then_with(|| right.id.cmp(&left.id))
        });
        jobs
    }

    fn persist(&self) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let payload = SideAgentStorePayload {
            jobs: self.list_jobs(),
        };
        let data = serde_json::to_string_pretty(&payload).map_err(io::Error::other)?;
        fs::write(&self.path, data)
    }
}

fn invalid_data_error(path: &Path, error: serde_json::Error) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("failed to parse {}: {error}", path.display()),
    )
}

fn generate_agent_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("agent-{now}-{}", std::process::id())
}

fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn summarize_side_agent_events(path: &Path, limit: usize) -> io::Result<Vec<String>> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };

    let mut out = Vec::new();
    for line in raw.lines() {
        if out.len() >= limit {
            break;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(summary) = summarize_event_value(&value) {
            out.push(summary);
        }
    }
    Ok(out)
}

fn summarize_event_value(value: &Value) -> Option<String> {
    let event_type = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let item = value.get("item");
    let item_type = item
        .and_then(|item| item.get("type"))
        .and_then(Value::as_str)
        .unwrap_or_default();

    match (event_type, item_type) {
        ("item.started", "command_execution") => {
            let command = item
                .and_then(|item| item.get("command"))
                .and_then(Value::as_str)
                .unwrap_or("command");
            Some(format!("command started: {command}"))
        }
        ("item.completed", "command_execution") => Some("command completed".to_string()),
        ("item.completed", "agent_message") => Some("assistant response captured".to_string()),
        ("turn.completed", _) => Some("turn completed".to_string()),
        ("error", _) => value
            .get("message")
            .and_then(Value::as_str)
            .map(|message| format!("error: {message}")),
        _ => None,
    }
}
