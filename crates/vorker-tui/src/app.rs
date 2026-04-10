use crossterm::cursor::MoveTo;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, poll, read};
use crossterm::execute;
use crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode, size,
};
use std::collections::VecDeque;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

use vorker_core::Snapshot;

use crate::boot::{BootStep, boot_minimum_ticks, render_boot_frame};
use crate::bridge::{AcpBridge, BridgeEvent};
use crate::mentions::{
    ComposerMentionBinding, extract_active_mention_query, filter_mention_items,
    insert_selected_mention, prune_mention_bindings, resolve_mention_context,
};
use crate::navigation::{NavigationState, Pane};
use crate::project_workspace::{ProjectWorkspace, render_project_confirmation};
use crate::prompt_history::PromptHistoryStore;
use crate::render::{DashboardOptions, RowKind, TranscriptRow, render_dashboard};
use crate::side_agent_store::{SideAgentStatus, SideAgentStore, summarize_side_agent_events};
use crate::slash::{SlashCommandId, filtered_commands, is_slash_mode};
use crate::thread_store::{ApprovalMode, StoredThread, ThreadStore};
use crate::transcript_export::write_transcript_export;

struct ReviewJob {
    child: Child,
    report_path: PathBuf,
    status_path: PathBuf,
    events_path: PathBuf,
    stderr_path: PathBuf,
    last_status: Option<String>,
    delivered_event_lines: usize,
    streamed_any_rows: bool,
}

struct SideAgentJob {
    id: String,
    prompt: String,
    child: Child,
    output_path: PathBuf,
    stderr_path: PathBuf,
    completed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppCommand {
    NewThread,
    ListThreads,
    SwitchThread {
        thread_id: String,
    },
    ChangeDirectory {
        path: String,
    },
    RunReview {
        focus: String,
        coach: bool,
        apply: bool,
        popout: bool,
        scope: Option<String>,
    },
    RunRalph {
        task: String,
        model: Option<String>,
        no_deslop: bool,
        xhigh: bool,
    },
    ExitShell,
    Stop,
    SteerPrompt {
        prompt_text: String,
    },
    QueuePrompt {
        display_text: String,
        prompt_text: String,
    },
    SpawnAgent {
        prompt_text: String,
    },
    ListAgents,
    StopAgent {
        id: String,
    },
    ShowAgentResult {
        id: String,
    },
    SetTheme {
        theme: String,
    },
    ExportTranscript,
    ShowStatus,
    ListPromptHistory,
    SetModel {
        model: String,
    },
    SubmitPrompt {
        display_text: String,
        prompt_text: String,
    },
    CancelPrompt,
    ResolvePermission {
        option_id: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermissionOptionView {
    pub option_id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PopupMode {
    Mention,
    Permission,
}

pub struct App {
    pub snapshot: Snapshot,
    pub navigation: NavigationState,
    pub status_line: String,
    workspace_path: String,
    current_thread: StoredThread,
    rows: Vec<TranscriptRow>,
    mention_bindings: Vec<ComposerMentionBinding>,
    workspace_files: Vec<String>,
    mention_items: Vec<String>,
    mention_selected_index: usize,
    slash_selected_index: usize,
    permission_title: Option<String>,
    permission_items: Vec<PermissionOptionView>,
    permission_selected_index: usize,
    popup_mode: Option<PopupMode>,
    working_started_at: Option<Instant>,
    needs_replay_context: bool,
    dirty: bool,
    archived_thread: Option<StoredThread>,
    pending_review_rows: VecDeque<TranscriptRow>,
    last_review_reveal_at: Option<Instant>,
    prompt_queue: VecDeque<(String, String)>,
    prompt_history: Vec<String>,
    prompt_history_cursor: Option<usize>,
    shell_theme: String,
    pending_actions: Vec<AppCommand>,
}

impl App {
    #[must_use]
    pub fn new(snapshot: Snapshot) -> Self {
        Self::with_default_model(snapshot, None)
    }

    #[must_use]
    pub fn with_default_model(snapshot: Snapshot, default_model: Option<String>) -> Self {
        let workspace_path = std::env::current_dir()
            .ok()
            .map(|path| path.display().to_string())
            .or_else(|| snapshot.sessions.first().map(|session| session.cwd.clone()))
            .unwrap_or_else(|| ".".to_string());
        let mut thread = StoredThread::ephemeral(workspace_path.clone());
        let selected_model = snapshot
            .sessions
            .first()
            .and_then(|session| session.model.clone())
            .or(default_model);
        thread.model = selected_model.clone();

        let mut navigation = NavigationState::default();
        navigation.focused_pane = Pane::Input;
        navigation.selected_model_id = selected_model.clone();
        navigation.model_choices = selected_model.into_iter().collect();

        let rows: Vec<TranscriptRow> = snapshot
            .sessions
            .first()
            .map(|session| {
                session
                    .transcript
                    .iter()
                    .map(|entry| TranscriptRow {
                        kind: match entry.role.as_str() {
                            "user" => RowKind::User,
                            "assistant" => RowKind::Assistant,
                            "tool" => RowKind::Tool,
                            _ => RowKind::System,
                        },
                        text: entry.text.clone(),
                        detail: None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        thread.rows = rows.clone();

        Self::from_thread(snapshot, thread)
    }

    #[must_use]
    pub fn from_thread(snapshot: Snapshot, thread: StoredThread) -> Self {
        let workspace_path = thread.cwd.clone();
        let selected_model = thread.model.clone();

        let mut navigation = NavigationState::default();
        navigation.focused_pane = Pane::Input;
        navigation.selected_model_id = selected_model.clone();
        navigation.model_choices = selected_model.into_iter().collect();

        Self {
            snapshot,
            navigation,
            status_line: "Ready.".to_string(),
            workspace_path,
            current_thread: thread.clone(),
            rows: thread.rows.clone(),
            mention_bindings: Vec::new(),
            workspace_files: Vec::new(),
            mention_items: Vec::new(),
            mention_selected_index: 0,
            slash_selected_index: 0,
            permission_title: None,
            permission_items: Vec::new(),
            permission_selected_index: 0,
            popup_mode: None,
            working_started_at: None,
            needs_replay_context: !thread.rows.is_empty(),
            dirty: false,
            archived_thread: None,
            pending_review_rows: VecDeque::new(),
            last_review_reveal_at: None,
            prompt_queue: VecDeque::new(),
            prompt_history: Vec::new(),
            prompt_history_cursor: None,
            shell_theme: current_shell_theme().to_string(),
            pending_actions: Vec::new(),
        }
    }

    #[must_use]
    pub fn thread_name(&self) -> &str {
        &self.current_thread.name
    }

    #[must_use]
    pub fn workspace_path(&self) -> &str {
        &self.workspace_path
    }

    #[must_use]
    pub fn approval_mode(&self) -> ApprovalMode {
        self.current_thread.approval_mode
    }

    #[must_use]
    pub fn thread_duration_seconds(&self) -> u64 {
        self.current_thread.total_active_seconds.saturating_add(
            self.working_started_at
                .map(|started_at| started_at.elapsed().as_secs())
                .unwrap_or(0),
        )
    }

    #[must_use]
    pub fn thread_record(&self) -> StoredThread {
        let mut thread = self.current_thread.clone();
        thread.cwd = self.workspace_path.clone();
        thread.rows = self.rows.clone();
        thread.model = self.navigation.selected_model_id.clone();
        thread.total_active_seconds = self.thread_duration_seconds();
        thread
    }

    pub fn take_dirty_thread(&mut self) -> Option<StoredThread> {
        if !self.dirty {
            return None;
        }
        self.dirty = false;
        Some(self.thread_record())
    }

    pub fn take_archived_thread(&mut self) -> Option<StoredThread> {
        self.archived_thread.take()
    }

    pub fn load_thread(&mut self, thread: StoredThread) {
        self.workspace_path = thread.cwd.clone();
        self.current_thread = thread.clone();
        self.rows = thread.rows.clone();
        self.navigation.command_buffer.clear();
        self.navigation.selected_model_id = thread.model.clone();
        self.navigation.model_choices = thread.model.into_iter().collect();
        self.navigation.model_picker_open = false;
        self.mention_bindings.clear();
        self.mention_items.clear();
        self.mention_selected_index = 0;
        self.permission_title = None;
        self.permission_items.clear();
        self.permission_selected_index = 0;
        self.popup_mode = None;
        self.working_started_at = None;
        self.needs_replay_context = !self.rows.is_empty();
        self.dirty = false;
        self.archived_thread = None;
        self.pending_review_rows.clear();
        self.last_review_reveal_at = None;
        self.prompt_history_cursor = None;
    }

    pub fn list_threads(&mut self, threads: &[StoredThread]) {
        if threads.is_empty() {
            self.apply_system_notice("No saved threads yet.");
            return;
        }

        self.apply_system_notice("Saved threads:");
        for thread in threads {
            let current = if thread.id == self.current_thread.id {
                " (current)"
            } else {
                ""
            };
            self.apply_system_notice(format!(
                "{}  {}{} · {} · {}",
                thread.id,
                thread.name,
                current,
                thread.cwd,
                format_thread_duration(thread.total_active_seconds)
            ));
        }
    }

    pub fn set_model_choices(&mut self, model_choices: Vec<String>) {
        self.navigation.model_choices = model_choices;
        if self
            .navigation
            .selected_model_id
            .as_deref()
            .is_none_or(|selected| {
                !self
                    .navigation
                    .model_choices
                    .iter()
                    .any(|item| item == selected)
            })
            && let Some(first) = self.navigation.model_choices.first()
        {
            self.navigation.selected_model_id = Some(first.clone());
        }
    }

    pub fn set_workspace_files(&mut self, workspace_files: Vec<String>) {
        self.workspace_files = workspace_files;
        self.sync_inline_popup();
    }

    pub fn set_prompt_history(&mut self, prompts: Vec<String>) {
        self.prompt_history = prompts;
        self.prompt_history_cursor = None;
    }

    pub fn record_prompt_history(&mut self, prompt: impl Into<String>) {
        let prompt = prompt.into().trim().to_string();
        if prompt.is_empty() {
            return;
        }
        self.prompt_history.retain(|entry| entry != &prompt);
        self.prompt_history.push(prompt);
        self.prompt_history_cursor = None;
    }

    pub fn take_actions(&mut self) -> Vec<AppCommand> {
        std::mem::take(&mut self.pending_actions)
    }

    pub fn queue_prompt(&mut self, display_text: String, prompt_text: String) {
        self.prompt_queue
            .push_back((display_text.clone(), prompt_text));
        self.apply_system_notice(format!("Queued prompt: {display_text}"));
    }

    pub fn apply_assistant_text(&mut self, text: &str) {
        self.dirty = true;
        self.rows.push(TranscriptRow {
            kind: RowKind::Assistant,
            text: text.to_string(),
            detail: None,
        });
    }

    pub fn apply_review_output(&mut self, markdown: &str) {
        self.dirty = true;
        self.pending_review_rows = VecDeque::from(parse_review_markdown(markdown));
        self.last_review_reveal_at = None;
    }

    pub fn pending_review_rows(&self) -> usize {
        self.pending_review_rows.len()
    }

    pub fn advance_review_presentation(&mut self) {
        if let Some(row) = self.pending_review_rows.pop_front() {
            self.rows.push(row);
            self.dirty = true;
            self.last_review_reveal_at = Some(Instant::now());
        }
    }

    pub fn tick(&mut self) {
        if self.pending_review_rows.is_empty() {
            return;
        }

        let should_reveal = self
            .last_review_reveal_at
            .is_none_or(|last| last.elapsed() >= Duration::from_millis(220));
        if should_reveal {
            self.advance_review_presentation();
        }
    }

    pub fn apply_assistant_chunk(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.dirty = true;

        match self.rows.last_mut() {
            Some(last) if last.kind == RowKind::Assistant => last.text.push_str(text),
            _ => self.rows.push(TranscriptRow {
                kind: RowKind::Assistant,
                text: text.to_string(),
                detail: None,
            }),
        }
    }

    pub fn apply_system_notice(&mut self, text: impl Into<String>) {
        self.dirty = true;
        self.rows.push(TranscriptRow {
            kind: RowKind::System,
            text: text.into(),
            detail: None,
        });
    }

    pub fn apply_session_ready(
        &mut self,
        current_model: impl Into<String>,
        mut available_models: Vec<String>,
    ) {
        let current_model = current_model.into();
        if !available_models.iter().any(|model| model == &current_model) {
            available_models.insert(0, current_model.clone());
        }

        self.navigation.selected_model_id = Some(current_model);
        self.set_model_choices(available_models);
        self.dirty = true;
    }

    pub fn apply_model_changed(&mut self, model: impl Into<String>) {
        let model = model.into();
        if !self
            .navigation
            .model_choices
            .iter()
            .any(|item| item == &model)
        {
            self.navigation.model_choices.push(model.clone());
        }
        self.navigation.selected_model_id = Some(model.clone());
        self.apply_system_notice(format!("Model changed to {model}"));
    }

    pub fn apply_tool_notice(&mut self, text: impl Into<String>, detail: Option<String>) {
        self.dirty = true;
        self.rows.push(TranscriptRow {
            kind: RowKind::Tool,
            text: text.into(),
            detail,
        });
    }

    pub fn apply_tool_update(&mut self, detail: impl Into<String>) {
        let detail = detail.into();
        self.dirty = true;
        if let Some(last) = self
            .rows
            .iter_mut()
            .rev()
            .find(|row| row.kind == RowKind::Tool)
        {
            last.detail = Some(detail);
            return;
        }

        self.rows.push(TranscriptRow {
            kind: RowKind::Tool,
            text: "Tool".to_string(),
            detail: Some(detail),
        });
    }

    pub fn finish_prompt(&mut self) {
        if let Some(started_at) = self.working_started_at.take() {
            self.current_thread.total_active_seconds = self
                .current_thread
                .total_active_seconds
                .saturating_add(started_at.elapsed().as_secs());
            self.dirty = true;
        }
        if let Some((display_text, prompt_text)) = self.prompt_queue.pop_front() {
            self.rows.push(TranscriptRow {
                kind: RowKind::User,
                text: display_text.clone(),
                detail: Some("queued follow-up".to_string()),
            });
            self.working_started_at = Some(Instant::now());
            self.pending_actions.push(AppCommand::SubmitPrompt {
                display_text,
                prompt_text,
            });
        }
    }

    pub fn stop_working_timer(&mut self) {
        if let Some(started_at) = self.working_started_at.take() {
            self.current_thread.total_active_seconds = self
                .current_thread
                .total_active_seconds
                .saturating_add(started_at.elapsed().as_secs());
            self.dirty = true;
        }
    }

    pub fn queued_prompt_count(&self) -> usize {
        self.prompt_queue.len()
    }

    pub fn open_permission_prompt(
        &mut self,
        title: impl Into<String>,
        items: Vec<PermissionOptionView>,
    ) {
        self.permission_title = Some(title.into());
        self.permission_items = items;
        self.permission_selected_index = 0;
        self.popup_mode = Some(PopupMode::Permission);
    }

    pub fn render(&self, width: usize, color: bool) -> String {
        render_dashboard(
            &self.snapshot,
            DashboardOptions {
                color,
                width,
                theme_name: self.shell_theme.clone(),
                workspace_path: self.workspace_path.clone(),
                selected_model_id: self.navigation.selected_model_id.clone(),
                model_choices: self.navigation.model_choices.clone(),
                model_picker_open: self.navigation.model_picker_open,
                command_buffer: self.navigation.command_buffer.clone(),
                slash_menu_selected_index: self.slash_selected_index,
                mention_items: self
                    .is_mention_popup()
                    .then(|| self.mention_items.clone())
                    .unwrap_or_default(),
                mention_selected_index: self.mention_selected_index,
                permission_title: self.permission_title.clone(),
                permission_items: self
                    .permission_items
                    .iter()
                    .map(|item| crate::render::PopupItem {
                        label: item.name.clone(),
                        description: None,
                    })
                    .collect(),
                permission_selected_index: self.permission_selected_index,
                context_left_label: "100% left".to_string(),
                approval_mode_label: self.current_thread.approval_mode.label().to_string(),
                thread_duration_label: format!(
                    "{} thread",
                    format_thread_duration(self.thread_duration_seconds())
                ),
                queue_label: format!("queue {}", self.prompt_queue.len()),
                activity_label: if self.working_started_at.is_some() {
                    "working".to_string()
                } else {
                    "idle".to_string()
                },
                working_seconds: self
                    .working_started_at
                    .map(|started_at| started_at.elapsed().as_secs()),
                transcript_rows: self.rows.clone(),
                tip_line: Some(if current_review_mode() {
                    "Tip: Use /model, /coach, or /apply. Esc exits review mode.".to_string()
                } else {
                    "Tip: Use /model or /new.".to_string()
                }),
                composer_placeholder: if current_review_mode() {
                    "Question the current implementation".to_string()
                } else {
                    "Improve documentation in @filename".to_string()
                },
            },
        )
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return false;
        }

        if self.popup_mode == Some(PopupMode::Permission) {
            self.handle_permission_key(key);
            return true;
        }

        if self.navigation.model_picker_open {
            self.handle_model_picker_key(key);
            return true;
        }

        if self.is_mention_popup() {
            self.handle_mention_key(key);
            return true;
        }

        match key.code {
            KeyCode::Esc => self.handle_escape(),
            KeyCode::Tab => self.autocomplete_slash_command(),
            KeyCode::Up => self.navigate_slash(-1),
            KeyCode::Down => self.navigate_slash(1),
            KeyCode::Enter => self.submit_current_input(),
            KeyCode::Backspace => {
                let _ = self.navigation.command_buffer.pop();
                self.prompt_history_cursor = None;
                self.mention_bindings =
                    prune_mention_bindings(&self.navigation.command_buffer, &self.mention_bindings);
                self.sync_inline_popup();
            }
            KeyCode::Char(ch)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.navigation.focused_pane = Pane::Input;
                self.navigation.command_buffer.push(ch);
                self.prompt_history_cursor = None;
                self.sync_inline_popup();
            }
            _ => {}
        }

        true
    }

    fn is_mention_popup(&self) -> bool {
        self.popup_mode == Some(PopupMode::Mention)
    }

    fn handle_permission_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => {
                self.permission_selected_index = cycle_index(
                    self.permission_selected_index,
                    self.permission_items.len(),
                    -1,
                );
            }
            KeyCode::Down => {
                self.permission_selected_index = cycle_index(
                    self.permission_selected_index,
                    self.permission_items.len(),
                    1,
                );
            }
            KeyCode::Enter => {
                let option_id = self
                    .permission_items
                    .get(self.permission_selected_index)
                    .map(|item| item.option_id.clone());
                self.pending_actions
                    .push(AppCommand::ResolvePermission { option_id });
                self.close_permission_prompt();
            }
            KeyCode::Esc => {
                self.pending_actions
                    .push(AppCommand::ResolvePermission { option_id: None });
                self.close_permission_prompt();
            }
            _ => {}
        }
    }

    fn close_permission_prompt(&mut self) {
        self.permission_title = None;
        self.permission_items.clear();
        self.permission_selected_index = 0;
        self.popup_mode = None;
    }

    fn handle_model_picker_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => {
                let current = self
                    .navigation
                    .selected_model_id
                    .as_deref()
                    .and_then(|selected| {
                        self.navigation
                            .model_choices
                            .iter()
                            .position(|item| item == selected)
                    })
                    .unwrap_or(0);
                let index = cycle_index(current, self.navigation.model_choices.len(), -1);
                self.navigation.selected_model_id =
                    self.navigation.model_choices.get(index).cloned();
            }
            KeyCode::Down => {
                let current = self
                    .navigation
                    .selected_model_id
                    .as_deref()
                    .and_then(|selected| {
                        self.navigation
                            .model_choices
                            .iter()
                            .position(|item| item == selected)
                    })
                    .unwrap_or(0);
                let index = cycle_index(current, self.navigation.model_choices.len(), 1);
                self.navigation.selected_model_id =
                    self.navigation.model_choices.get(index).cloned();
            }
            KeyCode::Enter => {
                if let Some(model) = self.navigation.selected_model_id.clone() {
                    self.pending_actions.push(AppCommand::SetModel { model });
                }
                self.navigation.model_picker_open = false;
            }
            KeyCode::Esc => {
                self.navigation.model_picker_open = false;
            }
            _ => {}
        }
    }

    fn handle_mention_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => {
                self.mention_selected_index =
                    cycle_index(self.mention_selected_index, self.mention_items.len(), -1);
            }
            KeyCode::Down => {
                self.mention_selected_index =
                    cycle_index(self.mention_selected_index, self.mention_items.len(), 1);
            }
            KeyCode::Enter => {
                if let Some(selected) = self.mention_items.get(self.mention_selected_index).cloned()
                    && let Some((text, binding)) =
                        insert_selected_mention(&self.navigation.command_buffer, &selected)
                {
                    self.navigation.command_buffer = text;
                    self.mention_bindings = prune_mention_bindings(
                        &self.navigation.command_buffer,
                        &[self.mention_bindings.clone(), vec![binding]].concat(),
                    );
                }
                self.sync_inline_popup();
            }
            KeyCode::Esc => {
                self.popup_mode = None;
                self.mention_items.clear();
                self.mention_selected_index = 0;
            }
            _ => {}
        }
    }

    fn handle_escape(&mut self) {
        if self.navigation.model_picker_open {
            self.navigation.model_picker_open = false;
            return;
        }

        if self.popup_mode.is_some() {
            self.popup_mode = None;
            self.mention_items.clear();
            self.mention_selected_index = 0;
            return;
        }

        if !self.navigation.command_buffer.is_empty() {
            self.navigation.command_buffer.clear();
            self.mention_bindings.clear();
            return;
        }

        if current_review_mode() {
            self.pending_actions.push(AppCommand::ExitShell);
        }
    }

    fn autocomplete_slash_command(&mut self) {
        if !is_slash_mode(&self.navigation.command_buffer) {
            return;
        }

        let commands = filtered_commands(&self.navigation.command_buffer, current_review_mode());
        if let Some(command) = commands.get(self.slash_selected_index) {
            self.navigation.command_buffer = format!("{} ", command.name);
        }
    }

    fn navigate_slash(&mut self, delta: isize) {
        if !is_slash_mode(&self.navigation.command_buffer) {
            self.recall_prompt_history(delta);
            return;
        }

        let commands = filtered_commands(&self.navigation.command_buffer, current_review_mode());
        if commands.is_empty() {
            return;
        }

        self.slash_selected_index = cycle_index(self.slash_selected_index, commands.len(), delta);
    }

    fn recall_prompt_history(&mut self, delta: isize) {
        if self.prompt_history.is_empty() {
            return;
        }

        let next = if delta < 0 {
            Some(match self.prompt_history_cursor {
                Some(0) => 0,
                Some(index) => index.saturating_sub(1),
                None => self.prompt_history.len().saturating_sub(1),
            })
        } else {
            match self.prompt_history_cursor {
                Some(index) if index + 1 < self.prompt_history.len() => Some(index + 1),
                Some(_) => None,
                None => return,
            }
        };

        self.prompt_history_cursor = next;
        self.navigation.command_buffer = next
            .and_then(|index| self.prompt_history.get(index).cloned())
            .unwrap_or_default();
        self.mention_bindings.clear();
        self.sync_inline_popup();
    }

    fn submit_current_input(&mut self) {
        if self.working_started_at.is_some() {
            let display_text = self.navigation.command_buffer.trim().to_string();
            if is_slash_mode(&display_text) {
                self.execute_slash_command(&display_text);
                return;
            }

            if !display_text.is_empty() {
                let prompt_text = self.build_prompt_text(&display_text);
                self.pending_actions.push(AppCommand::QueuePrompt {
                    display_text,
                    prompt_text,
                });
                self.navigation.command_buffer.clear();
                self.mention_bindings.clear();
                self.sync_inline_popup();
            }
            return;
        }

        if self.navigation.model_picker_open {
            self.handle_model_picker_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            return;
        }

        let trimmed = self.navigation.command_buffer.trim().to_string();
        if trimmed.is_empty() {
            return;
        }

        if is_slash_mode(&trimmed) {
            self.execute_slash_command(&trimmed);
            return;
        }

        let display_text = trimmed;
        let prompt_text = self.build_prompt_text(&display_text);
        self.rows.push(TranscriptRow {
            kind: RowKind::User,
            text: display_text.clone(),
            detail: None,
        });
        self.working_started_at = Some(Instant::now());
        self.pending_actions.push(AppCommand::SubmitPrompt {
            display_text,
            prompt_text,
        });
        self.needs_replay_context = false;
        self.dirty = true;
        self.navigation.command_buffer.clear();
        self.mention_bindings.clear();
        self.sync_inline_popup();
    }

    fn build_prompt_text(&mut self, display_text: &str) -> String {
        let mut sections = Vec::new();
        if self.needs_replay_context && !self.rows.is_empty() {
            sections.push(format!(
                "Previous thread transcript:\n{}",
                render_transcript_replay(&self.rows)
            ));
        }

        let context = resolve_mention_context(
            std::path::Path::new(&self.workspace_path),
            &self.mention_bindings,
        );
        for error in &context.errors {
            self.apply_system_notice(error.clone());
        }
        sections.extend(context.sections);

        if sections.is_empty() {
            return display_text.to_string();
        }

        format!(
            "{}\n\nUser request:\n{}",
            sections.join("\n\n"),
            display_text
        )
    }

    fn execute_slash_command(&mut self, buffer: &str) {
        let command = parse_exact_slash_command(buffer).or_else(|| {
            filtered_commands(buffer, current_review_mode())
                .get(self.slash_selected_index)
                .copied()
        });

        self.navigation.command_buffer.clear();
        self.popup_mode = None;
        self.mention_items.clear();
        self.slash_selected_index = 0;

        let Some(command) = command else {
            self.apply_system_notice("Unknown command.");
            return;
        };

        match command.id {
            SlashCommandId::Review => {
                let (coach, apply, popout, scope, focus) = parse_review_command(buffer);
                self.pending_actions.push(AppCommand::RunReview {
                    focus,
                    coach,
                    apply,
                    popout,
                    scope,
                });
            }
            SlashCommandId::Ralph => {
                let (no_deslop, xhigh, model, task) = parse_ralph_command(buffer);
                if task.is_empty() {
                    self.apply_system_notice(
                        "Usage: /ralph [--no-deslop] [--xhigh] [--model <model>] <task>",
                    );
                } else {
                    self.pending_actions.push(AppCommand::RunRalph {
                        task,
                        model,
                        no_deslop,
                        xhigh,
                    });
                }
            }
            SlashCommandId::Stop => {
                self.pending_actions.push(AppCommand::Stop);
            }
            SlashCommandId::Steer => {
                let guidance = command_tail(buffer);
                if guidance.is_empty() {
                    self.apply_system_notice("Usage: /steer <guidance>");
                } else {
                    self.pending_actions.push(AppCommand::SteerPrompt {
                        prompt_text: format!("[STEER]\n{guidance}"),
                    });
                }
            }
            SlashCommandId::Queue => {
                let display_text = command_tail(buffer);
                if display_text.is_empty() {
                    self.apply_system_notice("Usage: /queue <prompt>");
                } else {
                    let prompt_text = self.build_prompt_text(&display_text);
                    self.pending_actions.push(AppCommand::QueuePrompt {
                        display_text,
                        prompt_text,
                    });
                }
            }
            SlashCommandId::Agent => {
                let prompt_text = command_tail(buffer);
                if prompt_text.is_empty() {
                    self.apply_system_notice("Usage: /agent <task>");
                } else {
                    self.pending_actions
                        .push(AppCommand::SpawnAgent { prompt_text });
                }
            }
            SlashCommandId::Agents => {
                self.pending_actions.push(AppCommand::ListAgents);
            }
            SlashCommandId::AgentStop => {
                let id = command_tail(buffer);
                if id.is_empty() {
                    self.apply_system_notice("Usage: /agent-stop <id>");
                } else {
                    self.pending_actions.push(AppCommand::StopAgent { id });
                }
            }
            SlashCommandId::AgentResult => {
                let id = command_tail(buffer);
                if id.is_empty() {
                    self.apply_system_notice("Usage: /agent-result <id>");
                } else {
                    self.pending_actions
                        .push(AppCommand::ShowAgentResult { id });
                }
            }
            SlashCommandId::Theme => {
                let theme = command_tail(buffer);
                if theme.is_empty() {
                    self.apply_system_notice("Usage: /theme <default|review|opencode>");
                } else if theme == "list" {
                    self.apply_system_notice("Themes: default, review, opencode");
                } else {
                    self.pending_actions.push(AppCommand::SetTheme { theme });
                }
            }
            SlashCommandId::Export => {
                self.pending_actions.push(AppCommand::ExportTranscript);
            }
            SlashCommandId::Status => {
                self.pending_actions.push(AppCommand::ShowStatus);
            }
            SlashCommandId::History => {
                self.pending_actions.push(AppCommand::ListPromptHistory);
            }
            SlashCommandId::Coach => {
                self.pending_actions.push(AppCommand::RunReview {
                    focus: String::new(),
                    coach: true,
                    apply: false,
                    popout: false,
                    scope: current_shell_review_scope(),
                });
            }
            SlashCommandId::Apply => {
                self.pending_actions.push(AppCommand::RunReview {
                    focus: String::new(),
                    coach: true,
                    apply: true,
                    popout: false,
                    scope: current_shell_review_scope(),
                });
            }
            SlashCommandId::ExitReview => {
                self.pending_actions.push(AppCommand::ExitShell);
            }
            SlashCommandId::Model => {
                let requested = buffer.split_whitespace().nth(1).map(str::to_string);
                if let Some(model) = requested {
                    self.pending_actions.push(AppCommand::SetModel { model });
                } else {
                    self.navigation.model_picker_open = true;
                }
            }
            SlashCommandId::New => {
                self.archived_thread = Some(self.thread_record());
                self.rows.clear();
                self.working_started_at = None;
                self.dirty = true;
                self.pending_actions.push(AppCommand::NewThread);
            }
            SlashCommandId::Help => {
                self.apply_system_notice(if current_review_mode() {
                    "Commands: /stop /model /coach /apply /exit-review"
                } else {
                    "Commands: /review /ralph /export /status /history /stop /steer /queue /agent /agents /agent-stop /agent-result /theme /model /new /permissions /rename /list /cd /help"
                });
            }
            SlashCommandId::Permissions => {
                self.current_thread.approval_mode = self.current_thread.approval_mode.toggled();
                self.dirty = true;
                self.apply_system_notice(format!(
                    "Permissions set to {}.",
                    self.current_thread.approval_mode.label()
                ));
            }
            SlashCommandId::Rename => {
                let name = buffer
                    .split_once(char::is_whitespace)
                    .map(|(_, tail)| tail.trim())
                    .unwrap_or_default();
                if name.is_empty() {
                    self.apply_system_notice("Usage: /rename <thread name>");
                } else {
                    self.current_thread.name = name.to_string();
                    self.dirty = true;
                    self.apply_system_notice(format!("Renamed thread to {name}."));
                }
            }
            SlashCommandId::List => {
                let thread_id = buffer
                    .split_once(char::is_whitespace)
                    .map(|(_, tail)| tail.trim())
                    .filter(|value| !value.is_empty());
                if let Some(thread_id) = thread_id {
                    self.pending_actions.push(AppCommand::SwitchThread {
                        thread_id: thread_id.to_string(),
                    });
                } else {
                    self.pending_actions.push(AppCommand::ListThreads);
                }
            }
            SlashCommandId::Cd => {
                let path = buffer
                    .split_once(char::is_whitespace)
                    .map(|(_, tail)| tail.trim())
                    .unwrap_or_default();
                if path.is_empty() {
                    self.apply_system_notice("Usage: /cd <project directory>");
                } else {
                    self.pending_actions.push(AppCommand::ChangeDirectory {
                        path: path.to_string(),
                    });
                }
            }
        }
    }

    fn sync_inline_popup(&mut self) {
        self.mention_bindings =
            prune_mention_bindings(&self.navigation.command_buffer, &self.mention_bindings);

        if let Some(query) = extract_active_mention_query(&self.navigation.command_buffer) {
            self.popup_mode = Some(PopupMode::Mention);
            self.mention_items = filter_mention_items(&query, &self.workspace_files);
            self.mention_selected_index = self
                .mention_selected_index
                .min(self.mention_items.len().saturating_sub(1));
            return;
        }

        self.popup_mode = None;
        self.mention_items.clear();
        self.mention_selected_index = 0;
    }
}

fn render_transcript_replay(rows: &[TranscriptRow]) -> String {
    rows.iter()
        .map(|row| {
            let role = match row.kind {
                RowKind::User => "User",
                RowKind::Assistant => "Assistant",
                RowKind::Tool => "Tool",
                RowKind::System => "System",
            };
            let mut line = format!("{role}: {}", row.text);
            if let Some(detail) = &row.detail {
                line.push('\n');
                line.push_str(detail);
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn parse_review_markdown(markdown: &str) -> Vec<TranscriptRow> {
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
                    let snippet = code_lines.join("\n");
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

fn cycle_index(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    ((current as isize + delta).rem_euclid(len as isize)) as usize
}

fn parse_exact_slash_command(buffer: &str) -> Option<crate::slash::SlashCommand> {
    let command = buffer.split_whitespace().next().unwrap_or_default().trim();
    crate::slash::SLASH_COMMANDS
        .iter()
        .copied()
        .find(|entry| entry.matches_exact(command))
}

fn parse_review_command(buffer: &str) -> (bool, bool, bool, Option<String>, String) {
    let mut coach = false;
    let mut apply = false;
    let mut popout = true;
    let mut scope = None;
    let mut focus = Vec::new();

    for token in buffer.split_whitespace().skip(1) {
        match token {
            "--coach" => coach = true,
            "--apply" => apply = true,
            "--popout" => popout = true,
            "--no-popout" => popout = false,
            "--staged" => scope = Some("staged".to_string()),
            "--all-files" => scope = Some("all-files".to_string()),
            value => focus.push(value.to_string()),
        }
    }

    (coach, apply, popout, scope, focus.join(" "))
}

fn parse_ralph_command(buffer: &str) -> (bool, bool, Option<String>, String) {
    let mut no_deslop = false;
    let mut xhigh = false;
    let mut model = None;
    let mut task = Vec::new();
    let mut tokens = buffer.split_whitespace().skip(1);

    while let Some(token) = tokens.next() {
        match token {
            "--no-deslop" => no_deslop = true,
            "--xhigh" => xhigh = true,
            "--model" => {
                if let Some(value) = tokens.next() {
                    model = Some(value.to_string());
                }
            }
            value => task.push(value.to_string()),
        }
    }

    (no_deslop, xhigh, model, task.join(" "))
}

fn command_tail(buffer: &str) -> String {
    buffer
        .split_once(char::is_whitespace)
        .map(|(_, tail)| tail.trim().to_string())
        .unwrap_or_default()
}

fn current_shell_review_scope() -> Option<String> {
    std::env::var("VORKER_REVIEW_SCOPE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn current_review_model() -> String {
    std::env::var("VORKER_REVIEW_MODEL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "gpt-5.3-codex".to_string())
}

fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name)
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes"
    )
}

fn spawn_review_job(
    cwd: &Path,
    model: String,
    scope: Option<String>,
    coach: bool,
    apply: bool,
    focus: &str,
) -> io::Result<ReviewJob> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let report_path = std::env::temp_dir().join(format!("vorker-review-{stamp}.md"));
    let status_path = std::env::temp_dir().join(format!("vorker-review-{stamp}.status"));
    let events_path = std::env::temp_dir().join(format!("vorker-review-{stamp}.events"));
    let stderr_path = std::env::temp_dir().join(format!("vorker-review-{stamp}.stderr"));
    let stderr_file = std::fs::File::create(&stderr_path)?;
    let mut command = std::process::Command::new(std::env::current_exe()?);
    command
        .arg("--cwd")
        .arg(cwd)
        .arg("--model")
        .arg(model)
        .arg("adversarial")
        .arg("--output-report")
        .arg(&report_path)
        .arg("--events-file")
        .arg(&events_path)
        .arg("--status-file")
        .arg(&status_path)
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr_file));
    if let Some(scope) = scope {
        command.arg("--scope").arg(scope);
    }
    if coach {
        command.arg("--coach");
    }
    if apply {
        command.arg("--apply");
    }
    if !focus.trim().is_empty() {
        command.arg(focus);
    }

    let child = command.spawn()?;
    Ok(ReviewJob {
        child,
        report_path,
        status_path,
        events_path,
        stderr_path,
        last_status: None,
        delivered_event_lines: 0,
        streamed_any_rows: false,
    })
}

fn poll_review_job(app: &mut App, review_job: &mut Option<ReviewJob>) -> io::Result<()> {
    let Some(job) = review_job.as_mut() else {
        return Ok(());
    };

    if let Ok(status) = std::fs::read_to_string(&job.status_path) {
        let status = status.trim().to_string();
        if !status.is_empty() && job.last_status.as_deref() != Some(status.as_str()) {
            app.apply_tool_update(status.clone());
            job.last_status = Some(status);
        }
    }

    if let Ok(events) = std::fs::read_to_string(&job.events_path) {
        let lines = events.lines().collect::<Vec<_>>();
        for line in lines.iter().skip(job.delivered_event_lines) {
            if let Ok(row) = serde_json::from_str::<TranscriptRow>(line) {
                app.pending_review_rows.push_back(row);
                job.streamed_any_rows = true;
            }
        }
        job.delivered_event_lines = lines.len();
    }

    if let Some(exit_status) = job.child.try_wait()? {
        app.finish_prompt();
        if exit_status.success() {
            if !job.streamed_any_rows {
                if let Ok(report) = std::fs::read_to_string(&job.report_path)
                    && !report.trim().is_empty()
                {
                    app.apply_review_output(&report);
                } else {
                    app.apply_system_notice("Review finished, but no report was written.");
                }
            } else {
                app.apply_system_notice("Review finished.");
            }
        } else {
            let error = std::fs::read_to_string(&job.stderr_path)
                .ok()
                .map(|text| text.trim().to_string())
                .filter(|text| !text.is_empty())
                .unwrap_or_else(|| "Adversarial review failed.".to_string());
            app.apply_system_notice(error);
        }
        *review_job = None;
    }

    Ok(())
}

fn spawn_side_agent(
    cwd: &Path,
    prompt_text: &str,
    store: &mut SideAgentStore,
    agents_dir: &Path,
) -> io::Result<SideAgentJob> {
    let model = current_review_model();
    let record = store.create_job_in_dir(cwd, prompt_text, &model, agents_dir)?;
    let output_path = PathBuf::from(&record.output_path);
    let stderr_path = PathBuf::from(&record.stderr_path);
    let events_path = PathBuf::from(&record.events_path);
    let events = std::fs::File::create(&events_path)?;
    let stderr = std::fs::File::create(&stderr_path)?;
    let mut command = std::process::Command::new("codex");
    command
        .arg("exec")
        .arg("--model")
        .arg(model)
        .arg("--full-auto")
        .arg("--color")
        .arg("never")
        .arg("--json")
        .arg("--skip-git-repo-check")
        .arg("--output-last-message")
        .arg(&output_path)
        .arg("-C")
        .arg(cwd)
        .arg(prompt_text)
        .stdout(Stdio::from(events))
        .stderr(Stdio::from(stderr));

    match command.spawn() {
        Ok(child) => Ok(SideAgentJob {
            id: record.id,
            prompt: prompt_text.to_string(),
            child,
            output_path,
            stderr_path,
            completed: false,
        }),
        Err(error) => {
            let _ = store.mark_finished(&record.id, SideAgentStatus::Failed);
            Err(error)
        }
    }
}

fn poll_side_agent_jobs(
    app: &mut App,
    jobs: &mut [SideAgentJob],
    store: &mut SideAgentStore,
) -> io::Result<()> {
    for job in jobs.iter_mut().filter(|job| !job.completed) {
        if let Some(status) = job.child.try_wait()? {
            job.completed = true;
            let stored_status = if status.success() {
                SideAgentStatus::Completed
            } else {
                SideAgentStatus::Failed
            };
            store.mark_finished(&job.id, stored_status)?;
            if status.success() {
                app.apply_system_notice(format!("Side agent {} finished with {}.", job.id, status));
            } else {
                let detail = std::fs::read_to_string(&job.stderr_path)
                    .ok()
                    .map(|text| text.trim().to_string())
                    .filter(|text| !text.is_empty())
                    .unwrap_or_else(|| status.to_string());
                app.apply_system_notice(format!("Side agent {} failed: {detail}", job.id));
            }
        }
    }
    Ok(())
}

fn format_agent_result(id: &str, events: &[String], output: &str) -> String {
    let mut sections = vec![format!("Agent {id} result:")];
    if !events.is_empty() {
        sections.push("Events:".to_string());
        sections.extend(events.iter().map(|event| format!("- {event}")));
    }
    sections.push("Output:".to_string());
    sections.push(output.to_string());
    sections.join("\n")
}

fn open_review_window(
    cwd: &Path,
    model: &str,
    scope: Option<String>,
    coach: bool,
    apply: bool,
    focus: &str,
) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let scope = scope.unwrap_or_else(|| "auto".to_string());
        let command = format!(
            "cd '{}' && VORKER_THEME=review VORKER_REVIEW_MODE=1 VORKER_REVIEW_AUTO=1 VORKER_REVIEW_SCOPE={} VORKER_REVIEW_COACH={} VORKER_REVIEW_APPLY={} VORKER_REVIEW_FOCUS='{}' vorker --model {}",
            escape_single_quotes(&cwd.display().to_string()),
            scope,
            if coach { "1" } else { "0" },
            if apply { "1" } else { "0" },
            escape_single_quotes(focus),
            shell_escape_arg(model),
        );
        let script = format!(
            "tell application \"Terminal\" to do script \"{}\"",
            command.replace('\\', "\\\\").replace('"', "\\\"")
        );
        let status = std::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .status()?;
        if status.success() {
            return Ok(());
        }
        return Err(io::Error::other("failed to open review window"));
    }

    #[allow(unreachable_code)]
    Err(io::Error::other(
        "review popout is currently supported on macOS only",
    ))
}

fn open_ralph_window(
    cwd: &Path,
    task: &str,
    model: Option<&str>,
    no_deslop: bool,
    xhigh: bool,
) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let mut args = vec!["ralph".to_string()];
        if no_deslop {
            args.push("--no-deslop".to_string());
        }
        if xhigh {
            args.push("--xhigh".to_string());
        }
        if let Some(model) = model.filter(|model| !model.trim().is_empty()) {
            args.push("--model".to_string());
            args.push(shell_escape_arg(model));
        }
        args.push(shell_escape_arg(task));
        let command = format!(
            "cd '{}' && vorker {}",
            escape_single_quotes(&cwd.display().to_string()),
            args.join(" ")
        );
        let script = format!(
            "tell application \"Terminal\" to do script \"{}\"",
            command.replace('\\', "\\\\").replace('"', "\\\"")
        );
        let status = std::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .status()?;
        if status.success() {
            return Ok(());
        }
        return Err(io::Error::other("failed to open RALPH window"));
    }

    #[allow(unreachable_code)]
    Err(io::Error::other(
        "RALPH popout is currently supported on macOS only",
    ))
}

fn escape_single_quotes(input: &str) -> String {
    input.replace('\'', "'\"'\"'")
}

fn shell_escape_arg(input: &str) -> String {
    if input
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/'))
    {
        input.to_string()
    } else {
        format!("'{}'", escape_single_quotes(input))
    }
}

#[must_use]
pub fn render_once(width: usize, default_model: Option<String>) -> String {
    App::with_default_model(load_bootstrap_snapshot(), default_model).render(width, false)
}

fn normalize_for_raw_terminal(frame: &str) -> String {
    let mut output = String::with_capacity(frame.len() + frame.matches('\n').count());
    let mut chars = frame.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\r' => {
                output.push('\r');
                if chars.peek() == Some(&'\n') {
                    output.push('\n');
                    let _ = chars.next();
                }
            }
            '\n' => output.push_str("\r\n"),
            _ => output.push(ch),
        }
    }

    output
}

pub fn run_app(
    no_alt_screen: bool,
    auto_approve: bool,
    default_model: Option<String>,
) -> io::Result<()> {
    let mut cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut workspace = ProjectWorkspace::for_cwd(&cwd)?;
    let global_root = workspace.root().to_path_buf();
    let mut thread_store = workspace.open_thread_store()?;
    let mut side_agent_store = workspace.open_side_agent_store()?;
    let mut prompt_history_store = workspace.open_prompt_history_store()?;
    let mut initial_thread = thread_store
        .latest_for_cwd(&cwd)
        .unwrap_or_else(|| thread_store.create_thread(&cwd));
    initial_thread.model = initial_thread.model.or(default_model.clone());
    if auto_approve {
        initial_thread.approval_mode = ApprovalMode::Auto;
    }
    let mut app = App::from_thread(load_bootstrap_snapshot(), initial_thread);
    app.set_prompt_history(prompt_history_for_app(&prompt_history_store));
    if let Some(report_path) = std::env::var_os("VORKER_START_REPORT") {
        let report_path = PathBuf::from(report_path);
        app.apply_system_notice(format!("Adversarial report: {}", report_path.display()));
        if let Ok(report) = std::fs::read_to_string(&report_path)
            && !report.trim().is_empty()
        {
            app.apply_review_output(&report);
        }
    }
    app.set_workspace_files(load_workspace_files(&cwd));
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(io::Error::other)?;
    let mut bridge =
        runtime.block_on(AcpBridge::start(cwd.clone(), None, default_model.clone()))?;
    let mut pending_permission_reply = None;
    enable_raw_mode()?;
    let mut stdout = io::stdout();

    if !no_alt_screen {
        execute!(stdout, EnterAlternateScreen, Hide)?;
    }

    run_boot_sequence(
        &mut stdout,
        &mut app,
        &mut bridge,
        &mut pending_permission_reply,
    )?;
    if !workspace.is_confirmed() {
        let confirmed = confirm_project_workspace(&mut stdout, &workspace)?;
        if !confirmed {
            let _ = runtime.block_on(bridge.shutdown());
            if !no_alt_screen {
                execute!(stdout, Show, LeaveAlternateScreen)?;
            }
            disable_raw_mode()?;
            return Ok(());
        }
        thread_store = workspace.open_thread_store()?;
        side_agent_store = workspace.open_side_agent_store()?;
        prompt_history_store = workspace.open_prompt_history_store()?;
        app.apply_system_notice(format!(
            "Project workspace ready: {}",
            format_path_for_humans(&workspace.project_dir())
        ));
    }

    let mut review_job = None;
    let mut side_agent_jobs: Vec<SideAgentJob> = Vec::new();
    if current_review_mode()
        && std::env::var("VORKER_REVIEW_AUTO")
            .ok()
            .is_some_and(|value| value == "1")
    {
        app.pending_actions.push(AppCommand::RunReview {
            focus: std::env::var("VORKER_REVIEW_FOCUS").unwrap_or_default(),
            coach: env_flag("VORKER_REVIEW_COACH"),
            apply: env_flag("VORKER_REVIEW_APPLY"),
            popout: false,
            scope: current_shell_review_scope(),
        });
    }

    loop {
        drain_bridge_events(&mut app, &mut bridge, &mut pending_permission_reply);
        app.tick();
        poll_review_job(&mut app, &mut review_job)?;
        poll_side_agent_jobs(&mut app, &mut side_agent_jobs, &mut side_agent_store)?;
        persist_dirty_thread(&mut app, &mut thread_store)?;
        let width = size()
            .map(|(columns, _)| usize::from(columns))
            .unwrap_or(120);
        let frame = normalize_for_raw_terminal(&app.render(width, true));
        execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
        write!(stdout, "{frame}")?;
        stdout.flush()?;

        if poll(std::time::Duration::from_millis(50))?
            && let Event::Key(key) = read()?
            && !app.handle_key(key)
        {
            break;
        }

        let mut should_exit = false;
        for action in app.take_actions() {
            match action {
                AppCommand::NewThread => {
                    let previous_thread = app
                        .take_archived_thread()
                        .unwrap_or_else(|| app.thread_record());
                    thread_store.upsert(previous_thread)?;
                    if let Some(reply) = pending_permission_reply.take() {
                        let _ = reply.send(None);
                    }
                    runtime.block_on(bridge.shutdown())?;
                    let mut thread = thread_store.create_thread(&cwd);
                    thread.model = default_model.clone();
                    thread.approval_mode = app.approval_mode();
                    app.load_thread(thread);
                    app.set_workspace_files(load_workspace_files(&cwd));
                    thread_store = workspace.open_thread_store()?;
                    bridge = runtime.block_on(AcpBridge::start(
                        cwd.clone(),
                        None,
                        default_model.clone(),
                    ))?;
                }
                AppCommand::ListThreads => {
                    let threads = ProjectWorkspace::list_all_threads_under(global_root.clone())?;
                    app.list_threads(&threads);
                }
                AppCommand::SwitchThread { thread_id } => {
                    thread_store.upsert(app.thread_record())?;
                    if let Some(thread) =
                        ProjectWorkspace::find_thread_under(global_root.clone(), &thread_id)?
                    {
                        let next_cwd = PathBuf::from(&thread.cwd);
                        let next_workspace =
                            ProjectWorkspace::at_root(global_root.clone(), &next_cwd)?;
                        if !next_workspace.is_confirmed()
                            && !confirm_project_workspace(&mut stdout, &next_workspace)?
                        {
                            app.apply_system_notice("Thread switch cancelled.");
                            continue;
                        }
                        if let Err(error) = std::env::set_current_dir(&next_cwd) {
                            app.apply_system_notice(format!("Error: {error}"));
                            continue;
                        }
                        cwd = next_cwd;
                        workspace = next_workspace;
                        if let Some(reply) = pending_permission_reply.take() {
                            let _ = reply.send(None);
                        }
                        runtime.block_on(bridge.shutdown())?;
                        app.load_thread(thread);
                        thread_store = workspace.open_thread_store()?;
                        side_agent_store = workspace.open_side_agent_store()?;
                        prompt_history_store = workspace.open_prompt_history_store()?;
                        app.set_prompt_history(prompt_history_for_app(&prompt_history_store));
                        app.set_workspace_files(load_workspace_files(&cwd));
                        bridge = runtime.block_on(AcpBridge::start(
                            cwd.clone(),
                            None,
                            default_model.clone(),
                        ))?;
                    } else {
                        app.apply_system_notice(format!("Unknown thread id: {thread_id}"));
                    }
                }
                AppCommand::ChangeDirectory { path } => {
                    let next_cwd = resolve_directory_change(&cwd, &path)?;
                    let next_workspace = ProjectWorkspace::at_root(global_root.clone(), &next_cwd)?;
                    if !next_workspace.is_confirmed()
                        && !confirm_project_workspace(&mut stdout, &next_workspace)?
                    {
                        app.apply_system_notice("Directory change cancelled.");
                        continue;
                    }
                    thread_store.upsert(app.thread_record())?;
                    std::env::set_current_dir(&next_cwd)?;
                    cwd = next_cwd;
                    workspace = next_workspace;
                    if let Some(reply) = pending_permission_reply.take() {
                        let _ = reply.send(None);
                    }
                    runtime.block_on(bridge.shutdown())?;
                    thread_store = workspace.open_thread_store()?;
                    side_agent_store = workspace.open_side_agent_store()?;
                    prompt_history_store = workspace.open_prompt_history_store()?;
                    let thread = thread_store.latest_for_cwd(&cwd).unwrap_or_else(|| {
                        let mut created = thread_store.create_thread(&cwd);
                        created.model = default_model.clone();
                        created.approval_mode = app.approval_mode();
                        created
                    });
                    app.load_thread(thread);
                    app.set_workspace_files(load_workspace_files(&cwd));
                    app.set_prompt_history(prompt_history_for_app(&prompt_history_store));
                    app.apply_system_notice(format!("Project directory set to {}.", cwd.display()));
                    let cwd_label = cwd.display().to_string();
                    let threads = ProjectWorkspace::list_all_threads_under(global_root.clone())?
                        .into_iter()
                        .filter(|thread| thread.cwd == cwd_label)
                        .collect::<Vec<_>>();
                    app.list_threads(&threads);
                    bridge = runtime.block_on(AcpBridge::start(
                        cwd.clone(),
                        None,
                        default_model.clone(),
                    ))?;
                }
                AppCommand::RunReview {
                    focus,
                    coach,
                    apply,
                    popout,
                    scope,
                } => {
                    if popout {
                        let review_model = current_review_model();
                        open_review_window(
                            &cwd,
                            &review_model,
                            scope.clone(),
                            coach,
                            apply,
                            &focus,
                        )?;
                        app.apply_system_notice(
                            "Adversarial review started in the review window. Use Esc there to exit review mode."
                                .to_string(),
                        );
                    } else {
                        if review_job.is_some() {
                            app.apply_system_notice("A review is already running in this shell.");
                        } else {
                            app.apply_system_notice(format!(
                                "Running adversarial review{}{}.",
                                if coach { " with coaching" } else { "" },
                                if apply { " and patch follow-up" } else { "" },
                            ));
                            review_job = Some(spawn_review_job(
                                &cwd,
                                current_review_model(),
                                scope,
                                coach,
                                apply,
                                &focus,
                            )?);
                            app.working_started_at = Some(Instant::now());
                            app.apply_tool_notice(
                                "Review job".to_string(),
                                Some("queued".to_string()),
                            );
                        }
                    }
                }
                AppCommand::RunRalph {
                    task,
                    model,
                    no_deslop,
                    xhigh,
                } => {
                    let selected_model = model.or_else(|| app.navigation.selected_model_id.clone());
                    open_ralph_window(&cwd, &task, selected_model.as_deref(), no_deslop, xhigh)?;
                    app.apply_system_notice(format!("RALPH started in a new terminal: {task}"));
                }
                AppCommand::Stop => {
                    let _ = runtime.block_on(bridge.cancel());
                    if let Some(job) = review_job.as_mut() {
                        let _ = job.child.kill();
                    }
                    let mut stopped_agents = 0usize;
                    for job in side_agent_jobs.iter_mut().filter(|job| !job.completed) {
                        let _ = job.child.kill();
                        job.completed = true;
                        let _ = side_agent_store.mark_finished(&job.id, SideAgentStatus::Stopped);
                        stopped_agents += 1;
                    }
                    review_job = None;
                    app.stop_working_timer();
                    let queued = app.queued_prompt_count();
                    app.apply_system_notice(format!(
                        "Stopped active work. {stopped_agents} side agent(s) stopped; {queued} queued prompt(s) remain."
                    ));
                }
                AppCommand::SteerPrompt { prompt_text } => {
                    runtime.block_on(bridge.prompt(prompt_text))?;
                    app.apply_system_notice("Sent steering guidance.");
                }
                AppCommand::QueuePrompt {
                    display_text,
                    prompt_text,
                } => {
                    app.queue_prompt(display_text, prompt_text);
                }
                AppCommand::SpawnAgent { prompt_text } => {
                    match spawn_side_agent(
                        &cwd,
                        &prompt_text,
                        &mut side_agent_store,
                        &workspace.side_agents_dir(),
                    ) {
                        Ok(job) => {
                            app.apply_system_notice(format!(
                                "Spawned Codex agent {}: {}",
                                job.id, job.prompt
                            ));
                            side_agent_jobs.push(job);
                        }
                        Err(error) => {
                            app.apply_system_notice(format!("Failed to spawn agent: {error}"))
                        }
                    }
                }
                AppCommand::ListAgents => {
                    let jobs = side_agent_store.list_jobs();
                    if jobs.is_empty() {
                        app.apply_system_notice("No side agents in this session.");
                    } else {
                        app.apply_system_notice("Side agents:");
                        for job in jobs {
                            app.apply_system_notice(format!(
                                "{}  {}  {}  {}",
                                job.id,
                                job.status.label(),
                                job.model,
                                job.prompt
                            ));
                        }
                    }
                }
                AppCommand::StopAgent { id } => {
                    if let Some(job) = side_agent_jobs.iter_mut().find(|job| job.id == id) {
                        if job.completed {
                            app.apply_system_notice(format!(
                                "Side agent {id} is already finished."
                            ));
                        } else {
                            let _ = job.child.kill();
                            job.completed = true;
                            let _ = side_agent_store.mark_finished(&id, SideAgentStatus::Stopped);
                            app.apply_system_notice(format!("Stopped side agent {id}."));
                        }
                    } else if side_agent_store.job(&id).is_some() {
                        side_agent_store.mark_finished(&id, SideAgentStatus::Stopped)?;
                        app.apply_system_notice(format!(
                            "Marked stored side agent {id} as stopped."
                        ));
                    } else {
                        app.apply_system_notice(format!("Unknown agent id: {id}"));
                    }
                }
                AppCommand::ShowAgentResult { id } => {
                    if let Some(job) = side_agent_jobs.iter().find(|job| job.id == id) {
                        let output = std::fs::read_to_string(&job.output_path)
                            .unwrap_or_else(|_| "No output captured yet.".to_string());
                        let events = summarize_side_agent_events(
                            &PathBuf::from(
                                side_agent_store
                                    .job(&id)
                                    .map(|job| job.events_path)
                                    .unwrap_or_default(),
                            ),
                            8,
                        )
                        .unwrap_or_default();
                        app.apply_assistant_text(&format_agent_result(&id, &events, &output));
                    } else if let Some(job) = side_agent_store.job(&id) {
                        let output = std::fs::read_to_string(&job.output_path)
                            .unwrap_or_else(|_| "No output captured yet.".to_string());
                        let events =
                            summarize_side_agent_events(&PathBuf::from(&job.events_path), 8)
                                .unwrap_or_default();
                        app.apply_assistant_text(&format_agent_result(&id, &events, &output));
                    } else {
                        app.apply_system_notice(format!("Unknown agent id: {id}"));
                    }
                }
                AppCommand::SetTheme { theme } => {
                    let normalized = match theme.trim().to_ascii_lowercase().as_str() {
                        "default" | "green" => "default",
                        "review" | "purple" => "review",
                        "opencode" | "oc" => "opencode",
                        other => {
                            app.apply_system_notice(format!("Unknown theme: {other}"));
                            continue;
                        }
                    };
                    app.shell_theme = normalized.to_string();
                    app.apply_system_notice(format!("Theme changed to {normalized}."));
                }
                AppCommand::ExportTranscript => {
                    let path = write_transcript_export(
                        &workspace.project_dir().join("exports"),
                        &app.thread_record(),
                    )?;
                    app.apply_system_notice(format!("Transcript exported to {}", path.display()));
                }
                AppCommand::ShowStatus => {
                    let jobs = side_agent_store.list_jobs();
                    let running_agents = jobs
                        .iter()
                        .filter(|job| job.status == SideAgentStatus::Running)
                        .count();
                    app.apply_system_notice(format!(
                        "Status\nmodel: {}\ncwd: {}\nworkspace: {}\napprovals: {}\nthread: {} ({})\nside agents: {} total, {} running",
                        app.navigation
                            .selected_model_id
                            .as_deref()
                            .unwrap_or("detecting..."),
                        cwd.display(),
                        workspace.project_dir().display(),
                        app.approval_mode().label(),
                        app.thread_name(),
                        format_thread_duration(app.thread_duration_seconds()),
                        jobs.len(),
                        running_agents,
                    ));
                }
                AppCommand::ListPromptHistory => {
                    let recent = prompt_history_store.recent(10);
                    if recent.is_empty() {
                        app.apply_system_notice("No prompt history yet.");
                    } else {
                        app.apply_system_notice("Prompt history:");
                        for entry in recent {
                            app.apply_system_notice(format!("- {}", entry.text));
                        }
                    }
                }
                AppCommand::SetModel { model } => {
                    runtime.block_on(bridge.set_model(model))?;
                }
                AppCommand::SubmitPrompt {
                    display_text,
                    prompt_text,
                } => {
                    prompt_history_store.append(display_text.clone())?;
                    app.record_prompt_history(display_text);
                    runtime.block_on(bridge.prompt(prompt_text))?;
                }
                AppCommand::CancelPrompt => {
                    let _ = runtime.block_on(bridge.cancel());
                }
                AppCommand::ResolvePermission { option_id } => {
                    if let Some(reply) = pending_permission_reply.take() {
                        let _ = reply.send(option_id);
                    }
                }
                AppCommand::ExitShell => {
                    should_exit = true;
                }
            }
        }
        persist_dirty_thread(&mut app, &mut thread_store)?;
        if should_exit {
            break;
        }
    }

    if let Some(reply) = pending_permission_reply.take() {
        let _ = reply.send(None);
    }
    let _ = runtime.block_on(bridge.shutdown());
    if !no_alt_screen {
        execute!(stdout, Show, LeaveAlternateScreen)?;
    }
    disable_raw_mode()?;
    Ok(())
}

fn load_bootstrap_snapshot() -> Snapshot {
    Snapshot::default()
}

fn persist_dirty_thread(app: &mut App, thread_store: &mut ThreadStore) -> io::Result<()> {
    if let Some(thread) = app.take_dirty_thread() {
        thread_store.upsert(thread)?;
    }
    Ok(())
}

fn prompt_history_for_app(store: &PromptHistoryStore) -> Vec<String> {
    let mut prompts = store
        .recent(50)
        .into_iter()
        .map(|entry| entry.text)
        .collect::<Vec<_>>();
    prompts.reverse();
    prompts
}

fn resolve_directory_change(current: &Path, requested: &str) -> io::Result<PathBuf> {
    let candidate = PathBuf::from(requested);
    let resolved = if candidate.is_absolute() {
        candidate
    } else {
        current.join(candidate)
    };
    let resolved = resolved.canonicalize()?;
    if !resolved.is_dir() {
        return Err(io::Error::other(format!(
            "{} is not a directory",
            resolved.display()
        )));
    }
    Ok(resolved)
}

fn confirm_project_workspace(
    stdout: &mut io::Stdout,
    workspace: &ProjectWorkspace,
) -> io::Result<bool> {
    let cwd = workspace.cwd().display().to_string();
    let workspace_path = format_path_for_humans(&workspace.project_dir());

    loop {
        let width = size()
            .map(|(columns, _)| usize::from(columns))
            .unwrap_or(120);
        let frame = normalize_for_raw_terminal(&render_project_confirmation(
            width,
            &cwd,
            &workspace_path,
            true,
        ));
        execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
        write!(stdout, "{frame}")?;
        stdout.flush()?;

        if let Event::Key(key) = read()? {
            match key.code {
                KeyCode::Enter => {
                    workspace.confirm()?;
                    return Ok(true);
                }
                KeyCode::Esc => return Ok(false),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(false);
                }
                _ => {}
            }
        }
    }
}

fn run_boot_sequence(
    stdout: &mut io::Stdout,
    app: &mut App,
    bridge: &mut AcpBridge,
    pending_permission_reply: &mut Option<tokio::sync::oneshot::Sender<Option<String>>>,
) -> io::Result<()> {
    let mut tick = 0usize;
    let minimum_ticks = boot_minimum_ticks();

    loop {
        drain_bridge_events(app, bridge, pending_permission_reply);

        let model = app.navigation.selected_model_id.clone();
        let ready = model.is_some();
        let detail = model
            .map(|model| format!("ready on {model}"))
            .unwrap_or_else(|| "loading model inventory".to_string());
        let status = if ready { "ready" } else { "loading" };
        let active_step = (!ready).then_some("copilot-session");
        let steps = [BootStep::new("copilot-session", "copilot", status, &detail)];
        let width = size()
            .map(|(columns, _)| usize::from(columns))
            .unwrap_or(120);
        let frame =
            normalize_for_raw_terminal(&render_boot_frame(width, tick, active_step, &steps, true));
        execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
        write!(stdout, "{frame}")?;
        stdout.flush()?;

        if ready && tick >= minimum_ticks {
            break;
        }

        std::thread::sleep(std::time::Duration::from_millis(80));
        tick = tick.saturating_add(1);
    }

    Ok(())
}

fn drain_bridge_events(
    app: &mut App,
    bridge: &mut AcpBridge,
    pending_permission_reply: &mut Option<tokio::sync::oneshot::Sender<Option<String>>>,
) {
    while let Ok(event) = bridge.evt_rx.try_recv() {
        match event {
            BridgeEvent::TextChunk { text } => app.apply_assistant_chunk(&text),
            BridgeEvent::ToolCall { title } => app.apply_tool_notice(title, None),
            BridgeEvent::ToolUpdate { title, detail } => {
                if let Some(update) = tool_update_text(title, detail) {
                    app.apply_tool_update(update);
                }
            }
            BridgeEvent::PermissionRequest {
                title,
                options,
                reply,
            } => {
                if app.approval_mode() == ApprovalMode::Auto {
                    if let Some(option) = choose_auto_permission(&options) {
                        let _ = reply.send(Some(option.option_id.clone()));
                        app.apply_system_notice(format!("Auto-approved: {}", option.name));
                    } else {
                        let _ = reply.send(None);
                        app.apply_system_notice(format!("Permission cancelled: {title}"));
                    }
                    continue;
                }
                if let Some(previous) = pending_permission_reply.take() {
                    let _ = previous.send(None);
                }
                *pending_permission_reply = Some(reply);
                app.open_permission_prompt(
                    title,
                    options
                        .into_iter()
                        .map(|option| PermissionOptionView {
                            option_id: option.option_id,
                            name: option.name,
                        })
                        .collect(),
                );
            }
            BridgeEvent::PromptDone => app.finish_prompt(),
            BridgeEvent::SessionReady {
                current_model,
                available_models,
            } => {
                if let Some(current_model) = current_model {
                    app.apply_session_ready(current_model, available_models);
                } else if !available_models.is_empty() {
                    app.set_model_choices(available_models);
                }
            }
            BridgeEvent::ModelChanged { model } => app.apply_model_changed(model),
            BridgeEvent::Error { message } => {
                app.apply_system_notice(format!("Error: {message}"));
                app.finish_prompt();
            }
        }
    }
}

fn tool_update_text(title: Option<String>, detail: Option<String>) -> Option<String> {
    detail
        .filter(|detail| !detail.trim().is_empty())
        .or_else(|| title.filter(|title| !title.trim().is_empty()))
}

fn choose_auto_permission(
    options: &[crate::bridge::PermissionOption],
) -> Option<crate::bridge::PermissionOption> {
    let mut ranked = options.to_vec();
    ranked.sort_by_key(|option| match option.kind.as_str() {
        "allow_always" => 0,
        "allow_once" => 1,
        "reject_once" => 2,
        "reject_always" => 3,
        _ => 4,
    });
    ranked.into_iter().next()
}

fn format_thread_duration(seconds: u64) -> String {
    match seconds {
        0..=59 => format!("{seconds}s"),
        60..=3599 => format!("{}m {}s", seconds / 60, seconds % 60),
        _ => format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60),
    }
}

fn format_path_for_humans(path: &Path) -> String {
    let raw = path.display().to_string();
    if let Some(home) = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|value| value.display().to_string())
        && raw.starts_with(&home)
    {
        return raw.replacen(&home, "~", 1);
    }
    raw
}

fn current_shell_theme() -> &'static str {
    match std::env::var("VORKER_THEME")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "review" => "review",
        _ => "default",
    }
}

fn current_review_mode() -> bool {
    matches!(
        std::env::var("VORKER_REVIEW_MODE")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "review"
    )
}

fn load_workspace_files(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        let entries = match std::fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let entry_path = entry.path();
            let Ok(relative) = entry_path.strip_prefix(root) else {
                continue;
            };
            if relative.as_os_str().is_empty() {
                continue;
            }

            if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                let skip = relative.iter().any(|segment| {
                    matches!(
                        segment.to_string_lossy().as_ref(),
                        ".git" | "node_modules" | "target" | ".next" | "dist"
                    )
                });
                if !skip {
                    stack.push(entry_path);
                }
                continue;
            }

            files.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }

    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::{normalize_for_raw_terminal, tool_update_text};

    #[test]
    fn normalize_for_raw_terminal_converts_lf_to_crlf() {
        assert_eq!(
            normalize_for_raw_terminal("one\ntwo\nthree"),
            "one\r\ntwo\r\nthree"
        );
    }

    #[test]
    fn normalize_for_raw_terminal_preserves_existing_crlf() {
        assert_eq!(
            normalize_for_raw_terminal("one\r\ntwo\nthree\r\n"),
            "one\r\ntwo\r\nthree\r\n"
        );
    }

    #[test]
    fn tool_update_text_prefers_detail_over_title() {
        assert_eq!(
            tool_update_text(
                Some("Read".to_string()),
                Some("Read src/app.rs".to_string())
            ),
            Some("Read src/app.rs".to_string())
        );
        assert_eq!(
            tool_update_text(Some("Read".to_string()), Some("   ".to_string())),
            Some("Read".to_string())
        );
    }
}
