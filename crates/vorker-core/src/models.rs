use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::events::SupervisorEvent;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PreflightRecord {
    pub run_id: String,
    pub repo_input: String,
    pub repo_source_type: String,
    pub repo_origin: Option<String>,
    pub repo_path: Option<String>,
    pub classification: Option<String>,
    pub classification_confidence: Option<String>,
    pub strategy: Option<String>,
    pub runtime_family: Option<String>,
    pub package_manager: Option<String>,
    pub risk_level: Option<String>,
    pub risk_reasons: Vec<String>,
    pub sandbox_backend: Option<String>,
    pub sandbox_state: Option<String>,
    pub stage: String,
    pub outcome: Option<String>,
    pub preview_url: Option<String>,
    pub latest_failure: Option<String>,
    pub artifacts_dir: Option<String>,
    pub patch_diff_path: Option<String>,
    pub summary_path: Option<String>,
    pub report_path: Option<String>,
    pub metadata_path: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptEntry {
    pub role: String,
    pub text: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptItem {
    pub kind: String,
    pub role: Option<String>,
    pub text: String,
    pub session_id: Option<String>,
    pub run_id: Option<String>,
    pub task_id: Option<String>,
    pub status: Option<String>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecord {
    pub id: String,
    pub name: String,
    pub role: String,
    pub status: String,
    pub mode: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub cwd: String,
    pub transcript: Vec<TranscriptEntry>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecord {
    pub id: String,
    pub run_id: String,
    pub parent_task_id: Option<String>,
    pub title: String,
    pub description: String,
    pub status: String,
    pub assigned_agent_id: Option<String>,
    pub template_agent_id: Option<String>,
    pub execution_agent_id: Option<String>,
    pub workspace_path: Option<String>,
    pub branch_name: Option<String>,
    pub base_branch: Option<String>,
    pub commit_sha: Option<String>,
    pub change_count: i64,
    pub changed_files: Vec<String>,
    pub merge_status: Option<String>,
    pub merge_commit_sha: Option<String>,
    pub merge_error: Option<String>,
    pub merged_at: Option<String>,
    pub output_text: String,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RunRecord {
    pub id: String,
    pub name: String,
    pub goal: String,
    pub status: String,
    pub notes: String,
    #[serde(rename = "type")]
    pub run_type: Option<String>,
    pub worker_agent_ids: Vec<String>,
    pub arbitrator_agent_id: Option<String>,
    pub task_ids: Vec<String>,
    pub preflight: Option<PreflightRecord>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RunSnapshot {
    pub id: String,
    pub name: String,
    pub goal: String,
    pub status: String,
    pub notes: String,
    #[serde(rename = "type")]
    pub run_type: Option<String>,
    pub worker_agent_ids: Vec<String>,
    pub arbitrator_agent_id: Option<String>,
    pub task_ids: Vec<String>,
    pub preflight: Option<PreflightRecord>,
    pub created_at: String,
    pub updated_at: String,
    pub tasks: Vec<TaskRecord>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    pub runs: Vec<RunSnapshot>,
    pub tasks: Vec<TaskRecord>,
    pub sessions: Vec<SessionRecord>,
    pub transcript_items: Vec<TranscriptItem>,
    pub skills: Vec<Value>,
    pub share: Option<Value>,
    pub events: Vec<SupervisorEvent>,
}
