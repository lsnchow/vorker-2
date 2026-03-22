use crossterm::cursor::MoveTo;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, read};
use crossterm::execute;
use crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode, size,
};
use std::io::{self, Write};

use vorker_agent::{ProviderId, ProviderManager};
use vorker_core::{
    EventLog, Snapshot, create_supervisor_event, now_iso, restore_durable_supervisor_state,
};

use crate::navigation::{
    NavKey, NavigationState, Pane, apply_navigation_key, reconcile_navigation_state,
};
use crate::render::{DashboardOptions, InputMode, render_dashboard};
use crate::slash::{SlashCommandId, filtered_commands, is_slash_mode};

const AGENT_ROLES: [&str; 3] = ["worker", "planner", "reviewer"];
const SWARM_STRATEGIES: [&str; 3] = ["parallel", "review-first", "repair-first"];

#[derive(Clone, Debug, PartialEq, Eq)]
enum OverlayState {
    CreateAgent {
        role_index: usize,
    },
    #[allow(dead_code)]
    SwarmLaunch {
        goal: String,
        strategy_index: usize,
    },
}

pub struct App {
    pub snapshot: Snapshot,
    pub navigation: NavigationState,
    pub status_line: String,
    pub input_mode: InputMode,
    provider_id: ProviderId,
    provider_choices: Vec<ProviderId>,
    provider_picker_open: bool,
    provider_picker_index: usize,
    workspace_path: String,
    overlay: Option<OverlayState>,
    slash_menu_selected_index: usize,
    next_agent_index: usize,
    next_run_index: usize,
    next_task_index: usize,
}

impl App {
    #[must_use]
    pub fn new(snapshot: Snapshot) -> Self {
        let navigation = reconcile_navigation_state(&snapshot, NavigationState::default());
        let provider_id = snapshot
            .sessions
            .first()
            .and_then(|session| session.provider.as_deref())
            .and_then(|provider| provider.parse::<ProviderId>().ok())
            .unwrap_or_else(ProviderManager::default_provider);
        let provider_choices = ProviderManager::available_providers().to_vec();
        let workspace_path = std::env::current_dir()
            .ok()
            .map(|path| path.display().to_string())
            .or_else(|| snapshot.sessions.first().map(|session| session.cwd.clone()))
            .unwrap_or_else(|| ".".to_string());
        let next_task_index = snapshot
            .runs
            .iter()
            .map(|run| run.tasks.len())
            .sum::<usize>()
            + 1;

        Self {
            snapshot,
            navigation,
            status_line: "Ready for prompts.".to_string(),
            input_mode: InputMode::Prompt,
            provider_id,
            provider_choices,
            provider_picker_open: false,
            provider_picker_index: 0,
            workspace_path,
            overlay: None,
            slash_menu_selected_index: 0,
            next_agent_index: 1,
            next_run_index: 1,
            next_task_index,
        }
    }

    pub fn render(&self, width: usize, color: bool) -> String {
        render_dashboard(
            &self.snapshot,
            DashboardOptions {
                color,
                width,
                provider_id: self.provider_id.to_string(),
                provider_choices: self
                    .provider_choices
                    .iter()
                    .map(|provider| provider.to_string())
                    .collect(),
                provider_picker_selected_id: self
                    .provider_choices
                    .get(self.provider_picker_index)
                    .map(|provider| provider.to_string()),
                workspace_path: self.workspace_path.clone(),
                status_line: self.status_line.clone(),
                input_mode: self.input_mode.clone(),
                focused_pane: self.navigation.focused_pane,
                selected_action_id: self.navigation.selected_action_id,
                selected_model_id: self.navigation.selected_model_id.clone(),
                model_choices: self.navigation.model_choices.clone(),
                model_picker_open: self.navigation.model_picker_open,
                provider_picker_open: self.provider_picker_open,
                active_session_id: self.navigation.active_session_id.clone(),
                active_run_id: self.navigation.active_run_id.clone(),
                selected_task_id: self.navigation.selected_task_id.clone(),
                command_buffer: self.navigation.command_buffer.clone(),
                slash_menu_selected_index: self.slash_menu_selected_index,
                create_agent_overlay_open: matches!(
                    self.overlay,
                    Some(OverlayState::CreateAgent { .. })
                ),
                create_agent_role: match &self.overlay {
                    Some(OverlayState::CreateAgent { role_index }) => {
                        Some(AGENT_ROLES[*role_index].to_string())
                    }
                    _ => None,
                },
                swarm_overlay_open: matches!(self.overlay, Some(OverlayState::SwarmLaunch { .. })),
                swarm_goal: match &self.overlay {
                    Some(OverlayState::SwarmLaunch { goal, .. }) => goal.clone(),
                    _ => String::new(),
                },
                swarm_strategy: match &self.overlay {
                    Some(OverlayState::SwarmLaunch { strategy_index, .. }) => {
                        Some(SWARM_STRATEGIES[*strategy_index].to_string())
                    }
                    _ => None,
                },
            },
        )
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return false;
        }

        if self.navigation.model_picker_open {
            self.handle_model_picker_key(key);
            self.navigation = reconcile_navigation_state(&self.snapshot, self.navigation.clone());
            return true;
        }

        if self.provider_picker_open {
            self.handle_provider_picker_key(key);
            self.navigation = reconcile_navigation_state(&self.snapshot, self.navigation.clone());
            return true;
        }

        if self.overlay.is_some() {
            self.handle_overlay_key(key);
            self.navigation = reconcile_navigation_state(&self.snapshot, self.navigation.clone());
            return true;
        }

        match key.code {
            KeyCode::Left => {
                self.navigation =
                    apply_navigation_key(self.navigation.clone(), &self.snapshot, NavKey::Left);
            }
            KeyCode::Right => {
                self.navigation =
                    apply_navigation_key(self.navigation.clone(), &self.snapshot, NavKey::Right);
            }
            KeyCode::Up => {
                if !self.navigate_slash_menu(-1) {
                    self.navigation =
                        apply_navigation_key(self.navigation.clone(), &self.snapshot, NavKey::Up);
                }
            }
            KeyCode::Down => {
                if !self.navigate_slash_menu(1) {
                    self.navigation =
                        apply_navigation_key(self.navigation.clone(), &self.snapshot, NavKey::Down);
                }
            }
            KeyCode::Tab => {
                let nav_key = if key.modifiers.contains(KeyModifiers::SHIFT) {
                    NavKey::ShiftTab
                } else {
                    NavKey::Tab
                };
                self.navigation =
                    apply_navigation_key(self.navigation.clone(), &self.snapshot, nav_key);
            }
            KeyCode::Esc => {
                self.overlay = None;
                self.provider_picker_open = false;
                self.input_mode = InputMode::Prompt;
                if self.navigation.command_buffer.is_empty() {
                    self.status_line = "Ready for prompts.".to_string();
                } else {
                    self.navigation.command_buffer.clear();
                    self.slash_menu_selected_index = 0;
                    self.status_line = "Input cleared.".to_string();
                }
            }
            KeyCode::Enter => self.activate_current_selection(),
            KeyCode::Backspace => {
                self.navigation.focused_pane = Pane::Input;
                let _ = self.navigation.command_buffer.pop();
                self.sync_slash_index();
            }
            KeyCode::Char(ch) => {
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT)
                {
                    self.navigation.focused_pane = Pane::Input;
                    self.navigation.command_buffer.push(ch);
                    self.sync_slash_index();
                }
            }
            _ => {}
        }

        self.navigation = reconcile_navigation_state(&self.snapshot, self.navigation.clone());
        true
    }

    fn handle_model_picker_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Left => {
                self.navigation =
                    apply_navigation_key(self.navigation.clone(), &self.snapshot, NavKey::Left);
            }
            KeyCode::Right => {
                self.navigation =
                    apply_navigation_key(self.navigation.clone(), &self.snapshot, NavKey::Right);
            }
            KeyCode::Up => {
                self.navigation =
                    apply_navigation_key(self.navigation.clone(), &self.snapshot, NavKey::Up);
            }
            KeyCode::Down => {
                self.navigation =
                    apply_navigation_key(self.navigation.clone(), &self.snapshot, NavKey::Down);
            }
            KeyCode::Tab => {
                let nav_key = if key.modifiers.contains(KeyModifiers::SHIFT) {
                    NavKey::ShiftTab
                } else {
                    NavKey::Tab
                };
                self.navigation =
                    apply_navigation_key(self.navigation.clone(), &self.snapshot, nav_key);
            }
            KeyCode::Esc => {
                self.navigation.model_picker_open = false;
                self.status_line = "Model picker closed.".to_string();
            }
            KeyCode::Enter => {
                self.navigation.model_picker_open = false;
                self.status_line = format!(
                    "Model locked to {}.",
                    self.navigation
                        .selected_model_id
                        .as_deref()
                        .unwrap_or("unset")
                );
            }
            _ => {}
        }
    }

    fn handle_provider_picker_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Left | KeyCode::Up => {
                self.provider_picker_index =
                    cycle_index(self.provider_picker_index, self.provider_choices.len(), -1);
            }
            KeyCode::Right | KeyCode::Down | KeyCode::Tab => {
                self.provider_picker_index =
                    cycle_index(self.provider_picker_index, self.provider_choices.len(), 1);
            }
            KeyCode::Esc => {
                self.provider_picker_open = false;
                self.status_line = "Provider picker closed.".to_string();
            }
            KeyCode::Enter => {
                self.provider_picker_open = false;
                if let Some(provider) = self
                    .provider_choices
                    .get(self.provider_picker_index)
                    .copied()
                {
                    self.provider_id = provider;
                    self.navigation.selected_model_id =
                        Some(ProviderManager::default_model(provider).to_string());
                    self.status_line = format!("Provider locked to {}.", provider);
                }
            }
            _ => {}
        }
    }

    fn handle_overlay_key(&mut self, key: KeyEvent) {
        match self.overlay.clone() {
            Some(OverlayState::CreateAgent { role_index }) => match key.code {
                KeyCode::Left | KeyCode::Up => {
                    self.overlay = Some(OverlayState::CreateAgent {
                        role_index: cycle_index(role_index, AGENT_ROLES.len(), -1),
                    });
                }
                KeyCode::Right | KeyCode::Down => {
                    self.overlay = Some(OverlayState::CreateAgent {
                        role_index: cycle_index(role_index, AGENT_ROLES.len(), 1),
                    });
                }
                KeyCode::Enter => {
                    self.overlay = None;
                    self.create_agent(AGENT_ROLES[role_index]);
                }
                KeyCode::Esc => {
                    self.overlay = None;
                    self.status_line = "Create agent canceled.".to_string();
                }
                _ => {}
            },
            Some(OverlayState::SwarmLaunch {
                mut goal,
                strategy_index,
            }) => match key.code {
                KeyCode::Left | KeyCode::Up => {
                    self.overlay = Some(OverlayState::SwarmLaunch {
                        goal,
                        strategy_index: cycle_index(strategy_index, SWARM_STRATEGIES.len(), -1),
                    });
                }
                KeyCode::Right | KeyCode::Down => {
                    self.overlay = Some(OverlayState::SwarmLaunch {
                        goal,
                        strategy_index: cycle_index(strategy_index, SWARM_STRATEGIES.len(), 1),
                    });
                }
                KeyCode::Backspace => {
                    let _ = goal.pop();
                    self.overlay = Some(OverlayState::SwarmLaunch {
                        goal,
                        strategy_index,
                    });
                }
                KeyCode::Char(ch) => {
                    goal.push(ch);
                    self.overlay = Some(OverlayState::SwarmLaunch {
                        goal,
                        strategy_index,
                    });
                }
                KeyCode::Enter => {
                    if goal.trim().is_empty() {
                        self.status_line = "Swarm launch needs a goal.".to_string();
                    } else {
                        self.overlay = None;
                        self.launch_swarm(goal, SWARM_STRATEGIES[strategy_index]);
                        self.input_mode = InputMode::Prompt;
                    }
                }
                KeyCode::Esc => {
                    self.overlay = None;
                    self.input_mode = InputMode::Prompt;
                    self.status_line = "Swarm launch canceled.".to_string();
                }
                _ => {}
            },
            None => {}
        }
    }

    fn activate_current_selection(&mut self) {
        match self.navigation.focused_pane {
            Pane::Input => {
                if self.navigation.command_buffer.trim().is_empty() {
                    self.status_line = "Type a prompt or /command.".to_string();
                    return;
                }

                if is_slash_mode(&self.navigation.command_buffer) {
                    self.execute_slash_command();
                    return;
                }

                let prompt = self.navigation.command_buffer.clone();
                self.navigation.command_buffer.clear();
                self.slash_menu_selected_index = 0;
                self.send_prompt(prompt);
            }
            Pane::Sessions => {
                self.navigation.focused_pane = Pane::Input;
                self.status_line = "Agent selected. Type a prompt.".to_string();
            }
            Pane::Runs => {
                self.status_line = "Run selected.".to_string();
            }
            Pane::Tasks => {
                self.status_line = "Task selected.".to_string();
            }
            Pane::Actions | Pane::Events => {
                self.navigation.focused_pane = Pane::Input;
            }
        }
    }

    fn navigate_slash_menu(&mut self, delta: isize) -> bool {
        if self.navigation.focused_pane != Pane::Input
            || !is_slash_mode(&self.navigation.command_buffer)
        {
            return false;
        }

        let commands = filtered_commands(&self.navigation.command_buffer);
        if commands.is_empty() {
            return false;
        }

        let len = commands.len() as isize;
        self.slash_menu_selected_index =
            (self.slash_menu_selected_index as isize + delta).rem_euclid(len) as usize;
        true
    }

    fn sync_slash_index(&mut self) {
        if !is_slash_mode(&self.navigation.command_buffer) {
            self.slash_menu_selected_index = 0;
            return;
        }

        let commands = filtered_commands(&self.navigation.command_buffer);
        if commands.is_empty() {
            self.slash_menu_selected_index = 0;
        } else {
            self.slash_menu_selected_index = self
                .slash_menu_selected_index
                .min(commands.len().saturating_sub(1));
        }
    }

    fn execute_slash_command(&mut self) {
        let commands = filtered_commands(&self.navigation.command_buffer);
        let command = commands
            .get(
                self.slash_menu_selected_index
                    .min(commands.len().saturating_sub(1)),
            )
            .copied()
            .or_else(|| parse_exact_slash_command(&self.navigation.command_buffer));

        self.navigation.command_buffer.clear();
        self.slash_menu_selected_index = 0;

        let Some(command) = command else {
            self.status_line = "Unknown command.".to_string();
            return;
        };

        match command.id {
            SlashCommandId::Model => {
                self.navigation.model_picker_open = true;
                self.status_line = "Choose a model with arrows.".to_string();
            }
            SlashCommandId::Provider => {
                self.provider_picker_open = true;
                self.provider_picker_index = self
                    .provider_choices
                    .iter()
                    .position(|provider| *provider == self.provider_id)
                    .unwrap_or(0);
                self.status_line = "Choose a provider with arrows.".to_string();
            }
            SlashCommandId::New => {
                self.overlay = Some(OverlayState::CreateAgent { role_index: 0 });
                self.status_line = "Create agent: choose role, then Enter.".to_string();
            }
            SlashCommandId::Agents => {
                self.navigation.focused_pane = Pane::Sessions;
                self.status_line = "Sidebar focused on agents.".to_string();
            }
            SlashCommandId::Runs => {
                self.navigation.focused_pane = Pane::Runs;
                self.status_line = "Sidebar focused on runs.".to_string();
            }
            SlashCommandId::Tasks => {
                self.navigation.focused_pane = Pane::Tasks;
                self.status_line = "Sidebar focused on tasks.".to_string();
            }
            SlashCommandId::Review => {
                self.status_line = "Review flow is not wired yet.".to_string();
            }
            SlashCommandId::Permissions => {
                self.status_line = "Permissions: local execution only.".to_string();
            }
            SlashCommandId::Share => {
                let state = self
                    .snapshot
                    .share
                    .as_ref()
                    .and_then(|share| share.get("state"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("idle");
                self.status_line = format!("Tunnel state: {state}.");
            }
            SlashCommandId::Preflight => {
                self.navigation.focused_pane = Pane::Runs;
                self.status_line = "Run preflight with `vorker preflight <repo>`.".to_string();
            }
            SlashCommandId::Help => {
                self.status_line =
                    "Commands: /model /provider /new /agents /runs /tasks /share /preflight"
                        .to_string();
            }
        }
    }

    fn rebuild_snapshot(&mut self) {
        let mut store = vorker_core::SupervisorStore::new();
        for event in self.snapshot.events.clone() {
            store.append(event);
        }
        self.snapshot = store.snapshot();
    }

    fn create_agent(&mut self, role: &str) {
        let agent_id = format!("agent-{}", self.next_agent_index);
        let agent_name = format!("Agent {}", self.next_agent_index);
        self.next_agent_index += 1;
        let model = self
            .navigation
            .selected_model_id
            .clone()
            .unwrap_or_else(|| "gpt-5.4".to_string());
        self.snapshot.events.push(create_supervisor_event(
            "session.registered",
            serde_json::json!({
                "session": {
                    "id": agent_id,
                    "name": agent_name,
                    "role": role,
                    "status": "ready",
                    "provider": self.provider_id.to_string(),
                    "model": model,
                    "cwd": self.workspace_path
                }
            }),
        ));
        self.rebuild_snapshot();
        self.navigation.active_session_id = Some(agent_id);
        self.navigation.focused_pane = Pane::Input;
        self.status_line = format!(
            "Created {role} on {}.",
            self.navigation
                .selected_model_id
                .as_deref()
                .unwrap_or("the selected model")
        );
    }

    fn launch_swarm(&mut self, goal: String, strategy: &str) {
        let run_id = format!("run-{}", self.next_run_index);
        self.next_run_index += 1;
        let task_one = format!("task-{}", self.next_task_index);
        let task_two = format!("task-{}", self.next_task_index + 1);
        self.next_task_index += 2;

        self.snapshot.events.push(create_supervisor_event(
            "run.created",
            serde_json::json!({
                "run": {
                    "id": run_id,
                    "name": format!("Swarm {}", self.next_run_index - 1),
                    "goal": goal,
                    "status": "running",
                    "workerAgentIds": [],
                    "createdAt": now_iso(),
                    "updatedAt": now_iso()
                }
            }),
        ));
        self.snapshot.events.push(create_supervisor_event(
            "task.created",
            serde_json::json!({
                "task": {
                    "id": task_one,
                    "runId": run_id,
                    "title": "Plan the swarm",
                    "description": "Draft the work graph",
                    "status": "running",
                    "createdAt": now_iso(),
                    "updatedAt": now_iso()
                }
            }),
        ));
        self.snapshot.events.push(create_supervisor_event(
            "task.created",
            serde_json::json!({
                "task": {
                    "id": task_two,
                    "runId": run_id,
                    "title": "Execute the first lane",
                    "description": "Start the first worker lane",
                    "status": "ready",
                    "createdAt": now_iso(),
                    "updatedAt": now_iso()
                }
            }),
        ));
        self.rebuild_snapshot();
        self.navigation.active_run_id = Some(run_id);
        self.navigation.selected_task_id = Some(task_one);
        self.navigation.focused_pane = Pane::Runs;
        self.status_line = format!("Swarm launched with {strategy} strategy.");
    }

    fn send_prompt(&mut self, prompt: String) {
        let Some(session_id) = self.navigation.active_session_id.clone() else {
            self.status_line = "Create an agent first.".to_string();
            return;
        };

        self.snapshot.events.push(create_supervisor_event(
            "session.prompt.started",
            serde_json::json!({
                "sessionId": session_id,
                "message": {
                    "role": "user",
                    "text": prompt
                }
            }),
        ));
        self.snapshot.events.push(create_supervisor_event(
            "session.prompt.finished",
            serde_json::json!({
                "sessionId": session_id,
                "message": {
                    "role": "assistant",
                    "text": format!("Acknowledged: {prompt}")
                }
            }),
        ));

        self.rebuild_snapshot();
        self.status_line = "Prompt recorded.".to_string();
    }
}

fn cycle_index(current: usize, len: usize, delta: isize) -> usize {
    ((current as isize + delta).rem_euclid(len as isize)) as usize
}

fn parse_exact_slash_command(buffer: &str) -> Option<crate::slash::SlashCommand> {
    let command = buffer.split_whitespace().next().unwrap_or_default().trim();
    crate::slash::SLASH_COMMANDS
        .iter()
        .copied()
        .find(|entry| entry.name == command)
}

#[must_use]
pub fn render_once(width: usize) -> String {
    App::new(load_bootstrap_snapshot()).render(width, false)
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

pub fn run_app(no_alt_screen: bool) -> io::Result<()> {
    let mut app = App::new(load_bootstrap_snapshot());
    enable_raw_mode()?;
    let mut stdout = io::stdout();

    if !no_alt_screen {
        execute!(stdout, EnterAlternateScreen, Hide)?;
    }

    loop {
        let width = size()
            .map(|(columns, _)| usize::from(columns))
            .unwrap_or(120);
        let frame = normalize_for_raw_terminal(&app.render(width, true));
        execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
        write!(stdout, "{frame}")?;
        stdout.flush()?;

        if let Event::Key(key) = read()?
            && !app.handle_key(key)
        {
            break;
        }
    }

    if !no_alt_screen {
        execute!(stdout, Show, LeaveAlternateScreen)?;
    }
    disable_raw_mode()?;
    Ok(())
}

fn load_bootstrap_snapshot() -> Snapshot {
    let Ok(cwd) = std::env::current_dir() else {
        return Snapshot::default();
    };
    let log_root = cwd.join(".vorker-2").join("logs");
    let event_log = EventLog::new(&log_root, Some(log_root.join("supervisor.ndjson")));
    restore_durable_supervisor_state(&event_log).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::normalize_for_raw_terminal;

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
}
