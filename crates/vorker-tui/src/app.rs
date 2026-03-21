use crossterm::cursor::MoveTo;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, read};
use crossterm::execute;
use crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode, size,
};
use std::io::{self, Write};

use vorker_core::{Snapshot, create_supervisor_event, now_iso};

use crate::navigation::{
    ActionItem, NavKey, NavigationState, apply_navigation_key, reconcile_navigation_state,
};
use crate::render::{DashboardOptions, InputMode, render_dashboard};

pub struct App {
    pub snapshot: Snapshot,
    pub navigation: NavigationState,
    pub status_line: String,
    pub input_mode: InputMode,
    next_agent_index: usize,
    next_run_index: usize,
    next_task_index: usize,
}

impl App {
    #[must_use]
    pub fn new(snapshot: Snapshot) -> Self {
        let navigation = reconcile_navigation_state(&snapshot, NavigationState::default());
        Self {
            snapshot,
            navigation,
            status_line: "Ready for commands.".to_string(),
            input_mode: InputMode::Prompt,
            next_agent_index: 1,
            next_run_index: 1,
            next_task_index: 1,
        }
    }

    pub fn render(&self, width: usize) -> String {
        render_dashboard(
            &self.snapshot,
            DashboardOptions {
                width,
                status_line: self.status_line.clone(),
                input_mode: self.input_mode.clone(),
                focused_pane: self.navigation.focused_pane,
                selected_action_id: self.navigation.selected_action_id,
                selected_model_id: self.navigation.selected_model_id.clone(),
                model_choices: self.navigation.model_choices.clone(),
                model_picker_open: self.navigation.model_picker_open,
                active_session_id: self.navigation.active_session_id.clone(),
                active_run_id: self.navigation.active_run_id.clone(),
                selected_task_id: self.navigation.selected_task_id.clone(),
                command_buffer: self.navigation.command_buffer.clone(),
                ..DashboardOptions::default()
            },
        )
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return false;
        }

        match key.code {
            KeyCode::Left => {
                self.navigation =
                    apply_navigation_key(self.navigation.clone(), &self.snapshot, NavKey::Left)
            }
            KeyCode::Right => {
                self.navigation =
                    apply_navigation_key(self.navigation.clone(), &self.snapshot, NavKey::Right)
            }
            KeyCode::Up => {
                self.navigation =
                    apply_navigation_key(self.navigation.clone(), &self.snapshot, NavKey::Up)
            }
            KeyCode::Down => {
                self.navigation =
                    apply_navigation_key(self.navigation.clone(), &self.snapshot, NavKey::Down)
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
                if self.navigation.model_picker_open {
                    self.navigation.model_picker_open = false;
                    self.status_line = "Model picker closed.".to_string();
                } else {
                    self.navigation.command_buffer.clear();
                    self.input_mode = InputMode::Prompt;
                    self.status_line = "Input cleared.".to_string();
                }
            }
            KeyCode::Enter => self.activate_current_selection(),
            KeyCode::Backspace => {
                let _ = self.navigation.command_buffer.pop();
            }
            KeyCode::Char(ch) => {
                self.navigation.command_buffer.push(ch);
            }
            _ => {}
        }

        self.navigation = reconcile_navigation_state(&self.snapshot, self.navigation.clone());
        true
    }

    fn activate_current_selection(&mut self) {
        if self.navigation.model_picker_open {
            self.navigation.model_picker_open = false;
            self.status_line = format!(
                "Model locked to {}.",
                self.navigation
                    .selected_model_id
                    .as_deref()
                    .unwrap_or("unset")
            );
            return;
        }

        if !self.navigation.command_buffer.is_empty() {
            match self.input_mode {
                InputMode::SwarmGoal => {
                    self.launch_swarm(self.navigation.command_buffer.clone());
                    self.navigation.command_buffer.clear();
                    self.input_mode = InputMode::Prompt;
                }
                InputMode::Prompt => {
                    self.send_prompt(self.navigation.command_buffer.clone());
                    self.navigation.command_buffer.clear();
                }
            }
            return;
        }

        match self.navigation.selected_action_id {
            ActionItem::Model => {
                self.navigation.model_picker_open = true;
                self.status_line = "Choose a model with arrows.".to_string();
            }
            ActionItem::NewAgent => self.create_agent(),
            ActionItem::Swarm => {
                self.input_mode = InputMode::SwarmGoal;
                self.status_line = "Describe the swarm goal and press Enter.".to_string();
            }
        }
    }

    fn create_agent(&mut self) {
        self.next_agent_index += 1;
        let agent_id = format!("agent-{}", self.next_agent_index);
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
                    "name": format!("Agent {}", self.next_agent_index),
                    "role": "worker",
                    "status": "ready",
                    "model": model,
                    "cwd": "."
                }
            }),
        ));
        let mut store = vorker_core::SupervisorStore::new();
        for event in self.snapshot.events.clone() {
            store.append(event);
        }
        self.snapshot = store.snapshot();
        self.status_line = format!(
            "Created agent on {}.",
            self.navigation
                .selected_model_id
                .as_deref()
                .unwrap_or("the selected model")
        );
    }

    fn launch_swarm(&mut self, goal: String) {
        self.next_run_index += 1;
        self.next_task_index += 2;
        let run_id = format!("run-{}", self.next_run_index);
        let task_one = format!("task-{}", self.next_task_index - 1);
        let task_two = format!("task-{}", self.next_task_index);

        self.snapshot.events.push(create_supervisor_event(
            "run.created",
            serde_json::json!({
                "run": {
                    "id": run_id,
                    "name": format!("Swarm {}", self.next_run_index),
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
        let mut store = vorker_core::SupervisorStore::new();
        for event in self.snapshot.events.clone() {
            store.append(event);
        }
        self.snapshot = store.snapshot();
        self.status_line = "Swarm launched.".to_string();
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
                    "text": format!("Acknowledged: {}", self.navigation.command_buffer)
                }
            }),
        ));

        let mut store = vorker_core::SupervisorStore::new();
        for event in self.snapshot.events.clone() {
            store.append(event);
        }
        self.snapshot = store.snapshot();
        self.status_line = "Prompt recorded.".to_string();
    }
}

#[must_use]
pub fn render_once(width: usize) -> String {
    App::new(Snapshot::default()).render(width)
}

pub fn run_app(no_alt_screen: bool) -> io::Result<()> {
    let mut app = App::new(Snapshot::default());
    enable_raw_mode()?;
    let mut stdout = io::stdout();

    if !no_alt_screen {
        execute!(stdout, EnterAlternateScreen, Hide)?;
    }

    loop {
        let width = size()
            .map(|(columns, _)| usize::from(columns))
            .unwrap_or(120);
        execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
        write!(stdout, "{}", app.render(width))?;
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
