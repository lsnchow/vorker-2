use std::collections::{HashMap, HashSet};

use serde::Deserialize;
use serde_json::Value;

use crate::events::{SupervisorEvent, now_iso};
use crate::models::{RunRecord, RunSnapshot, SessionRecord, Snapshot, TaskRecord, TranscriptEntry};

#[derive(Debug, Default, Clone)]
pub struct SupervisorStore {
    events: Vec<SupervisorEvent>,
    runs: HashMap<String, RunRecord>,
    tasks: HashMap<String, TaskRecord>,
    sessions: HashMap<String, SessionRecord>,
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
                worker_agent_ids: run.worker_agent_ids,
                arbitrator_agent_id: run.arbitrator_agent_id,
                task_ids: run.task_ids,
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
                }
            }
            "task.created" | "task.updated" => {
                if let Ok(payload) = serde_json::from_value::<TaskPayload>(event.payload.clone()) {
                    self.apply_task(payload.task);
                }
            }
            "session.registered" | "session.updated" => {
                if let Ok(payload) = serde_json::from_value::<SessionPayload>(event.payload.clone())
                {
                    self.apply_session(payload.session);
                }
            }
            "session.prompt.started" | "session.prompt.finished" => {
                if let Ok(payload) = serde_json::from_value::<PromptPayload>(event.payload.clone())
                {
                    self.append_transcript(payload.session_id, payload.message);
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
            worker_agent_ids: Vec::new(),
            arbitrator_agent_id: None,
            task_ids: Vec::new(),
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
            worker_agent_ids: normalize_vec(input.worker_agent_ids)
                .unwrap_or(current.worker_agent_ids),
            arbitrator_agent_id: input
                .arbitrator_agent_id
                .and_then(normalize_string)
                .or(current.arbitrator_agent_id),
            task_ids,
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
                worker_agent_ids: Vec::new(),
                arbitrator_agent_id: None,
                task_ids: Vec::new(),
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
                model: input.model.and_then(normalize_string).or(current.model),
                cwd: normalize_option(input.cwd).unwrap_or(current.cwd),
                transcript: current.transcript,
                created_at: normalize_option(input.created_at).unwrap_or(current.created_at),
                updated_at: normalize_option(input.updated_at).unwrap_or(current.updated_at),
            },
        );
    }

    fn append_transcript(&mut self, session_id: String, message: PromptMessage) {
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
                model: None,
                cwd: String::new(),
                transcript: Vec::new(),
                created_at: now_iso(),
                updated_at: now_iso(),
            });

        let mut transcript = current.transcript;
        transcript.push(TranscriptEntry {
            role: normalize_option(Some(message.role)).unwrap_or_else(|| "assistant".into()),
            text: message.text,
        });

        self.sessions.insert(
            session_id.clone(),
            SessionRecord {
                transcript,
                updated_at: now_iso(),
                ..current
            },
        );
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
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunInput {
    id: Option<String>,
    name: Option<String>,
    goal: Option<String>,
    status: Option<String>,
    notes: Option<String>,
    worker_agent_ids: Option<Vec<String>>,
    arbitrator_agent_id: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
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
