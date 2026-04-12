use crossterm::cursor::MoveTo;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, poll, read};
use crossterm::execute;
use crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode, size,
};
use std::collections::{BTreeSet, VecDeque};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

use vorker_core::Snapshot;

use crate::boot::{BootStep, boot_minimum_ticks, render_boot_frame};
use crate::bottom_pane_state::{
    BottomPaneDispatch, BottomPaneState, BottomPaneSurface, BusyActionIntent, BusySurfaceAction,
    ComposerKeyAction, ComposerSubmitIntent, ListSurfaceAction, PermissionIntent,
    SkillActionIntent, SkillToggleSurfaceAction,
};
use crate::bridge::{AcpBridge, BridgeEvent};
use crate::mentions::{
    collect_buffer_mentions, extract_active_mention_query, filter_mention_items,
    prune_mention_bindings, resolve_mention_context,
};
use crate::navigation::{NavigationState, Pane};
pub use crate::popup_state::PermissionOptionView;
use crate::project_workspace::{ProjectWorkspace, render_project_confirmation};
use crate::prompt_context::{render_transcript_replay, vorker_harness_instructions};
use crate::prompt_history::PromptHistoryStore;
use crate::render::{DashboardOptions, FooterMode, RowKind, TranscriptRow, render_dashboard};
use crate::review_output::parse_review_markdown;
use crate::session_event_store::{
    SessionEventStore, apply_events_to_thread, derive_thread_events,
    render_session_event_timeline_with_mode,
};
use crate::shell_reports::{
    format_path_for_humans, format_thread_duration, render_agent_roster, render_status_summary,
    render_thread_timeline, render_thread_timeline_with_mode,
};
use crate::side_agent_store::{SideAgentStatus, SideAgentStore, summarize_side_agent_events};
use crate::skill_store::{SkillInfo, build_skill_context, discover_skills};
use crate::slash::{
    SlashCommandId, command_is_enabled_in_state, filtered_commands_for_state,
    help_summary_for_state, is_slash_mode,
};
use crate::thread_store::{ApprovalMode, StoredThread, ThreadStore};
use crate::transcript_export::write_transcript_export;

mod runtime_actions;

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
    display_name: String,
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
    ListQueuedPrompts,
    PopQueuedPrompt,
    ClearQueuedPrompts,
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
    ExportTranscript {
        mode: String,
    },
    CopyTranscriptMode {
        mode: String,
    },
    CopyStatus,
    CopyDiff,
    CopyTimeline,
    ShowDiff,
    ShowStagedDiff,
    CompactTranscript,
    ShowTimeline,
    ShowTimelineMode {
        mode: String,
        filter: Option<String>,
        limit: Option<usize>,
    },
    ShowStatus,
    ListPromptHistory,
    ListSkills,
    SetSkillEnabled {
        name: String,
        enabled: bool,
    },
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

pub struct App {
    pub snapshot: Snapshot,
    pub navigation: NavigationState,
    pub status_line: String,
    workspace_path: String,
    current_thread: StoredThread,
    rows: Vec<TranscriptRow>,
    workspace_files: Vec<String>,
    bottom_pane: BottomPaneState,
    working_started_at: Option<Instant>,
    needs_replay_context: bool,
    dirty: bool,
    archived_thread: Option<StoredThread>,
    pending_review_rows: VecDeque<TranscriptRow>,
    last_review_reveal_at: Option<Instant>,
    prompt_queue: VecDeque<(String, String)>,
    prompt_history: Vec<String>,
    prompt_history_cursor: Option<usize>,
    skills: Vec<SkillInfo>,
    enabled_skills: BTreeSet<String>,
    skill_context: String,
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

        Self {
            snapshot,
            navigation,
            status_line: "Ready.".to_string(),
            workspace_path,
            current_thread: thread.clone(),
            rows: thread.rows.clone(),
            workspace_files: Vec::new(),
            bottom_pane: {
                let mut state = BottomPaneState::default();
                state
                    .model_picker_mut()
                    .set_selected_model_id(selected_model.clone());
                state
                    .model_picker_mut()
                    .set_model_choices(selected_model.into_iter().collect());
                state
            },
            working_started_at: None,
            needs_replay_context: !thread.rows.is_empty(),
            dirty: false,
            archived_thread: None,
            pending_review_rows: VecDeque::new(),
            last_review_reveal_at: None,
            prompt_queue: VecDeque::new(),
            prompt_history: Vec::new(),
            prompt_history_cursor: None,
            skills: Vec::new(),
            enabled_skills: BTreeSet::new(),
            skill_context: String::new(),
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

    fn composer(&self) -> &crate::ComposerState {
        self.bottom_pane.composer()
    }

    fn composer_mut(&mut self) -> &mut crate::ComposerState {
        self.bottom_pane.composer_mut()
    }

    fn popup(&self) -> &crate::AppPopupState {
        self.bottom_pane.popup()
    }

    fn popup_mut(&mut self) -> &mut crate::AppPopupState {
        self.bottom_pane.popup_mut()
    }

    fn model_picker(&self) -> &crate::ModelPickerState {
        self.bottom_pane.model_picker()
    }

    fn model_picker_mut(&mut self) -> &mut crate::ModelPickerState {
        self.bottom_pane.model_picker_mut()
    }

    #[must_use]
    pub fn command_buffer(&self) -> &str {
        self.composer().buffer()
    }

    #[must_use]
    pub fn selected_model_id(&self) -> Option<&str> {
        self.model_picker().selected_model_id()
    }

    #[must_use]
    pub fn model_choices(&self) -> &[String] {
        self.model_picker().model_choices()
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
        thread.model = self.model_picker().selected_model_id().map(str::to_string);
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
        self.composer_mut().clear_buffer();
        self.model_picker_mut()
            .set_selected_model_id(thread.model.clone());
        self.model_picker_mut()
            .set_model_choices(thread.model.into_iter().collect());
        self.model_picker_mut().close();
        self.composer_mut().clear_mentions();
        self.popup_mut().close();
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
        self.model_picker_mut().set_model_choices(model_choices);
    }

    pub fn set_workspace_files(&mut self, workspace_files: Vec<String>) {
        self.workspace_files = workspace_files;
        self.sync_inline_popup();
    }

    pub fn set_prompt_history(&mut self, prompts: Vec<String>) {
        self.prompt_history = prompts;
        self.prompt_history_cursor = None;
    }

    pub fn set_skills(&mut self, skills: Vec<SkillInfo>, enabled: BTreeSet<String>) {
        self.skills = skills;
        self.enabled_skills = enabled;
        self.sync_skill_toggle_items();
    }

    pub fn set_skill_context(&mut self, context: impl Into<String>) {
        self.skill_context = context.into();
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

        self.model_picker_mut()
            .set_selected_model_id(Some(current_model));
        self.set_model_choices(available_models);
        self.dirty = true;
    }

    pub fn apply_model_changed(&mut self, model: impl Into<String>) {
        let model = model.into();
        self.model_picker_mut().ensure_choice(model.clone());
        self.model_picker_mut()
            .set_selected_model_id(Some(model.clone()));
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

    pub fn compact_transcript(&mut self) {
        if self.rows.is_empty() {
            self.apply_system_notice("Transcript is already empty.");
            return;
        }

        let summary = summarize_transcript_rows(&self.rows);
        self.rows = vec![TranscriptRow {
            kind: RowKind::System,
            text: "Conversation compacted.".to_string(),
            detail: Some(summary),
        }];
        self.needs_replay_context = true;
        self.dirty = true;
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

    pub fn queued_prompts(&self) -> Vec<String> {
        self.prompt_queue
            .iter()
            .map(|(display_text, _)| display_text.clone())
            .collect()
    }

    pub fn clear_queued_prompts(&mut self) -> usize {
        let count = self.prompt_queue.len();
        self.prompt_queue.clear();
        count
    }

    pub fn pop_queued_prompt(&mut self) -> Option<String> {
        self.prompt_queue
            .pop_front()
            .map(|(display_text, _)| display_text)
    }

    pub fn open_permission_prompt(
        &mut self,
        title: impl Into<String>,
        items: Vec<PermissionOptionView>,
    ) {
        self.popup_mut().open_permission_prompt(title, items);
    }

    pub fn render(&self, width: usize, color: bool) -> String {
        let popup = self.popup().render_state(&self.filtered_skill_items());
        let review_mode = current_review_mode();
        let busy = self.working_started_at.is_some();
        render_dashboard(
            &self.snapshot,
            DashboardOptions {
                color,
                width,
                theme_name: self.shell_theme.clone(),
                workspace_path: self.workspace_path.clone(),
                selected_model_id: self.selected_model_id().map(str::to_string),
                model_choices: self.model_picker().model_choices().to_vec(),
                model_picker_open: self.model_picker().is_open(),
                command_buffer: self.composer().buffer().to_string(),
                slash_menu_selected_index: self.composer().slash_selected_index(),
                mention_items: self
                    .bottom_pane
                    .active_surface()
                    .eq(&BottomPaneSurface::Mention)
                    .then(|| self.popup().mention_items().to_vec())
                    .unwrap_or_default(),
                mention_selected_index: self.popup().selected_index(),
                permission_title: popup.0,
                permission_items: popup.1,
                permission_selected_index: popup.2,
                context_left_label: "100% left".to_string(),
                approval_mode_label: self.current_thread.approval_mode.label().to_string(),
                thread_duration_label: format!(
                    "{} thread",
                    format_thread_duration(self.thread_duration_seconds())
                ),
                queue_label: format!("queue {}", self.prompt_queue.len()),
                activity_label: if self.working_started_at.is_some() {
                    "working".to_string()
                } else if review_mode {
                    "review".to_string()
                } else {
                    "idle".to_string()
                },
                working_seconds: self
                    .working_started_at
                    .map(|started_at| started_at.elapsed().as_secs()),
                transcript_rows: self.rows.clone(),
                tip_line: Some(if review_mode {
                    "Tip: Use /model, /coach, or /apply. Esc exits review mode.".to_string()
                } else {
                    "Tip: Use /model or /new.".to_string()
                }),
                composer_placeholder: if review_mode {
                    "Question the current implementation".to_string()
                } else {
                    "Improve documentation in @filename".to_string()
                },
                footer_mode: match (
                    review_mode,
                    busy,
                    self.composer().buffer().trim().is_empty(),
                ) {
                    (true, true, _) => FooterMode::ReviewBusy,
                    (true, false, _) => FooterMode::Review,
                    (false, true, _) => FooterMode::Busy,
                    (false, false, true) => FooterMode::Empty,
                    (false, false, false) => FooterMode::Draft,
                },
            },
        )
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return false;
        }

        match self
            .bottom_pane
            .dispatch_key(key, self.prompt_history.is_empty())
        {
            BottomPaneDispatch::Permission(key) => self.handle_permission_key(key),
            BottomPaneDispatch::SkillAction(key) => self.handle_skill_action_key(key),
            BottomPaneDispatch::SkillToggle(key) => self.handle_skill_toggle_key(key),
            BottomPaneDispatch::BusyAction(key) => self.handle_busy_action_key(key),
            BottomPaneDispatch::ModelPicker(key) => self.handle_model_picker_key(key),
            BottomPaneDispatch::Mention(key) => self.handle_mention_key(key),
            BottomPaneDispatch::Composer(action) => match action {
                ComposerKeyAction::Escape => self.handle_escape(),
                ComposerKeyAction::AutocompleteSlash => self.autocomplete_slash_command(),
                ComposerKeyAction::NavigateSlash(delta) => self.navigate_slash(delta),
                ComposerKeyAction::RecallHistory(delta) => self.recall_prompt_history(delta),
                ComposerKeyAction::Submit => self.submit_current_input(),
                ComposerKeyAction::Backspace => {
                    self.bottom_pane.apply_composer_backspace();
                    self.prompt_history_cursor = None;
                    self.sync_inline_popup();
                }
                ComposerKeyAction::Insert(ch) => {
                    self.navigation.focused_pane = Pane::Input;
                    self.bottom_pane.apply_composer_insert(ch);
                    self.prompt_history_cursor = None;
                    self.sync_inline_popup();
                }
                ComposerKeyAction::None => {}
            },
        }

        true
    }

    fn handle_permission_key(&mut self, key: KeyEvent) {
        match self
            .bottom_pane
            .handle_permission_action(self.bottom_pane.dispatch_permission_key(key))
        {
            PermissionIntent::Resolve(option_id) => self
                .pending_actions
                .push(AppCommand::ResolvePermission { option_id }),
            PermissionIntent::None => {}
        }
    }

    fn handle_skill_action_key(&mut self, key: KeyEvent) {
        match self
            .bottom_pane
            .handle_skill_action(self.bottom_pane.dispatch_skill_action_key(key))
        {
            SkillActionIntent::ListSkills => self.pending_actions.push(AppCommand::ListSkills),
            SkillActionIntent::OpenSkillToggle => self.sync_skill_toggle_items(),
            SkillActionIntent::None => {}
        }
    }

    fn handle_skill_toggle_key(&mut self, key: KeyEvent) {
        match self.bottom_pane.dispatch_skill_toggle_key(key) {
            SkillToggleSurfaceAction::Move(delta) => {
                let len = self.filtered_skill_names().len();
                self.popup_mut().cycle_selected_index(len, delta);
            }
            SkillToggleSurfaceAction::ToggleSelected => {
                if let Some(name) = self
                    .filtered_skill_names()
                    .get(self.popup().selected_index())
                    .cloned()
                {
                    let enabled = !self.enabled_skills.contains(&name);
                    self.pending_actions
                        .push(AppCommand::SetSkillEnabled { name, enabled });
                }
            }
            SkillToggleSurfaceAction::QueryBackspace => {
                let len = self.filtered_skill_names().len();
                self.bottom_pane.apply_skill_toggle_query_backspace(len);
            }
            SkillToggleSurfaceAction::QueryInsert(ch) => {
                let len = self.filtered_skill_names().len();
                self.bottom_pane.apply_skill_toggle_query_insert(ch, len);
            }
            SkillToggleSurfaceAction::Close => self.popup_mut().close(),
            SkillToggleSurfaceAction::None => {}
        }
    }

    fn handle_busy_action_key(&mut self, key: KeyEvent) {
        let action = self.bottom_pane.dispatch_busy_action_key(key);
        let display_text = self.composer().buffer().trim().to_string();
        match self
            .bottom_pane
            .handle_busy_action(action, !display_text.is_empty())
        {
            BusyActionIntent::Queue => {
                let prompt_text = self.build_prompt_text(&display_text);
                self.pending_actions.push(AppCommand::QueuePrompt {
                    display_text,
                    prompt_text,
                });
                self.bottom_pane.clear_composer();
                self.sync_inline_popup();
            }
            BusyActionIntent::Steer => {
                self.pending_actions.push(AppCommand::SteerPrompt {
                    prompt_text: format!("[STEER]\n{display_text}"),
                });
                self.bottom_pane.clear_composer();
                self.sync_inline_popup();
            }
            BusyActionIntent::None => {}
        }
        match action {
            BusySurfaceAction::EditBackspace => {
                self.bottom_pane.apply_composer_backspace();
            }
            BusySurfaceAction::EditInsert(ch) => {
                self.bottom_pane.apply_composer_insert(ch);
            }
            BusySurfaceAction::Move(_)
            | BusySurfaceAction::Submit
            | BusySurfaceAction::Close
            | BusySurfaceAction::None => {}
        }
    }

    fn filtered_skill_items(&self) -> Vec<crate::render::PopupItem> {
        let query = self
            .popup()
            .skill_toggle_query()
            .trim()
            .to_ascii_lowercase();
        self.skills
            .iter()
            .filter(|skill| {
                query.is_empty()
                    || skill.name.to_ascii_lowercase().contains(&query)
                    || skill.description.to_ascii_lowercase().contains(&query)
            })
            .map(|skill| {
                let marker = if self.enabled_skills.contains(&skill.name) {
                    "[x]"
                } else {
                    "[ ]"
                };
                crate::render::PopupItem {
                    label: format!("{marker} {}", skill.name),
                    description: Some(skill.description.clone()),
                    selectable: true,
                }
            })
            .collect()
    }

    fn filtered_skill_names(&self) -> Vec<String> {
        let query = self
            .popup()
            .skill_toggle_query()
            .trim()
            .to_ascii_lowercase();
        self.skills
            .iter()
            .filter(|skill| {
                query.is_empty()
                    || skill.name.to_ascii_lowercase().contains(&query)
                    || skill.description.to_ascii_lowercase().contains(&query)
            })
            .map(|skill| skill.name.clone())
            .collect()
    }

    fn sync_skill_toggle_items(&mut self) {
        let len = self.filtered_skill_names().len();
        self.popup_mut().clamp_selected_index(len);
    }

    fn handle_model_picker_key(&mut self, key: KeyEvent) {
        if let Some(model) = self
            .bottom_pane
            .handle_model_picker_action(self.bottom_pane.dispatch_model_picker_key(key))
        {
            self.pending_actions.push(AppCommand::SetModel { model });
        }
    }

    fn handle_mention_key(&mut self, key: KeyEvent) {
        match self.bottom_pane.dispatch_mention_key(key) {
            ListSurfaceAction::Move(delta) => {
                let len = self.popup().mention_items().len();
                self.popup_mut().cycle_selected_index(len, delta);
            }
            ListSurfaceAction::Submit => {
                if let Some(selected) = self
                    .popup()
                    .mention_items()
                    .get(self.popup().selected_index())
                    .cloned()
                    && self.bottom_pane.apply_mention_selection(&selected)
                {}
                self.sync_inline_popup();
            }
            ListSurfaceAction::Close => {
                self.popup_mut().close();
            }
            ListSurfaceAction::None => {}
        }
    }

    fn handle_escape(&mut self) {
        match self.bottom_pane.escape_action(current_review_mode()) {
            crate::bottom_pane_state::BottomPaneEscapeAction::CloseModelPicker => {
                self.model_picker_mut().close();
            }
            crate::bottom_pane_state::BottomPaneEscapeAction::ClosePopup => {
                self.popup_mut().close();
            }
            crate::bottom_pane_state::BottomPaneEscapeAction::ClearComposer => {
                self.bottom_pane.clear_composer();
            }
            crate::bottom_pane_state::BottomPaneEscapeAction::ExitReview => {
                self.pending_actions.push(AppCommand::ExitShell);
            }
            crate::bottom_pane_state::BottomPaneEscapeAction::None => {}
        }
    }

    fn autocomplete_slash_command(&mut self) {
        if !is_slash_mode(self.composer().buffer()) {
            return;
        }

        let commands = filtered_commands_for_state(
            self.composer().buffer(),
            current_review_mode(),
            self.working_started_at.is_some(),
            !self.rows.is_empty(),
        );
        if let Some(command) = commands.get(self.composer().slash_selected_index()) {
            self.bottom_pane.apply_autocomplete(command.name);
        }
    }

    fn navigate_slash(&mut self, delta: isize) {
        if !is_slash_mode(self.composer().buffer()) {
            self.recall_prompt_history(delta);
            return;
        }

        let commands = filtered_commands_for_state(
            self.composer().buffer(),
            current_review_mode(),
            self.working_started_at.is_some(),
            !self.rows.is_empty(),
        );
        if commands.is_empty() {
            return;
        }

        let next_index = cycle_index(
            self.composer().slash_selected_index(),
            commands.len(),
            delta,
        );
        self.composer_mut().set_slash_selected_index(next_index);
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
        let recalled = next
            .and_then(|index| self.prompt_history.get(index).cloned())
            .unwrap_or_default();
        self.bottom_pane.apply_history_recall(recalled);
        self.sync_inline_popup();
    }

    fn submit_current_input(&mut self) {
        match self
            .bottom_pane
            .composer_submit_intent(self.working_started_at.is_some())
        {
            ComposerSubmitIntent::None => {}
            ComposerSubmitIntent::ExecuteSlash(command) => {
                self.execute_slash_command(&command);
            }
            ComposerSubmitIntent::OpenBusyAction => {
                self.popup_mut().open_busy_action();
            }
            ComposerSubmitIntent::SubmitPrompt(display_text) => {
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
                self.bottom_pane.clear_composer();
                self.sync_inline_popup();
            }
        }
    }

    fn build_prompt_text(&mut self, display_text: &str) -> String {
        let mut sections = Vec::new();
        sections.push(vorker_harness_instructions().to_string());
        if !self.skill_context.trim().is_empty() {
            sections.push(self.skill_context.trim().to_string());
        }
        if self.needs_replay_context && !self.rows.is_empty() {
            sections.push(format!(
                "Previous thread transcript:\n{}",
                render_transcript_replay(&self.rows)
            ));
        }

        let bindings =
            collect_buffer_mentions(self.composer().buffer(), self.composer().mention_bindings());
        let context =
            resolve_mention_context(std::path::Path::new(&self.workspace_path), &bindings);
        for error in &context.errors {
            self.apply_system_notice(error.clone());
        }
        sections.extend(context.sections);

        format!(
            "{}\n\nUser request:\n{}",
            sections.join("\n\n"),
            display_text
        )
    }

    fn execute_slash_command(&mut self, buffer: &str) {
        let command = parse_exact_slash_command(buffer).or_else(|| {
            filtered_commands_for_state(
                buffer,
                current_review_mode(),
                self.working_started_at.is_some(),
                !self.rows.is_empty(),
            )
            .get(self.composer().slash_selected_index())
            .copied()
        });

        self.composer_mut().clear_buffer();
        self.popup_mut().close();
        self.composer_mut().set_slash_selected_index(0);

        let Some(command) = command else {
            self.apply_system_notice("Unknown command.");
            return;
        };

        if !command_is_enabled_in_state(
            command,
            current_review_mode(),
            self.working_started_at.is_some(),
            !self.rows.is_empty(),
        ) {
            self.apply_system_notice(format!(
                "{} is unavailable in the current state.",
                command.name
            ));
            return;
        }

        match command.id {
            SlashCommandId::Review
            | SlashCommandId::Ralph
            | SlashCommandId::Coach
            | SlashCommandId::Apply
            | SlashCommandId::ExitReview => self.execute_review_command(command.id, buffer),
            SlashCommandId::Stop
            | SlashCommandId::Steer
            | SlashCommandId::Queue
            | SlashCommandId::Agent
            | SlashCommandId::Agents
            | SlashCommandId::AgentStop
            | SlashCommandId::AgentResult => {
                self.execute_agent_workflow_command(command.id, buffer)
            }
            SlashCommandId::Theme
            | SlashCommandId::Export
            | SlashCommandId::Copy
            | SlashCommandId::Diff
            | SlashCommandId::Compact
            | SlashCommandId::Timeline
            | SlashCommandId::Status
            | SlashCommandId::History => self.execute_transcript_command(command.id, buffer),
            SlashCommandId::Skills
            | SlashCommandId::Model
            | SlashCommandId::New
            | SlashCommandId::Help
            | SlashCommandId::Permissions
            | SlashCommandId::Rename
            | SlashCommandId::List
            | SlashCommandId::Cd => self.execute_session_command(command.id, buffer),
        }
    }

    fn execute_review_command(&mut self, command_id: SlashCommandId, buffer: &str) {
        match command_id {
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
            _ => {}
        }
    }

    fn execute_agent_workflow_command(&mut self, command_id: SlashCommandId, buffer: &str) {
        match command_id {
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
                } else if display_text == "list" {
                    self.pending_actions.push(AppCommand::ListQueuedPrompts);
                } else if display_text == "pop" {
                    self.pending_actions.push(AppCommand::PopQueuedPrompt);
                } else if display_text == "clear" {
                    self.pending_actions.push(AppCommand::ClearQueuedPrompts);
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
            _ => {}
        }
    }

    fn execute_transcript_command(&mut self, command_id: SlashCommandId, buffer: &str) {
        match command_id {
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
                let mode = command_tail(buffer);
                let mode = if mode.is_empty() {
                    "auto".to_string()
                } else {
                    mode
                };
                self.pending_actions
                    .push(AppCommand::ExportTranscript { mode });
            }
            SlashCommandId::Copy => {
                let scope = command_tail(buffer);
                if scope.eq_ignore_ascii_case("diff") {
                    self.pending_actions.push(AppCommand::CopyDiff);
                } else if scope.eq_ignore_ascii_case("status") {
                    self.pending_actions.push(AppCommand::CopyStatus);
                } else if scope.eq_ignore_ascii_case("timeline") {
                    self.pending_actions.push(AppCommand::CopyTimeline);
                } else {
                    let mode = if scope.is_empty() {
                        "auto".to_string()
                    } else {
                        scope
                    };
                    self.pending_actions
                        .push(AppCommand::CopyTranscriptMode { mode });
                }
            }
            SlashCommandId::Diff => {
                let scope = command_tail(buffer);
                if scope.eq_ignore_ascii_case("staged") || scope.eq_ignore_ascii_case("cached") {
                    self.pending_actions.push(AppCommand::ShowStagedDiff);
                } else {
                    self.pending_actions.push(AppCommand::ShowDiff);
                }
            }
            SlashCommandId::Compact => {
                self.pending_actions.push(AppCommand::CompactTranscript);
            }
            SlashCommandId::Timeline => {
                let tail = command_tail(buffer);
                if tail.eq_ignore_ascii_case("recent") {
                    self.pending_actions.push(AppCommand::ShowTimelineMode {
                        mode: "recent".to_string(),
                        filter: None,
                        limit: None,
                    });
                } else if let Some(value) = tail.strip_prefix("recent ").map(str::trim) {
                    match value.parse::<usize>() {
                        Ok(limit) => self.pending_actions.push(AppCommand::ShowTimelineMode {
                            mode: "recent".to_string(),
                            filter: None,
                            limit: Some(limit),
                        }),
                        Err(_) => self.apply_system_notice("Usage: /timeline recent <count>"),
                    }
                } else if let Some(filter) = tail.strip_prefix("filter ").map(str::trim) {
                    if filter.is_empty() {
                        self.apply_system_notice("Usage: /timeline filter <kind>");
                    } else {
                        self.pending_actions.push(AppCommand::ShowTimelineMode {
                            mode: "filter".to_string(),
                            filter: Some(filter.to_string()),
                            limit: None,
                        });
                    }
                } else {
                    self.pending_actions.push(AppCommand::ShowTimeline);
                }
            }
            SlashCommandId::Status => {
                self.pending_actions.push(AppCommand::ShowStatus);
            }
            SlashCommandId::History => {
                self.pending_actions.push(AppCommand::ListPromptHistory);
            }
            _ => {}
        }
    }

    fn execute_session_command(&mut self, command_id: SlashCommandId, buffer: &str) {
        match command_id {
            SlashCommandId::Skills => {
                let mut tokens = buffer.split_whitespace().skip(1);
                match tokens.next() {
                    None => {
                        self.popup_mut().open_skill_action();
                    }
                    Some("list") => self.pending_actions.push(AppCommand::ListSkills),
                    Some("enable") => {
                        let name = tokens.collect::<Vec<_>>().join(" ");
                        if name.is_empty() {
                            self.apply_system_notice("Usage: /skills enable <skill-name>");
                        } else {
                            self.pending_actions.push(AppCommand::SetSkillEnabled {
                                name,
                                enabled: true,
                            });
                        }
                    }
                    Some("disable") => {
                        let name = tokens.collect::<Vec<_>>().join(" ");
                        if name.is_empty() {
                            self.apply_system_notice("Usage: /skills disable <skill-name>");
                        } else {
                            self.pending_actions.push(AppCommand::SetSkillEnabled {
                                name,
                                enabled: false,
                            });
                        }
                    }
                    Some("toggle") => {
                        let name = tokens.collect::<Vec<_>>().join(" ");
                        if name.is_empty() {
                            self.apply_system_notice("Usage: /skills toggle <skill-name>");
                        } else {
                            let resolved = resolve_skill_name(&self.skills, &name).unwrap_or(name);
                            let enabled = !self.enabled_skills.contains(&resolved);
                            self.pending_actions.push(AppCommand::SetSkillEnabled {
                                name: resolved,
                                enabled,
                            });
                        }
                    }
                    Some("search") => {
                        self.popup_mut().open_skill_toggle(true);
                        for ch in tokens.collect::<Vec<_>>().join(" ").chars() {
                            self.popup_mut().push_skill_toggle_char(ch);
                        }
                        self.sync_skill_toggle_items();
                    }
                    Some(_) => {
                        self.apply_system_notice(
                            "Usage: /skills [list|enable <name>|disable <name>|toggle <name>|search <query>]",
                        );
                    }
                }
            }
            SlashCommandId::Model => {
                let requested = buffer.split_whitespace().nth(1).map(str::to_string);
                if let Some(model) = requested {
                    self.pending_actions.push(AppCommand::SetModel { model });
                } else {
                    self.model_picker_mut().open();
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
                self.apply_system_notice(help_summary_for_state(
                    current_review_mode(),
                    self.working_started_at.is_some(),
                    !self.rows.is_empty(),
                ));
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
            _ => {}
        }
    }

    fn sync_inline_popup(&mut self) {
        let bindings =
            prune_mention_bindings(self.composer().buffer(), self.composer().mention_bindings());
        self.composer_mut().set_mention_bindings(bindings);

        if let Some(query) = extract_active_mention_query(self.composer().buffer()) {
            let mention_items = filter_mention_items(&query, &self.workspace_files);
            self.popup_mut().open_mention();
            self.popup_mut().set_mention_items(mention_items);
            return;
        }

        self.popup_mut().close();
    }
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
            display_name: record.display_name,
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

fn format_agent_result(id: &str, display_name: &str, events: &[String], output: &str) -> String {
    let mut sections = vec![format!("Agent {display_name} ({id}) result:")];
    if !events.is_empty() {
        sections.push("Events:".to_string());
        sections.extend(events.iter().map(|event| format!("- {event}")));
    }
    sections.push("Output:".to_string());
    sections.push(output.to_string());
    sections.join("\n")
}

fn resolve_agent_identifier(
    requested: &str,
    live_jobs: &[SideAgentJob],
    store: &SideAgentStore,
) -> Option<String> {
    if live_jobs.iter().any(|job| job.id == requested) || store.job(requested).is_some() {
        return Some(requested.to_string());
    }

    let lower = requested.to_ascii_lowercase();
    let mut matches = live_jobs
        .iter()
        .map(|job| (job.id.clone(), job.display_name.clone()))
        .chain(
            store
                .list_jobs()
                .into_iter()
                .map(|job| (job.id, job.display_name)),
        )
        .filter(|(_, name)| name.to_ascii_lowercase() == lower)
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    matches.sort();
    matches.dedup();
    (matches.len() == 1).then(|| matches.remove(0))
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
    let mut skill_store = workspace.open_skill_store()?;
    let mut session_event_store = workspace.open_session_event_store()?;
    let mut initial_thread = thread_store
        .latest_for_cwd(&cwd)
        .unwrap_or_else(|| thread_store.create_thread(&cwd));
    initial_thread = hydrate_thread_from_events(initial_thread, &session_event_store)?;
    initial_thread.model = initial_thread.model.or(default_model.clone());
    if auto_approve {
        initial_thread.approval_mode = ApprovalMode::Auto;
    }
    let mut app = App::from_thread(load_bootstrap_snapshot(), initial_thread);
    app.set_prompt_history(prompt_history_for_app(&prompt_history_store));
    refresh_skill_state(&mut app, &cwd, &skill_store)?;
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
        skill_store = workspace.open_skill_store()?;
        session_event_store = workspace.open_session_event_store()?;
        refresh_skill_state(&mut app, &cwd, &skill_store)?;
        app.apply_system_notice(format!(
            "Project workspace ready: {}",
            format_path_for_humans(&workspace.project_dir())
        ));
    }

    let mut review_job = None;
    let mut side_agent_jobs: Vec<SideAgentJob> = Vec::new();
    let mut last_frame = String::new();
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
        persist_dirty_thread(&mut app, &mut thread_store, &session_event_store)?;
        let width = size()
            .map(|(columns, _)| usize::from(columns))
            .unwrap_or(120);
        let frame = normalize_for_raw_terminal(&app.render(width, true));
        if should_redraw_frame(&last_frame, &frame) {
            execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
            write!(stdout, "{frame}")?;
            stdout.flush()?;
            last_frame = frame;
        }

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
                        thread_store = workspace.open_thread_store()?;
                        side_agent_store = workspace.open_side_agent_store()?;
                        prompt_history_store = workspace.open_prompt_history_store()?;
                        skill_store = workspace.open_skill_store()?;
                        session_event_store = workspace.open_session_event_store()?;
                        let thread = hydrate_thread_from_events(thread, &session_event_store)?;
                        app.load_thread(thread);
                        app.set_prompt_history(prompt_history_for_app(&prompt_history_store));
                        app.set_workspace_files(load_workspace_files(&cwd));
                        refresh_skill_state(&mut app, &cwd, &skill_store)?;
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
                    skill_store = workspace.open_skill_store()?;
                    session_event_store = workspace.open_session_event_store()?;
                    let thread = thread_store.latest_for_cwd(&cwd).unwrap_or_else(|| {
                        let mut created = thread_store.create_thread(&cwd);
                        created.model = default_model.clone();
                        created.approval_mode = app.approval_mode();
                        created
                    });
                    let thread = hydrate_thread_from_events(thread, &session_event_store)?;
                    app.load_thread(thread);
                    app.set_workspace_files(load_workspace_files(&cwd));
                    app.set_prompt_history(prompt_history_for_app(&prompt_history_store));
                    refresh_skill_state(&mut app, &cwd, &skill_store)?;
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
                } => self::runtime_actions::handle_review_runtime_action(
                    &mut app,
                    &cwd,
                    &mut review_job,
                    AppCommand::RunReview {
                        focus,
                        coach,
                        apply,
                        popout,
                        scope,
                    },
                )?,
                AppCommand::RunRalph {
                    task,
                    model,
                    no_deslop,
                    xhigh,
                } => self::runtime_actions::handle_review_runtime_action(
                    &mut app,
                    &cwd,
                    &mut review_job,
                    AppCommand::RunRalph {
                        task,
                        model,
                        no_deslop,
                        xhigh,
                    },
                )?,
                AppCommand::Stop => {
                    self::runtime_actions::handle_workflow_action(
                        &runtime,
                        &mut bridge,
                        &mut app,
                        &mut review_job,
                        &mut side_agent_store,
                        &mut side_agent_jobs,
                        AppCommand::Stop,
                    )?;
                }
                AppCommand::SteerPrompt { prompt_text } => {
                    self::runtime_actions::handle_workflow_action(
                        &runtime,
                        &mut bridge,
                        &mut app,
                        &mut review_job,
                        &mut side_agent_store,
                        &mut side_agent_jobs,
                        AppCommand::SteerPrompt { prompt_text },
                    )?;
                }
                AppCommand::QueuePrompt {
                    display_text,
                    prompt_text,
                } => {
                    self::runtime_actions::handle_workflow_action(
                        &runtime,
                        &mut bridge,
                        &mut app,
                        &mut review_job,
                        &mut side_agent_store,
                        &mut side_agent_jobs,
                        AppCommand::QueuePrompt {
                            display_text,
                            prompt_text,
                        },
                    )?;
                }
                AppCommand::ListQueuedPrompts => {
                    self::runtime_actions::handle_workflow_action(
                        &runtime,
                        &mut bridge,
                        &mut app,
                        &mut review_job,
                        &mut side_agent_store,
                        &mut side_agent_jobs,
                        AppCommand::ListQueuedPrompts,
                    )?;
                }
                AppCommand::PopQueuedPrompt => {
                    self::runtime_actions::handle_workflow_action(
                        &runtime,
                        &mut bridge,
                        &mut app,
                        &mut review_job,
                        &mut side_agent_store,
                        &mut side_agent_jobs,
                        AppCommand::PopQueuedPrompt,
                    )?;
                }
                AppCommand::ClearQueuedPrompts => {
                    self::runtime_actions::handle_workflow_action(
                        &runtime,
                        &mut bridge,
                        &mut app,
                        &mut review_job,
                        &mut side_agent_store,
                        &mut side_agent_jobs,
                        AppCommand::ClearQueuedPrompts,
                    )?;
                }
                AppCommand::SpawnAgent { prompt_text } => {
                    handle_side_agent_action(
                        &mut app,
                        &cwd,
                        &workspace,
                        &mut side_agent_store,
                        &mut side_agent_jobs,
                        AppCommand::SpawnAgent { prompt_text },
                    )?;
                }
                AppCommand::ListAgents => {
                    handle_side_agent_action(
                        &mut app,
                        &cwd,
                        &workspace,
                        &mut side_agent_store,
                        &mut side_agent_jobs,
                        AppCommand::ListAgents,
                    )?;
                }
                AppCommand::StopAgent { id } => {
                    handle_side_agent_action(
                        &mut app,
                        &cwd,
                        &workspace,
                        &mut side_agent_store,
                        &mut side_agent_jobs,
                        AppCommand::StopAgent { id },
                    )?;
                }
                AppCommand::ShowAgentResult { id } => {
                    handle_side_agent_action(
                        &mut app,
                        &cwd,
                        &workspace,
                        &mut side_agent_store,
                        &mut side_agent_jobs,
                        AppCommand::ShowAgentResult { id },
                    )?;
                }
                AppCommand::SetTheme { theme } => {
                    self::runtime_actions::handle_review_runtime_action(
                        &mut app,
                        &cwd,
                        &mut review_job,
                        AppCommand::SetTheme { theme },
                    )?;
                }
                AppCommand::ExportTranscript { mode } => {
                    self::runtime_actions::handle_transcript_runtime_action(
                        &mut app,
                        &cwd,
                        &workspace,
                        &session_event_store,
                        &side_agent_store,
                        AppCommand::ExportTranscript { mode },
                    )?;
                }
                AppCommand::CopyTranscriptMode { mode } => {
                    self::runtime_actions::handle_transcript_runtime_action(
                        &mut app,
                        &cwd,
                        &workspace,
                        &session_event_store,
                        &side_agent_store,
                        AppCommand::CopyTranscriptMode { mode },
                    )?;
                }
                AppCommand::CopyDiff => self::runtime_actions::handle_transcript_runtime_action(
                    &mut app,
                    &cwd,
                    &workspace,
                    &session_event_store,
                    &side_agent_store,
                    AppCommand::CopyDiff,
                )?,
                AppCommand::CopyStatus => {
                    self::runtime_actions::handle_transcript_runtime_action(
                        &mut app,
                        &cwd,
                        &workspace,
                        &session_event_store,
                        &side_agent_store,
                        AppCommand::CopyStatus,
                    )?;
                }
                AppCommand::CopyTimeline => {
                    self::runtime_actions::handle_transcript_runtime_action(
                        &mut app,
                        &cwd,
                        &workspace,
                        &session_event_store,
                        &side_agent_store,
                        AppCommand::CopyTimeline,
                    )?
                }
                AppCommand::ShowDiff => self::runtime_actions::handle_transcript_runtime_action(
                    &mut app,
                    &cwd,
                    &workspace,
                    &session_event_store,
                    &side_agent_store,
                    AppCommand::ShowDiff,
                )?,
                AppCommand::ShowStagedDiff => {
                    self::runtime_actions::handle_transcript_runtime_action(
                        &mut app,
                        &cwd,
                        &workspace,
                        &session_event_store,
                        &side_agent_store,
                        AppCommand::ShowStagedDiff,
                    )?
                }
                AppCommand::CompactTranscript => {
                    self::runtime_actions::handle_transcript_runtime_action(
                        &mut app,
                        &cwd,
                        &workspace,
                        &session_event_store,
                        &side_agent_store,
                        AppCommand::CompactTranscript,
                    )?
                }
                AppCommand::ShowTimeline => {
                    self::runtime_actions::handle_transcript_runtime_action(
                        &mut app,
                        &cwd,
                        &workspace,
                        &session_event_store,
                        &side_agent_store,
                        AppCommand::ShowTimeline,
                    )?
                }
                AppCommand::ShowTimelineMode {
                    mode,
                    filter,
                    limit,
                } => self::runtime_actions::handle_transcript_runtime_action(
                    &mut app,
                    &cwd,
                    &workspace,
                    &session_event_store,
                    &side_agent_store,
                    AppCommand::ShowTimelineMode {
                        mode,
                        filter,
                        limit,
                    },
                )?,
                AppCommand::ShowStatus => self::runtime_actions::handle_transcript_runtime_action(
                    &mut app,
                    &cwd,
                    &workspace,
                    &session_event_store,
                    &side_agent_store,
                    AppCommand::ShowStatus,
                )?,
                AppCommand::ListPromptHistory => {
                    should_exit = self::runtime_actions::handle_local_session_action(
                        &runtime,
                        &mut bridge,
                        &mut app,
                        &mut prompt_history_store,
                        &mut skill_store,
                        &cwd,
                        &mut pending_permission_reply,
                        AppCommand::ListPromptHistory,
                    )? || should_exit;
                }
                AppCommand::ListSkills => {
                    should_exit = self::runtime_actions::handle_local_session_action(
                        &runtime,
                        &mut bridge,
                        &mut app,
                        &mut prompt_history_store,
                        &mut skill_store,
                        &cwd,
                        &mut pending_permission_reply,
                        AppCommand::ListSkills,
                    )? || should_exit;
                }
                AppCommand::SetSkillEnabled { name, enabled } => {
                    should_exit = self::runtime_actions::handle_local_session_action(
                        &runtime,
                        &mut bridge,
                        &mut app,
                        &mut prompt_history_store,
                        &mut skill_store,
                        &cwd,
                        &mut pending_permission_reply,
                        AppCommand::SetSkillEnabled { name, enabled },
                    )? || should_exit;
                }
                AppCommand::SetModel { model } => {
                    should_exit = self::runtime_actions::handle_local_session_action(
                        &runtime,
                        &mut bridge,
                        &mut app,
                        &mut prompt_history_store,
                        &mut skill_store,
                        &cwd,
                        &mut pending_permission_reply,
                        AppCommand::SetModel { model },
                    )? || should_exit;
                }
                AppCommand::SubmitPrompt {
                    display_text,
                    prompt_text,
                } => {
                    should_exit = self::runtime_actions::handle_local_session_action(
                        &runtime,
                        &mut bridge,
                        &mut app,
                        &mut prompt_history_store,
                        &mut skill_store,
                        &cwd,
                        &mut pending_permission_reply,
                        AppCommand::SubmitPrompt {
                            display_text,
                            prompt_text,
                        },
                    )? || should_exit;
                }
                AppCommand::CancelPrompt => {
                    should_exit = self::runtime_actions::handle_local_session_action(
                        &runtime,
                        &mut bridge,
                        &mut app,
                        &mut prompt_history_store,
                        &mut skill_store,
                        &cwd,
                        &mut pending_permission_reply,
                        AppCommand::CancelPrompt,
                    )? || should_exit;
                }
                AppCommand::ResolvePermission { option_id } => {
                    should_exit = self::runtime_actions::handle_local_session_action(
                        &runtime,
                        &mut bridge,
                        &mut app,
                        &mut prompt_history_store,
                        &mut skill_store,
                        &cwd,
                        &mut pending_permission_reply,
                        AppCommand::ResolvePermission { option_id },
                    )? || should_exit;
                }
                AppCommand::ExitShell => {
                    should_exit = self::runtime_actions::handle_local_session_action(
                        &runtime,
                        &mut bridge,
                        &mut app,
                        &mut prompt_history_store,
                        &mut skill_store,
                        &cwd,
                        &mut pending_permission_reply,
                        AppCommand::ExitShell,
                    )? || should_exit;
                }
            }
        }
        persist_dirty_thread(&mut app, &mut thread_store, &session_event_store)?;
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

fn handle_side_agent_action(
    app: &mut App,
    cwd: &Path,
    workspace: &ProjectWorkspace,
    side_agent_store: &mut SideAgentStore,
    side_agent_jobs: &mut Vec<SideAgentJob>,
    action: AppCommand,
) -> io::Result<()> {
    match action {
        AppCommand::SpawnAgent { prompt_text } => {
            match spawn_side_agent(
                cwd,
                &prompt_text,
                side_agent_store,
                &workspace.side_agents_dir(),
            ) {
                Ok(job) => {
                    app.apply_system_notice(format!(
                        "Spawned Codex agent {} ({}).",
                        job.display_name, job.id
                    ));
                    side_agent_jobs.push(job);
                }
                Err(error) => app.apply_system_notice(format!("Failed to spawn agent: {error}")),
            }
        }
        AppCommand::ListAgents => {
            let jobs = side_agent_store.list_jobs();
            if jobs.is_empty() {
                app.apply_system_notice("No side agents in this session.");
            } else {
                app.apply_assistant_text(&render_agent_roster(&jobs));
            }
        }
        AppCommand::StopAgent { id } => {
            let resolved = resolve_agent_identifier(&id, side_agent_jobs, side_agent_store);
            if let Some(job) = side_agent_jobs
                .iter_mut()
                .find(|job| Some(job.id.as_str()) == resolved.as_deref())
            {
                if job.completed {
                    app.apply_system_notice(format!(
                        "Side agent {} ({}) is already finished.",
                        job.display_name, job.id
                    ));
                } else {
                    let _ = job.child.kill();
                    job.completed = true;
                    let _ = side_agent_store.mark_finished(&job.id, SideAgentStatus::Stopped);
                    app.apply_system_notice(format!(
                        "Stopped side agent {} ({}).",
                        job.display_name, job.id
                    ));
                }
            } else if let Some(agent_id) = resolved
                && let Some(job) = side_agent_store.job(&agent_id)
            {
                side_agent_store.mark_finished(&agent_id, SideAgentStatus::Stopped)?;
                app.apply_system_notice(format!(
                    "Marked stored side agent {} ({}) as stopped.",
                    job.display_name, agent_id
                ));
            } else {
                app.apply_system_notice(format!("Unknown agent id: {id}"));
            }
        }
        AppCommand::ShowAgentResult { id } => {
            let resolved = resolve_agent_identifier(&id, side_agent_jobs, side_agent_store);
            if let Some(job) = side_agent_jobs
                .iter()
                .find(|job| Some(job.id.as_str()) == resolved.as_deref())
            {
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
                app.apply_assistant_text(&format_agent_result(
                    &job.id,
                    &job.display_name,
                    &events,
                    &output,
                ));
            } else if let Some(agent_id) = resolved
                && let Some(job) = side_agent_store.job(&agent_id)
            {
                let output = std::fs::read_to_string(&job.output_path)
                    .unwrap_or_else(|_| "No output captured yet.".to_string());
                let events = summarize_side_agent_events(&PathBuf::from(&job.events_path), 8)
                    .unwrap_or_default();
                app.apply_assistant_text(&format_agent_result(
                    &agent_id,
                    &job.display_name,
                    &events,
                    &output,
                ));
            } else {
                app.apply_system_notice(format!("Unknown agent id: {id}"));
            }
        }
        _ => {}
    }

    Ok(())
}

fn persist_dirty_thread(
    app: &mut App,
    thread_store: &mut ThreadStore,
    session_event_store: &SessionEventStore,
) -> io::Result<()> {
    if let Some(thread) = app.take_dirty_thread() {
        let previous = thread_store.thread(&thread.id);
        let events = derive_thread_events(previous.as_ref(), &thread);
        thread_store.upsert(thread.clone())?;
        session_event_store.append(&thread.id, &events)?;
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

fn should_redraw_frame(previous: &str, next: &str) -> bool {
    previous != next
}

fn refresh_skill_state(app: &mut App, cwd: &Path, store: &crate::SkillStore) -> io::Result<()> {
    let skills = discover_skills(&skill_roots_for(cwd))?;
    let enabled = store.enabled();
    let context = build_skill_context(&skills, &enabled)?;
    app.set_skills(skills, enabled);
    app.set_skill_context(context);
    Ok(())
}

fn hydrate_thread_from_events(
    thread: StoredThread,
    session_event_store: &SessionEventStore,
) -> io::Result<StoredThread> {
    let events = session_event_store.events(&thread.id)?;
    if events.is_empty() {
        Ok(thread)
    } else {
        Ok(apply_events_to_thread(&thread, &events))
    }
}

fn apply_skill_listing(app: &mut App) {
    if app.skills.is_empty() {
        app.apply_system_notice("No skills found.");
        return;
    }

    app.apply_system_notice("Skills:");
    for skill in app.skills.clone() {
        let marker = if app.enabled_skills.contains(&skill.name) {
            "[x]"
        } else {
            "[ ]"
        };
        app.apply_system_notice(format!(
            "{marker} {}  [Skill] {}",
            skill.name, skill.description
        ));
    }
    app.apply_system_notice(
        "Use /skills enable <name>, /skills disable <name>, or /skills toggle <name>.",
    );
}

fn resolve_skill_name(skills: &[SkillInfo], requested: &str) -> Option<String> {
    let requested = requested.trim();
    if requested.is_empty() {
        return None;
    }

    skills
        .iter()
        .find(|skill| skill.name == requested)
        .or_else(|| {
            let lower = requested.to_ascii_lowercase();
            skills
                .iter()
                .find(|skill| skill.name.to_ascii_lowercase() == lower)
        })
        .or_else(|| {
            let lower = requested.to_ascii_lowercase();
            let mut matches = skills
                .iter()
                .filter(|skill| skill.name.to_ascii_lowercase().contains(&lower));
            let first = matches.next()?;
            matches.next().is_none().then_some(first)
        })
        .map(|skill| skill.name.clone())
}

fn skill_roots_for(cwd: &Path) -> Vec<PathBuf> {
    let mut roots = vec![
        cwd.join(".codex").join("skills"),
        cwd.join(".agents").join("skills"),
        cwd.join(".github").join("skills"),
    ];

    if let Ok(codex_home) = std::env::var("CODEX_HOME") {
        roots.push(PathBuf::from(codex_home).join("skills"));
    } else if let Some(home) = home_dir() {
        roots.push(home.join(".codex").join("skills"));
        roots.push(home.join(".codex").join("superpowers").join("skills"));
        roots.push(home.join(".agents").join("skills"));
    }

    roots
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
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

        let model = app.selected_model_id().map(str::to_string);
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

fn copy_to_clipboard(text: &str) -> io::Result<()> {
    let mut child = std::process::Command::new("pbcopy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write as _;
        stdin.write_all(text.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(io::Error::other(if stderr.is_empty() {
            "pbcopy failed".to_string()
        } else {
            format!("pbcopy failed: {stderr}")
        }))
    }
}

fn render_working_tree_diff(cwd: &Path, max_lines: usize) -> io::Result<String> {
    let status = run_git(cwd, ["status", "--short", "--untracked-files=all"])?;
    let diff = run_git(cwd, ["diff", "--unified=3"])?;
    let staged = run_git(cwd, ["diff", "--cached", "--unified=3"])?;

    let mut sections = Vec::new();
    if !status.trim().is_empty() {
        sections.push(format!("## Git status\n{}", status.trim_end()));
    }
    if !staged.trim().is_empty() {
        sections.push(format!(
            "## Staged diff\n{}",
            truncate_lines(&staged, max_lines)
        ));
    }
    if !diff.trim().is_empty() {
        sections.push(format!(
            "## Unstaged diff\n{}",
            truncate_lines(&diff, max_lines)
        ));
    }

    if sections.is_empty() {
        Ok("Working tree is clean.".to_string())
    } else {
        Ok(sections.join("\n\n"))
    }
}

fn render_staged_diff(cwd: &Path, max_lines: usize) -> io::Result<String> {
    let status = run_git(cwd, ["status", "--short", "--untracked-files=all"])?;
    let staged = run_git(cwd, ["diff", "--cached", "--unified=3"])?;

    let mut sections = Vec::new();
    if !status.trim().is_empty() {
        sections.push(format!("## Git status\n{}", status.trim_end()));
    }
    if !staged.trim().is_empty() {
        sections.push(format!(
            "## Staged diff\n{}",
            truncate_lines(&staged, max_lines)
        ));
    }

    if sections.is_empty() {
        Ok("No staged changes.".to_string())
    } else {
        Ok(sections.join("\n\n"))
    }
}

fn load_timeline_text(
    session_event_store: &SessionEventStore,
    thread: &StoredThread,
) -> io::Result<String> {
    let events = session_event_store.events(&thread.id)?;
    if events.is_empty() {
        Ok(render_thread_timeline(thread))
    } else {
        Ok(render_session_event_timeline_with_mode(
            &thread.name,
            &events,
            "full",
            None,
            None,
        ))
    }
}

fn load_timeline_text_with_mode(
    session_event_store: &SessionEventStore,
    thread: &StoredThread,
    mode: &str,
    filter: Option<&str>,
    limit: Option<usize>,
) -> io::Result<String> {
    let events = session_event_store.events(&thread.id)?;
    if events.is_empty() {
        Ok(render_thread_timeline_with_mode(
            thread, mode, filter, limit,
        ))
    } else {
        Ok(render_session_event_timeline_with_mode(
            &thread.name,
            &events,
            mode,
            filter,
            limit,
        ))
    }
}

fn summarize_transcript_rows(rows: &[TranscriptRow]) -> String {
    let mut lines = vec![format!("Compacted {} row(s).", rows.len())];
    for (index, row) in rows.iter().take(8).enumerate() {
        let kind = match row.kind {
            RowKind::System => "system",
            RowKind::User => "user",
            RowKind::Assistant => "assistant",
            RowKind::Tool => "tool",
        };
        let summary = row
            .text
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .chars()
            .take(100)
            .collect::<String>();
        lines.push(format!("{}. [{}] {}", index + 1, kind, summary));
    }
    if rows.len() > 8 {
        lines.push(format!("… {} more row(s) omitted", rows.len() - 8));
    }
    lines.join("\n")
}

fn truncate_lines(text: &str, max_lines: usize) -> String {
    let lines = text.lines().collect::<Vec<_>>();
    if lines.len() <= max_lines {
        text.trim_end().to_string()
    } else {
        let mut truncated = lines[..max_lines].join("\n");
        truncated.push_str("\n\n[diff truncated]");
        truncated
    }
}

fn run_git<I, S>(cwd: &Path, args: I) -> io::Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = std::process::Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(io::Error::other(if stderr.is_empty() {
            "git command failed".to_string()
        } else {
            stderr
        }))
    }
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
    use super::{
        SideAgentJob, copy_to_clipboard, format_agent_result, normalize_for_raw_terminal,
        render_agent_roster, render_staged_diff, render_status_summary, render_thread_timeline,
        render_thread_timeline_with_mode, render_working_tree_diff, resolve_agent_identifier,
        should_redraw_frame, summarize_transcript_rows, tool_update_text, truncate_lines,
    };
    use crate::{RowKind, SideAgentStatus, SideAgentStore, StoredThread, TranscriptRow};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

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

    #[test]
    fn copy_to_clipboard_writes_to_pbcopy() {
        copy_to_clipboard("vorker clipboard smoke test").expect("pbcopy");
    }

    fn unique_temp_dir(name: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("vorker-app-{name}-{suffix}"))
    }

    #[test]
    fn render_working_tree_diff_reports_clean_repo() {
        let root = unique_temp_dir("clean-repo");
        fs::create_dir_all(&root).expect("root");
        std::process::Command::new("git")
            .current_dir(&root)
            .args(["init"])
            .output()
            .expect("git init");

        let output = render_working_tree_diff(&root, 20).expect("diff");
        assert_eq!(output, "Working tree is clean.");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn render_working_tree_diff_errors_outside_git_repo() {
        let root = unique_temp_dir("not-a-repo");
        fs::create_dir_all(&root).expect("root");

        let error = render_working_tree_diff(&root, 20).expect_err("expected git failure");
        assert!(
            error.to_string().contains("not a git repository")
                || error.to_string().contains("git command failed")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn render_staged_diff_reports_no_staged_changes_for_clean_repo() {
        let root = unique_temp_dir("clean-staged");
        fs::create_dir_all(&root).expect("root");
        std::process::Command::new("git")
            .current_dir(&root)
            .args(["init"])
            .output()
            .expect("git init");

        let output = render_staged_diff(&root, 20).expect("diff");
        assert_eq!(output, "No staged changes.");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn truncate_lines_marks_truncated_output() {
        let text = "a\nb\nc\nd";
        assert_eq!(truncate_lines(text, 2), "a\nb\n\n[diff truncated]");
    }

    #[test]
    fn render_thread_timeline_summarizes_rows() {
        let mut thread = StoredThread::ephemeral("/workspace/pod");
        thread.name = "Hyperloop controls".to_string();
        thread.rows = vec![
            TranscriptRow {
                kind: RowKind::User,
                text: "build the controller".to_string(),
                detail: None,
            },
            TranscriptRow {
                kind: RowKind::Tool,
                text: "Explored".to_string(),
                detail: Some("Read src/controller.rs".to_string()),
            },
        ];

        let timeline = render_thread_timeline(&thread);
        assert!(timeline.contains("## Timeline"));
        assert!(timeline.contains("1. [user] build the controller"));
        assert!(timeline.contains("2. [tool] Explored"));
    }

    #[test]
    fn render_thread_timeline_recent_respects_limit() {
        let mut thread = StoredThread::ephemeral("/workspace/pod");
        thread.name = "Hyperloop controls".to_string();
        for index in 0..12 {
            thread.rows.push(TranscriptRow {
                kind: RowKind::User,
                text: format!("row {index}"),
                detail: None,
            });
        }

        let timeline = render_thread_timeline_with_mode(&thread, "recent", None, Some(3));

        assert!(timeline.contains("- mode: recent"));
        assert!(timeline.contains("3. [user] row 11"));
        assert!(!timeline.contains("4."));
    }

    #[test]
    fn render_thread_timeline_filter_limits_to_matching_kind() {
        let mut thread = StoredThread::ephemeral("/workspace/pod");
        thread.name = "Hyperloop controls".to_string();
        thread.rows = vec![
            TranscriptRow {
                kind: RowKind::User,
                text: "user row".to_string(),
                detail: None,
            },
            TranscriptRow {
                kind: RowKind::Assistant,
                text: "assistant row".to_string(),
                detail: None,
            },
        ];

        let timeline = render_thread_timeline_with_mode(&thread, "filter", Some("assistant"), None);

        assert!(timeline.contains("- mode: filter"));
        assert!(timeline.contains("[assistant] assistant row"));
        assert!(!timeline.contains("[user] user row"));
    }

    #[test]
    fn summarize_transcript_rows_compacts_and_limits_output() {
        let rows = vec![
            TranscriptRow {
                kind: RowKind::User,
                text: "first".to_string(),
                detail: None,
            },
            TranscriptRow {
                kind: RowKind::Assistant,
                text: "second".to_string(),
                detail: None,
            },
        ];

        let summary = summarize_transcript_rows(&rows);
        assert!(summary.contains("Compacted 2 row(s)."));
        assert!(summary.contains("1. [user] first"));
        assert!(summary.contains("2. [assistant] second"));
    }

    #[test]
    fn render_status_summary_includes_queue_and_transcript_counts() {
        let output = render_status_summary(
            "claude-sonnet-4.5",
            "/workspace",
            "/workspace/.vorker",
            "manual approvals",
            "Thread 1",
            "4s",
            12,
            5,
            3,
            2,
            1,
            &["Auth Inspector".to_string()],
        );

        assert!(output.contains("transcript rows: 12"));
        assert!(output.contains("events: 5"));
        assert!(output.contains("queued prompts: 3"));
        assert!(output.contains("side agents: 2 total, 1 running"));
        assert!(output.contains("running agents: Auth Inspector"));
    }

    #[test]
    fn render_agent_roster_prefers_display_names_and_status() {
        let job = crate::StoredSideAgentJob {
            id: "agent-1".to_string(),
            display_name: "Auth Inspector".to_string(),
            prompt: "inspect auth".to_string(),
            cwd: "/workspace".to_string(),
            model: "gpt-5.4".to_string(),
            status: SideAgentStatus::Running,
            created_at_epoch_seconds: 1,
            finished_at_epoch_seconds: None,
            output_path: "/tmp/out".to_string(),
            stderr_path: "/tmp/err".to_string(),
            events_path: "/tmp/events".to_string(),
        };

        let output = render_agent_roster(&[job]);
        assert!(output.contains("## Side agents"));
        assert!(output.contains("Auth Inspector [running]"));
        assert!(output.contains("id: agent-1"));
        assert!(output.contains("prompt: inspect auth"));
    }

    #[test]
    fn should_redraw_frame_only_when_frame_changes() {
        assert!(!should_redraw_frame("same", "same"));
        assert!(should_redraw_frame("before", "after"));
    }

    #[test]
    fn format_agent_result_uses_display_name() {
        let result = format_agent_result(
            "agent-1",
            "Auth Inspector",
            &["turn completed".to_string()],
            "Looks good.",
        );

        assert!(result.contains("Agent Auth Inspector (agent-1) result:"));
        assert!(result.contains("- turn completed"));
    }

    #[test]
    fn resolve_agent_identifier_accepts_exact_display_name() {
        let live_jobs = vec![SideAgentJob {
            id: "agent-1".to_string(),
            display_name: "Auth Inspector".to_string(),
            child: std::process::Command::new("true")
                .spawn()
                .expect("spawn true"),
            output_path: std::path::PathBuf::from("/tmp/out"),
            stderr_path: std::path::PathBuf::from("/tmp/err"),
            completed: true,
        }];
        let store = SideAgentStore::open_at(unique_temp_dir("resolve-id").join("agents.json"))
            .expect("store");

        let resolved = resolve_agent_identifier("Auth Inspector", &live_jobs, &store);
        assert_eq!(resolved.as_deref(), Some("agent-1"));
    }
}
