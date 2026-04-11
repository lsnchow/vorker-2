use std::fmt;

use vorker_core::Snapshot;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionItem {
    Model,
    NewAgent,
    Swarm,
}

pub const ACTION_ITEMS: [ActionItem; 3] =
    [ActionItem::Model, ActionItem::NewAgent, ActionItem::Swarm];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    Actions,
    Sessions,
    Runs,
    Tasks,
    Events,
    Input,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavKey {
    Left,
    Right,
    Up,
    Down,
    Tab,
    ShiftTab,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavigationState {
    pub focused_pane: Pane,
    pub selected_action_id: ActionItem,
    pub active_session_id: Option<String>,
    pub active_run_id: Option<String>,
    pub selected_task_id: Option<String>,
    pub selected_model_id: Option<String>,
    pub model_choices: Vec<String>,
    pub model_picker_open: bool,
}

impl Default for NavigationState {
    fn default() -> Self {
        Self {
            focused_pane: Pane::Input,
            selected_action_id: ActionItem::NewAgent,
            active_session_id: None,
            active_run_id: None,
            selected_task_id: None,
            selected_model_id: None,
            model_choices: Vec::new(),
            model_picker_open: false,
        }
    }
}

impl fmt::Display for ActionItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Model => "model",
            Self::NewAgent => "new-agent",
            Self::Swarm => "swarm",
        })
    }
}

impl fmt::Display for Pane {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Actions => "actions",
            Self::Sessions => "sessions",
            Self::Runs => "runs",
            Self::Tasks => "tasks",
            Self::Events => "events",
            Self::Input => "input",
        })
    }
}

impl std::str::FromStr for ActionItem {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "model" => Ok(Self::Model),
            "new-agent" => Ok(Self::NewAgent),
            "swarm" => Ok(Self::Swarm),
            _ => Err(format!("unknown action item: {value}")),
        }
    }
}

const PANE_ORDER: [Pane; 4] = [Pane::Input, Pane::Sessions, Pane::Runs, Pane::Tasks];
const FALLBACK_MODELS: [&str; 3] = ["gpt-5.4", "gpt-5", "gpt-4.1"];

#[must_use]
pub fn reconcile_navigation_state(snapshot: &Snapshot, state: NavigationState) -> NavigationState {
    let mut next = state;

    if !PANE_ORDER.contains(&next.focused_pane) {
        next.focused_pane = Pane::Input;
    }
    if !ACTION_ITEMS.contains(&next.selected_action_id) {
        next.selected_action_id = ActionItem::NewAgent;
    }

    next.model_choices = collect_model_choices(snapshot, &next);
    next.selected_model_id = if contains_id(&next.model_choices, next.selected_model_id.as_deref())
    {
        next.selected_model_id
    } else {
        next.model_choices.first().cloned()
    };

    let session_ids = session_ids(snapshot);
    next.active_session_id = if contains_id(&session_ids, next.active_session_id.as_deref()) {
        next.active_session_id
    } else {
        session_ids.first().cloned()
    };

    let run_ids = run_ids(snapshot);
    next.active_run_id = if contains_id(&run_ids, next.active_run_id.as_deref()) {
        next.active_run_id
    } else {
        run_ids.first().cloned()
    };

    let task_ids = task_ids(snapshot, next.active_run_id.as_deref());
    next.selected_task_id = if contains_id(&task_ids, next.selected_task_id.as_deref()) {
        next.selected_task_id
    } else {
        task_ids.first().cloned()
    };

    next
}

#[must_use]
pub fn apply_navigation_key(
    state: NavigationState,
    snapshot: &Snapshot,
    key: NavKey,
) -> NavigationState {
    let mut next = reconcile_navigation_state(snapshot, state);

    if next.model_picker_open {
        match key {
            NavKey::Left | NavKey::Up | NavKey::ShiftTab => {
                next.selected_model_id =
                    move_wrapped(&next.model_choices, next.selected_model_id.as_deref(), -1);
            }
            NavKey::Right | NavKey::Down | NavKey::Tab => {
                next.selected_model_id =
                    move_wrapped(&next.model_choices, next.selected_model_id.as_deref(), 1);
            }
        }
        return reconcile_navigation_state(snapshot, next);
    }

    match key {
        NavKey::Left => match next.focused_pane {
            Pane::Runs => next.focused_pane = Pane::Sessions,
            Pane::Tasks => next.focused_pane = Pane::Runs,
            _ => {}
        },
        NavKey::Right => match next.focused_pane {
            Pane::Sessions => next.focused_pane = Pane::Runs,
            Pane::Runs => next.focused_pane = Pane::Tasks,
            _ => {}
        },
        NavKey::Tab => next.focused_pane = cycle_focus(next.focused_pane, 1),
        NavKey::ShiftTab => next.focused_pane = cycle_focus(next.focused_pane, -1),
        NavKey::Up => match next.focused_pane {
            Pane::Sessions => {
                next.active_session_id = move_selection(
                    &session_ids(snapshot),
                    next.active_session_id.as_deref(),
                    -1,
                );
            }
            Pane::Runs => {
                next.active_run_id =
                    move_selection(&run_ids(snapshot), next.active_run_id.as_deref(), -1);
                next.selected_task_id = None;
            }
            Pane::Tasks => {
                next.selected_task_id = move_selection(
                    &task_ids(snapshot, next.active_run_id.as_deref()),
                    next.selected_task_id.as_deref(),
                    -1,
                );
            }
            _ => {}
        },
        NavKey::Down => match next.focused_pane {
            Pane::Sessions => {
                next.active_session_id =
                    move_selection(&session_ids(snapshot), next.active_session_id.as_deref(), 1);
            }
            Pane::Runs => {
                next.active_run_id =
                    move_selection(&run_ids(snapshot), next.active_run_id.as_deref(), 1);
                next.selected_task_id = None;
            }
            Pane::Tasks => {
                next.selected_task_id = move_selection(
                    &task_ids(snapshot, next.active_run_id.as_deref()),
                    next.selected_task_id.as_deref(),
                    1,
                );
            }
            _ => {}
        },
    }

    reconcile_navigation_state(snapshot, next)
}

fn collect_model_choices(snapshot: &Snapshot, state: &NavigationState) -> Vec<String> {
    let mut choices = Vec::new();
    for value in &state.model_choices {
        push_unique(&mut choices, value);
    }
    for session in &snapshot.sessions {
        if let Some(model) = &session.model {
            push_unique(&mut choices, model);
        }
    }
    for value in FALLBACK_MODELS {
        push_unique(&mut choices, value);
    }
    if let Some(model) = &state.selected_model_id {
        push_unique(&mut choices, model);
    }
    choices
}

fn push_unique(values: &mut Vec<String>, value: impl AsRef<str>) {
    let normalized = value.as_ref().trim();
    if normalized.is_empty() || values.iter().any(|entry| entry == normalized) {
        return;
    }
    values.push(normalized.to_string());
}

fn contains_id(ids: &[String], current: Option<&str>) -> bool {
    current.is_some_and(|value| ids.iter().any(|entry| entry == value))
}

fn move_selection(ids: &[String], current: Option<&str>, delta: isize) -> Option<String> {
    if ids.is_empty() {
        return None;
    }
    let current_index = current
        .and_then(|value| ids.iter().position(|entry| entry == value))
        .unwrap_or(0) as isize;
    let next_index = (current_index + delta).clamp(0, ids.len() as isize - 1) as usize;
    ids.get(next_index).cloned()
}

fn move_wrapped(ids: &[String], current: Option<&str>, delta: isize) -> Option<String> {
    if ids.is_empty() {
        return None;
    }
    let current_index = current
        .and_then(|value| ids.iter().position(|entry| entry == value))
        .unwrap_or(0) as isize;
    let len = ids.len() as isize;
    let next_index = (current_index + delta).rem_euclid(len) as usize;
    ids.get(next_index).cloned()
}

fn cycle_focus(current: Pane, delta: isize) -> Pane {
    let current_index = PANE_ORDER
        .iter()
        .position(|entry| *entry == current)
        .unwrap_or(0) as isize;
    let next_index = (current_index + delta).rem_euclid(PANE_ORDER.len() as isize) as usize;
    PANE_ORDER[next_index]
}

fn session_ids(snapshot: &Snapshot) -> Vec<String> {
    snapshot
        .sessions
        .iter()
        .map(|session| session.id.clone())
        .collect()
}

fn run_ids(snapshot: &Snapshot) -> Vec<String> {
    snapshot.runs.iter().map(|run| run.id.clone()).collect()
}

fn task_ids(snapshot: &Snapshot, run_id: Option<&str>) -> Vec<String> {
    snapshot
        .runs
        .iter()
        .find(|run| Some(run.id.as_str()) == run_id)
        .map(|run| run.tasks.iter().map(|task| task.id.clone()).collect())
        .unwrap_or_default()
}
