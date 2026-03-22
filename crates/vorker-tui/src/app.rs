use crossterm::cursor::MoveTo;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, read};
use crossterm::execute;
use crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode, size,
};
use std::io::{self, Write};

use vorker_core::{
    EventLog, Snapshot, create_supervisor_event, now_iso, restore_durable_supervisor_state,
};

use crate::navigation::{
    ActionItem, NavKey, NavigationState, apply_navigation_key, reconcile_navigation_state,
};
use crate::render::{DashboardOptions, InputMode, render_dashboard};

const AGENT_ROLES: [&str; 3] = ["worker", "planner", "reviewer"];
const SWARM_STRATEGIES: [&str; 3] = ["parallel", "review-first", "repair-first"];

#[derive(Clone, Debug, PartialEq, Eq)]
enum OverlayState {
    CreateAgent { role_index: usize },
    SwarmLaunch { goal: String, strategy_index: usize },
}

pub struct App {
    pub snapshot: Snapshot,
    pub navigation: NavigationState,
    pub status_line: String,
    pub input_mode: InputMode,
    overlay: Option<OverlayState>,
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
            overlay: None,
            next_agent_index: 1,
            next_run_index: 1,
            next_task_index: 1,
        }
    }

    pub fn render(&self, width: usize, color: bool) -> String {
        render_dashboard(
            &self.snapshot,
            DashboardOptions {
                color,
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

        if self.overlay.is_some() {
            self.handle_overlay_key(key);
            self.navigation = reconcile_navigation_state(&self.snapshot, self.navigation.clone());
            return true;
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
                self.navigation.command_buffer.clear();
                self.input_mode = InputMode::Prompt;
                self.status_line = "Input cleared.".to_string();
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

    fn handle_model_picker_key(&mut self, key: KeyEvent) {
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
        if !self.navigation.command_buffer.is_empty() {
            self.send_prompt(self.navigation.command_buffer.clone());
            self.navigation.command_buffer.clear();
            return;
        }

        match self.navigation.selected_action_id {
            ActionItem::Model => {
                self.navigation.model_picker_open = true;
                self.status_line = "Choose a model with arrows.".to_string();
            }
            ActionItem::NewAgent => {
                self.overlay = Some(OverlayState::CreateAgent { role_index: 0 });
                self.status_line = "Create agent: choose role, Enter confirms.".to_string();
            }
            ActionItem::Swarm => {
                self.overlay = Some(OverlayState::SwarmLaunch {
                    goal: String::new(),
                    strategy_index: 0,
                });
                self.input_mode = InputMode::SwarmGoal;
                self.status_line =
                    "Swarm launch: type goal, arrows change strategy, Enter confirms.".to_string();
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
                    "role": role,
                    "status": "ready",
                    "model": model,
                    "cwd": "."
                }
            }),
        ));
        self.rebuild_snapshot();
        self.navigation.active_session_id = Some(agent_id);
        self.navigation.focused_pane = crate::navigation::Pane::Sessions;
        self.status_line = format!(
            "Created {role} on {}.",
            self.navigation
                .selected_model_id
                .as_deref()
                .unwrap_or("the selected model")
        );
    }

    fn launch_swarm(&mut self, goal: String, strategy: &str) {
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
        self.rebuild_snapshot();
        self.navigation.active_run_id = Some(run_id);
        self.navigation.selected_task_id = Some(task_one);
        self.navigation.focused_pane = crate::navigation::Pane::Runs;
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
                    "text": format!("Acknowledged: {}", self.navigation.command_buffer)
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
