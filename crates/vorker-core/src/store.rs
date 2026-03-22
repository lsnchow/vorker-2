use std::collections::{HashMap, HashSet};

use serde::Deserialize;
use serde_json::Value;

use crate::events::{SupervisorEvent, now_iso};
use crate::models::{
    PreflightRecord, RunRecord, RunSnapshot, SessionRecord, Snapshot, TaskRecord, TranscriptEntry,
    TranscriptItem,
};

#[derive(Debug, Default, Clone)]
pub struct SupervisorStore {
    events: Vec<SupervisorEvent>,
    runs: HashMap<String, RunRecord>,
    tasks: HashMap<String, TaskRecord>,
    sessions: HashMap<String, SessionRecord>,
    transcript_items: Vec<TranscriptItem>,
    skills: Vec<Value>,
    share: Option<Value>,
}

impl SupervisorStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn append(&mut self, event: SupervisorEvent) {
        self.events.push(event.clone());
        self.apply(&event);
    }

    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        let tasks = sort_tasks(self.tasks.values().cloned().collect());
        let mut runs: Vec<_> = self.runs.values().cloned().collect();
        runs.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));

        let runs = runs
            .into_iter()
            .map(|run| RunSnapshot {
                tasks: tasks
                    .iter()
                    .filter(|task| task.run_id == run.id)
                    .cloned()
                    .collect(),
                id: run.id,
                name: run.name,
                goal: run.goal,
                status: run.status,
                notes: run.notes,
                run_type: run.run_type,
                worker_agent_ids: run.worker_agent_ids,
                arbitrator_agent_id: run.arbitrator_agent_id,
                task_ids: run.task_ids,
                preflight: run.preflight,
                created_at: run.created_at,
                updated_at: run.updated_at,
            })
            .collect();

        let mut sessions: Vec<_> = self.sessions.values().cloned().collect();
        sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));

        Snapshot {
            runs,
            tasks,
            sessions,
            transcript_items: self.transcript_items.clone(),
            skills: self.skills.clone(),
            share: self.share.clone(),
            events: self.events.clone(),
        }
    }

    fn apply(&mut self, event: &SupervisorEvent) {
        match event.kind.as_str() {
            "run.created" | "run.updated" => {
                if let Ok(payload) = serde_json::from_value::<RunPayload>(event.payload.clone()) {
                    self.apply_run(payload.run);
                    if let Some(preflight) = payload.preflight {
                        self.append_preflight_transcript_item(
                            event.kind.as_str(),
                            &preflight,
                            &event.timestamp,
                        );
                        self.apply_preflight(preflight);
                    }
                }
            }
            kind if kind.starts_with("preflight.") => {
                if let Ok(payload) =
                    serde_json::from_value::<PreflightEventPayload>(event.payload.clone())
                {
                    if let Some(run) = payload.run {
                        self.apply_run(run);
                    }
                    self.append_preflight_transcript_item(
                        event.kind.as_str(),
                        &payload.preflight,
                        &event.timestamp,
                    );
                    self.apply_preflight(payload.preflight);
                }
            }
            "task.created" | "task.updated" => {
                if let Ok(payload) = serde_json::from_value::<TaskPayload>(event.payload.clone()) {
                    self.append_task_transcript_item(
                        event.kind.as_str(),
                        &payload.task,
                        &event.timestamp,
                    );
                    self.apply_task(payload.task);
                }
            }
            "session.registered" | "session.updated" => {
                if let Ok(payload) = serde_json::from_value::<SessionPayload>(event.payload.clone())
                {
                    let session = payload.session;
                    let system_notice = if event.kind == "session.registered" {
                        Some(TranscriptItem {
                            kind: "system_notice".into(),
                            role: None,
                            text: format!(
                                "session {} ready",
                                session
                                    .name
                                    .clone()
                                    .or(session.id.clone())
                                    .unwrap_or_else(|| "unknown".into())
                            ),
                            session_id: session.id.clone(),
                            run_id: None,
                            task_id: None,
                            status: session.status.clone(),
                            timestamp: event.timestamp.clone(),
                        })
                    } else {
                        None
                    };
                    self.apply_session(session);
                    if event.kind == "session.registered" {
                        self.append_transcript_item(system_notice.expect("system notice exists"));
                    }
                }
            }
            "session.prompt.started" | "session.prompt.finished" => {
                if let Ok(payload) = serde_json::from_value::<PromptPayload>(event.payload.clone())
                {
                    self.append_transcript(payload.session_id, payload.message, &event.timestamp);
                }
            }
            "skills.updated" => {
                if let Ok(payload) = serde_json::from_value::<SkillsPayload>(event.payload.clone())
                {
                    self.skills = payload.skills;
                }
            }
            "share.updated" => {
                if let Ok(payload) = serde_json::from_value::<SharePayload>(event.payload.clone()) {
                    self.share = payload.share;
                    if let Some(share) = self.share.as_ref() {
                        let state = share
                            .get("state")
                            .and_then(Value::as_str)
                            .unwrap_or("idle")
                            .to_string();
                        let public_url = share
                            .get("publicUrl")
                            .and_then(Value::as_str)
                            .map(|url| format!(" ({url})"))
                            .unwrap_or_default();
                        self.append_transcript_item(TranscriptItem {
                            kind: "system_notice".into(),
                            role: None,
                            text: format!("share {state}{public_url}"),
                            session_id: None,
                            run_id: None,
                            task_id: None,
                            status: Some(state),
                            timestamp: event.timestamp.clone(),
                        });
                    }
                }
            }
            _ => {}
        }
    }

    fn apply_run(&mut self, input: RunInput) {
        let Some(id) = normalize_option(input.id) else {
            return;
        };

        let current = self.runs.get(&id).cloned().unwrap_or_else(|| RunRecord {
            id: id.clone(),
            name: "Untitled run".into(),
            goal: String::new(),
            status: "draft".into(),
            notes: String::new(),
            run_type: None,
            worker_agent_ids: Vec::new(),
            arbitrator_agent_id: None,
            task_ids: Vec::new(),
            preflight: None,
            created_at: input.created_at.clone().unwrap_or_else(now_iso),
            updated_at: input.updated_at.clone().unwrap_or_else(now_iso),
        });

        let task_ids = current.task_ids.clone();
        let next = RunRecord {
            id: id.clone(),
            name: normalize_option(input.name).unwrap_or(current.name),
            goal: normalize_option(input.goal).unwrap_or(current.goal),
            status: normalize_option(input.status).unwrap_or(current.status),
            notes: normalize_option(input.notes).unwrap_or(current.notes),
            run_type: input
                .run_type
                .and_then(normalize_string)
                .or(current.run_type),
            worker_agent_ids: normalize_vec(input.worker_agent_ids)
                .unwrap_or(current.worker_agent_ids),
            arbitrator_agent_id: input
                .arbitrator_agent_id
                .and_then(normalize_string)
                .or(current.arbitrator_agent_id),
            task_ids,
            preflight: current.preflight,
            created_at: normalize_option(input.created_at).unwrap_or(current.created_at),
            updated_at: normalize_option(input.updated_at).unwrap_or(current.updated_at),
        };

        self.runs.insert(id, next);
    }

    fn apply_task(&mut self, input: TaskInput) {
        let Some(id) = normalize_option(input.id) else {
            return;
        };
        let Some(run_id) = normalize_option(input.run_id) else {
            return;
        };

        let current = self.tasks.get(&id).cloned().unwrap_or_else(|| TaskRecord {
            id: id.clone(),
            run_id: run_id.clone(),
            parent_task_id: None,
            title: "Untitled task".into(),
            description: String::new(),
            status: "draft".into(),
            assigned_agent_id: None,
            template_agent_id: None,
            execution_agent_id: None,
            workspace_path: None,
            branch_name: None,
            base_branch: None,
            commit_sha: None,
            change_count: 0,
            changed_files: Vec::new(),
            merge_status: None,
            merge_commit_sha: None,
            merge_error: None,
            merged_at: None,
            output_text: String::new(),
            error: None,
            created_at: input.created_at.clone().unwrap_or_else(now_iso),
            updated_at: input.updated_at.clone().unwrap_or_else(now_iso),
        });

        let next = TaskRecord {
            id: id.clone(),
            run_id: run_id.clone(),
            parent_task_id: input
                .parent_task_id
                .and_then(normalize_string)
                .or(current.parent_task_id),
            title: normalize_option(input.title).unwrap_or(current.title),
            description: normalize_option(input.description).unwrap_or(current.description),
            status: normalize_option(input.status).unwrap_or(current.status),
            assigned_agent_id: input
                .assigned_agent_id
                .and_then(normalize_string)
                .or(current.assigned_agent_id),
            template_agent_id: input
                .template_agent_id
                .and_then(normalize_string)
                .or(current.template_agent_id),
            execution_agent_id: input
                .execution_agent_id
                .and_then(normalize_string)
                .or(current.execution_agent_id),
            workspace_path: input
                .workspace_path
                .and_then(normalize_string)
                .or(current.workspace_path),
            branch_name: input
                .branch_name
                .and_then(normalize_string)
                .or(current.branch_name),
            base_branch: input
                .base_branch
                .and_then(normalize_string)
                .or(current.base_branch),
            commit_sha: input
                .commit_sha
                .and_then(normalize_string)
                .or(current.commit_sha),
            change_count: input.change_count.unwrap_or(current.change_count),
            changed_files: normalize_vec(input.changed_files).unwrap_or(current.changed_files),
            merge_status: input
                .merge_status
                .and_then(normalize_string)
                .or(current.merge_status),
            merge_commit_sha: input
                .merge_commit_sha
                .and_then(normalize_string)
                .or(current.merge_commit_sha),
            merge_error: input
                .merge_error
                .and_then(normalize_string)
                .or(current.merge_error),
            merged_at: input
                .merged_at
                .and_then(normalize_string)
                .or(current.merged_at),
            output_text: input.output_text.unwrap_or(current.output_text),
            error: input.error.and_then(normalize_string).or(current.error),
            created_at: normalize_option(input.created_at).unwrap_or(current.created_at),
            updated_at: normalize_option(input.updated_at).unwrap_or(current.updated_at),
        };

        self.tasks.insert(id.clone(), next.clone());

        let existing_run = self
            .runs
            .get(&run_id)
            .cloned()
            .unwrap_or_else(|| RunRecord {
                id: run_id.clone(),
                name: "Untitled run".into(),
                goal: String::new(),
                status: "draft".into(),
                notes: String::new(),
                run_type: None,
                worker_agent_ids: Vec::new(),
                arbitrator_agent_id: None,
                task_ids: Vec::new(),
                preflight: None,
                created_at: now_iso(),
                updated_at: now_iso(),
            });
        let mut task_ids: HashSet<_> = existing_run.task_ids.into_iter().collect();
        task_ids.insert(id);
        let mut task_ids: Vec<_> = task_ids.into_iter().collect();
        task_ids.sort();
        self.runs.insert(
            run_id,
            RunRecord {
                task_ids,
                updated_at: next.updated_at.clone(),
                ..existing_run
            },
        );
    }

    fn apply_session(&mut self, input: SessionInput) {
        let Some(id) = normalize_option(input.id) else {
            return;
        };

        let current = self
            .sessions
            .get(&id)
            .cloned()
            .unwrap_or_else(|| SessionRecord {
                id: id.clone(),
                name: id.clone(),
                role: "worker".into(),
                status: "unknown".into(),
                mode: None,
                provider: None,
                model: None,
                cwd: String::new(),
                transcript: Vec::new(),
                created_at: input.created_at.clone().unwrap_or_else(now_iso),
                updated_at: input.updated_at.clone().unwrap_or_else(now_iso),
            });

        self.sessions.insert(
            id.clone(),
            SessionRecord {
                id,
                name: normalize_option(input.name).unwrap_or(current.name),
                role: normalize_option(input.role).unwrap_or(current.role),
                status: normalize_option(input.status).unwrap_or(current.status),
                mode: input.mode.and_then(normalize_string).or(current.mode),
                provider: input
                    .provider
                    .and_then(normalize_string)
                    .or(current.provider),
                model: input.model.and_then(normalize_string).or(current.model),
                cwd: normalize_option(input.cwd).unwrap_or(current.cwd),
                transcript: current.transcript,
                created_at: normalize_option(input.created_at).unwrap_or(current.created_at),
                updated_at: normalize_option(input.updated_at).unwrap_or(current.updated_at),
            },
        );
    }

    fn apply_preflight(&mut self, input: PreflightInput) {
        let Some(run_id) = normalize_option(input.run_id) else {
            return;
        };

        let current_run = self
            .runs
            .get(&run_id)
            .cloned()
            .unwrap_or_else(|| RunRecord {
                id: run_id.clone(),
                name: "Untitled run".into(),
                goal: String::new(),
                status: "draft".into(),
                notes: String::new(),
                run_type: Some("preflight".into()),
                worker_agent_ids: Vec::new(),
                arbitrator_agent_id: None,
                task_ids: Vec::new(),
                preflight: None,
                created_at: now_iso(),
                updated_at: now_iso(),
            });

        let current = current_run.preflight.unwrap_or_else(|| PreflightRecord {
            run_id: run_id.clone(),
            repo_input: String::new(),
            repo_source_type: String::new(),
            stage: "intake".into(),
            ..PreflightRecord::default()
        });

        let next = PreflightRecord {
            run_id: run_id.clone(),
            repo_input: normalize_option(input.repo_input).unwrap_or(current.repo_input),
            repo_source_type: normalize_option(input.repo_source_type)
                .unwrap_or(current.repo_source_type),
            repo_origin: input
                .repo_origin
                .and_then(normalize_string)
                .or(current.repo_origin),
            repo_path: input
                .repo_path
                .and_then(normalize_string)
                .or(current.repo_path),
            classification: input
                .classification
                .and_then(normalize_string)
                .or(current.classification),
            classification_confidence: input
                .classification_confidence
                .and_then(normalize_string)
                .or(current.classification_confidence),
            strategy: input
                .strategy
                .and_then(normalize_string)
                .or(current.strategy),
            runtime_family: input
                .runtime_family
                .and_then(normalize_string)
                .or(current.runtime_family),
            package_manager: input
                .package_manager
                .and_then(normalize_string)
                .or(current.package_manager),
            risk_level: input
                .risk_level
                .and_then(normalize_string)
                .or(current.risk_level),
            risk_reasons: normalize_vec(input.risk_reasons).unwrap_or(current.risk_reasons),
            sandbox_backend: input
                .sandbox_backend
                .and_then(normalize_string)
                .or(current.sandbox_backend),
            sandbox_state: input
                .sandbox_state
                .and_then(normalize_string)
                .or(current.sandbox_state),
            stage: normalize_option(input.stage).unwrap_or(current.stage),
            outcome: input.outcome.and_then(normalize_string).or(current.outcome),
            preview_url: input
                .preview_url
                .and_then(normalize_string)
                .or(current.preview_url),
            latest_failure: input
                .latest_failure
                .and_then(normalize_string)
                .or(current.latest_failure),
            artifacts_dir: input
                .artifacts_dir
                .and_then(normalize_string)
                .or(current.artifacts_dir),
            patch_diff_path: input
                .patch_diff_path
                .and_then(normalize_string)
                .or(current.patch_diff_path),
            summary_path: input
                .summary_path
                .and_then(normalize_string)
                .or(current.summary_path),
            report_path: input
                .report_path
                .and_then(normalize_string)
                .or(current.report_path),
            metadata_path: input
                .metadata_path
                .and_then(normalize_string)
                .or(current.metadata_path),
        };

        self.runs.insert(
            run_id,
            RunRecord {
                run_type: Some("preflight".into()),
                preflight: Some(next),
                updated_at: now_iso(),
                ..current_run
            },
        );
    }

    fn append_transcript(&mut self, session_id: String, message: PromptMessage, timestamp: &str) {
        if normalize_option(Some(message.text.clone())).is_none() {
            return;
        }

        let current = self
            .sessions
            .get(&session_id)
            .cloned()
            .unwrap_or_else(|| SessionRecord {
                id: session_id.clone(),
                name: session_id.clone(),
                role: "worker".into(),
                status: "unknown".into(),
                mode: None,
                provider: None,
                model: None,
                cwd: String::new(),
                transcript: Vec::new(),
                created_at: now_iso(),
                updated_at: now_iso(),
            });

        let mut transcript = current.transcript;
        transcript.push(TranscriptEntry {
            role: normalize_option(Some(message.role.clone()))
                .unwrap_or_else(|| "assistant".into()),
            text: message.text.clone(),
        });

        self.sessions.insert(
            session_id.clone(),
            SessionRecord {
                transcript,
                updated_at: now_iso(),
                ..current
            },
        );

        let role = normalize_option(Some(message.role)).unwrap_or_else(|| "assistant".into());
        let kind = if role == "user" {
            "user_prompt"
        } else {
            "assistant_message"
        };
        self.append_transcript_item(TranscriptItem {
            kind: kind.into(),
            role: Some(role),
            text: message.text,
            session_id: Some(session_id),
            run_id: None,
            task_id: None,
            status: None,
            timestamp: timestamp.to_string(),
        });
    }

    fn append_task_transcript_item(&mut self, kind: &str, task: &TaskInput, timestamp: &str) {
        let status = task
            .status
            .clone()
            .and_then(normalize_string)
            .unwrap_or_else(|| "updated".into());
        let item_kind = match kind {
            "task.created" => "tool_started",
            _ if matches!(status.as_str(), "completed" | "merged" | "verified") => "tool_finished",
            _ => "tool_updated",
        };
        let title = task
            .title
            .clone()
            .and_then(normalize_string)
            .or(task.id.clone().and_then(normalize_string))
            .unwrap_or_else(|| "task".into());
        self.append_transcript_item(TranscriptItem {
            kind: item_kind.into(),
            role: None,
            text: format!("task {title} -> {status}"),
            session_id: None,
            run_id: task.run_id.clone(),
            task_id: task.id.clone(),
            status: Some(status),
            timestamp: timestamp.to_string(),
        });
    }

    fn append_preflight_transcript_item(
        &mut self,
        kind: &str,
        preflight: &PreflightInput,
        timestamp: &str,
    ) {
        let stage = preflight
            .stage
            .clone()
            .and_then(normalize_string)
            .unwrap_or_else(|| "updated".into());
        let item_kind = if matches!(kind, "preflight.verified" | "preflight.completed") {
            "tool_finished"
        } else {
            "tool_updated"
        };
        let risk = preflight
            .risk_level
            .clone()
            .and_then(normalize_string)
            .map(|risk| format!(" risk {risk}"))
            .unwrap_or_default();
        let outcome = preflight
            .outcome
            .clone()
            .and_then(normalize_string)
            .map(|value| format!(" outcome {value}"))
            .unwrap_or_default();
        self.append_transcript_item(TranscriptItem {
            kind: item_kind.into(),
            role: None,
            text: format!("preflight {stage}{risk}{outcome}"),
            session_id: None,
            run_id: preflight.run_id.clone(),
            task_id: None,
            status: preflight.sandbox_state.clone(),
            timestamp: timestamp.to_string(),
        });
    }

    fn append_transcript_item(&mut self, item: TranscriptItem) {
        self.transcript_items.push(item);
    }
}

fn sort_tasks(mut tasks: Vec<TaskRecord>) -> Vec<TaskRecord> {
    tasks.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    tasks
}

fn normalize_option(value: Option<String>) -> Option<String> {
    value.and_then(normalize_string)
}

fn normalize_string(candidate: String) -> Option<String> {
    let trimmed = candidate.trim().to_owned();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn normalize_vec(value: Option<Vec<String>>) -> Option<Vec<String>> {
    value.map(|items| items.into_iter().filter_map(normalize_string).collect())
}

#[derive(Debug, Deserialize)]
struct RunPayload {
    run: RunInput,
    #[serde(default)]
    preflight: Option<PreflightInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunInput {
    id: Option<String>,
    name: Option<String>,
    goal: Option<String>,
    status: Option<String>,
    notes: Option<String>,
    #[serde(rename = "type")]
    run_type: Option<String>,
    worker_agent_ids: Option<Vec<String>>,
    arbitrator_agent_id: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PreflightEventPayload {
    #[serde(default)]
    run: Option<RunInput>,
    preflight: PreflightInput,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreflightInput {
    run_id: Option<String>,
    repo_input: Option<String>,
    repo_source_type: Option<String>,
    repo_origin: Option<String>,
    repo_path: Option<String>,
    classification: Option<String>,
    classification_confidence: Option<String>,
    strategy: Option<String>,
    runtime_family: Option<String>,
    package_manager: Option<String>,
    risk_level: Option<String>,
    risk_reasons: Option<Vec<String>>,
    sandbox_backend: Option<String>,
    sandbox_state: Option<String>,
    stage: Option<String>,
    outcome: Option<String>,
    preview_url: Option<String>,
    latest_failure: Option<String>,
    artifacts_dir: Option<String>,
    patch_diff_path: Option<String>,
    summary_path: Option<String>,
    report_path: Option<String>,
    metadata_path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TaskPayload {
    task: TaskInput,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskInput {
    id: Option<String>,
    run_id: Option<String>,
    parent_task_id: Option<String>,
    title: Option<String>,
    description: Option<String>,
    status: Option<String>,
    assigned_agent_id: Option<String>,
    template_agent_id: Option<String>,
    execution_agent_id: Option<String>,
    workspace_path: Option<String>,
    branch_name: Option<String>,
    base_branch: Option<String>,
    commit_sha: Option<String>,
    change_count: Option<i64>,
    changed_files: Option<Vec<String>>,
    merge_status: Option<String>,
    merge_commit_sha: Option<String>,
    merge_error: Option<String>,
    merged_at: Option<String>,
    output_text: Option<String>,
    error: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SessionPayload {
    session: SessionInput,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionInput {
    id: Option<String>,
    name: Option<String>,
    role: Option<String>,
    status: Option<String>,
    mode: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    cwd: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptPayload {
    session_id: String,
    message: PromptMessage,
}

#[derive(Debug, Deserialize)]
struct PromptMessage {
    role: String,
    text: String,
}

#[derive(Debug, Deserialize)]
struct SkillsPayload {
    skills: Vec<Value>,
}

#[derive(Debug, Deserialize)]
struct SharePayload {
    share: Option<Value>,
}
