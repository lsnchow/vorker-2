use serde_json::Value;
use vorker_core::{Snapshot, TaskRecord};

use crate::navigation::{ActionItem, NavigationState, Pane};
use crate::slash::filtered_commands;
use crate::theme::{colorize, emphasize, fit, hard_wrap, highlight, pad, truncate, visible_length};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputMode {
    Prompt,
    SwarmGoal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DashboardOptions {
    pub color: bool,
    pub width: usize,
    pub provider_id: String,
    pub workspace_path: String,
    pub status_line: String,
    pub focused_pane: Pane,
    pub selected_action_id: ActionItem,
    pub selected_model_id: Option<String>,
    pub model_choices: Vec<String>,
    pub model_picker_open: bool,
    pub active_session_id: Option<String>,
    pub active_run_id: Option<String>,
    pub selected_task_id: Option<String>,
    pub command_buffer: String,
    pub slash_menu_selected_index: usize,
    pub input_mode: InputMode,
    pub create_agent_overlay_open: bool,
    pub create_agent_role: Option<String>,
    pub swarm_overlay_open: bool,
    pub swarm_goal: String,
    pub swarm_strategy: Option<String>,
}

impl Default for DashboardOptions {
    fn default() -> Self {
        Self {
            color: false,
            width: 120,
            provider_id: "copilot".to_string(),
            workspace_path: ".".to_string(),
            status_line: "Ready.".to_string(),
            focused_pane: Pane::Input,
            selected_action_id: ActionItem::NewAgent,
            selected_model_id: None,
            model_choices: Vec::new(),
            model_picker_open: false,
            active_session_id: None,
            active_run_id: None,
            selected_task_id: None,
            command_buffer: String::new(),
            slash_menu_selected_index: 0,
            input_mode: InputMode::Prompt,
            create_agent_overlay_open: false,
            create_agent_role: None,
            swarm_overlay_open: false,
            swarm_goal: String::new(),
            swarm_strategy: None,
        }
    }
}

impl From<NavigationState> for DashboardOptions {
    fn from(value: NavigationState) -> Self {
        Self {
            focused_pane: value.focused_pane,
            selected_action_id: value.selected_action_id,
            active_session_id: value.active_session_id,
            active_run_id: value.active_run_id,
            selected_task_id: value.selected_task_id,
            selected_model_id: value.selected_model_id,
            model_choices: value.model_choices,
            model_picker_open: value.model_picker_open,
            command_buffer: value.command_buffer,
            ..Self::default()
        }
    }
}

pub fn render_dashboard(snapshot: &Snapshot, options: DashboardOptions) -> String {
    let color = options.color;
    let width = options.width.clamp(60, 160).saturating_sub(4).max(40);
    let show_sidebar = width >= 84;
    let sidebar_width = if show_sidebar { width.min(28) } else { 0 };
    let main_width = if show_sidebar {
        width.saturating_sub(sidebar_width + 1)
    } else {
        width
    };

    let header = render_header(snapshot, &options, width, color);
    let sidebar = if show_sidebar {
        Some(render_sidebar(snapshot, &options, sidebar_width, color))
    } else {
        None
    };
    let main = render_main(snapshot, &options, main_width, color);

    let body = if let Some(sidebar) = sidebar {
        combine_columns(&sidebar, &main, 1).join("\n")
    } else {
        main
    };

    [header, body].join("\n")
}

fn render_header(
    snapshot: &Snapshot,
    options: &DashboardOptions,
    width: usize,
    color: bool,
) -> String {
    let app_name = emphasize(&colorize("[vorker]", "brightGreen", color), color);
    let provider = options.provider_id.as_str();
    let model = options.selected_model_id.as_deref().unwrap_or("gpt-5.4");
    let cwd = truncate(&options.workspace_path, 22);
    let target = truncate(&current_target(snapshot, options), 24);
    let status = truncate(&options.status_line, 28);
    truncate(
        &format!(
            "{app_name}  provider {provider}  model {model}  cwd {cwd}  target {target}  status {status}"
        ),
        width,
    )
}

fn render_sidebar(
    snapshot: &Snapshot,
    options: &DashboardOptions,
    width: usize,
    color: bool,
) -> String {
    let mut lines = Vec::new();
    lines.push(section_heading(
        "Chats",
        options.focused_pane == Pane::Sessions,
        color,
    ));
    if let Some(session) = active_session(snapshot, options) {
        lines.push(style_row(
            &truncate(&format!("> {}", session.name), width.saturating_sub(4)),
            options.focused_pane == Pane::Sessions,
            color,
        ));
    } else {
        lines.push(colorize("  none", "gray", color));
    }

    lines.push(String::new());
    lines.push(section_heading(
        "Agents",
        options.focused_pane == Pane::Sessions,
        color,
    ));
    if snapshot.sessions.is_empty() {
        lines.push(colorize("  none", "gray", color));
    } else {
        lines.extend(snapshot.sessions.iter().map(|session| {
            let selected = options.active_session_id.as_deref() == Some(session.id.as_str());
            let line = truncate(
                &format!(
                    "{} {} [{}]",
                    if selected { ">" } else { " " },
                    session.name,
                    if session.status.is_empty() {
                        "unknown"
                    } else {
                        session.status.as_str()
                    }
                ),
                width.saturating_sub(4),
            );
            style_row(
                &line,
                selected && options.focused_pane == Pane::Sessions,
                color,
            )
        }));
    }

    lines.push(String::new());
    lines.push(section_heading(
        "Runs",
        options.focused_pane == Pane::Runs,
        color,
    ));
    if snapshot.runs.is_empty() {
        lines.push(colorize("  none", "gray", color));
    } else {
        lines.extend(snapshot.runs.iter().map(|run| {
            let selected = options.active_run_id.as_deref() == Some(run.id.as_str());
            let line = truncate(
                &format!(
                    "{} {} [{}]",
                    if selected { ">" } else { " " },
                    run.name,
                    if run.status.is_empty() {
                        "idle"
                    } else {
                        run.status.as_str()
                    }
                ),
                width.saturating_sub(4),
            );
            style_row(&line, selected && options.focused_pane == Pane::Runs, color)
        }));
    }

    lines.push(String::new());
    lines.push(section_heading(
        "Tasks",
        options.focused_pane == Pane::Tasks,
        color,
    ));
    let tasks = active_run_tasks(snapshot, options);
    if tasks.is_empty() {
        lines.push(colorize("  select a run", "gray", color));
    } else {
        lines.extend(tasks.iter().map(|task| {
            let selected = options.selected_task_id.as_deref() == Some(task.id.as_str());
            style_row(
                &format!(
                    "{} {} [{}]",
                    if selected { ">" } else { " " },
                    truncate(&task.title, width.saturating_sub(10)),
                    if task.status.is_empty() {
                        "idle"
                    } else {
                        task.status.as_str()
                    }
                ),
                selected && options.focused_pane == Pane::Tasks,
                color,
            )
        }));
    }

    build_panel("Navigation", &lines, width, false)
}

fn render_main(
    snapshot: &Snapshot,
    options: &DashboardOptions,
    width: usize,
    color: bool,
) -> String {
    let mut parts = vec![render_conversation(snapshot, options, width, color)];
    if let Some(overlay) = render_overlay(options, width, color) {
        parts.push(overlay);
    }
    parts.push(render_composer(snapshot, options, width, color));
    if let Some(slash_panel) = render_slash_commands(options, width, color) {
        parts.push(slash_panel);
    }
    parts.join("\n")
}

fn render_conversation(
    snapshot: &Snapshot,
    options: &DashboardOptions,
    width: usize,
    _color: bool,
) -> String {
    let mut lines = Vec::new();

    if let Some(session) = active_session(snapshot, options) {
        for entry in &session.transcript {
            lines.push(format!(
                "{:<10}{}",
                entry.role,
                truncate(&entry.text, width.saturating_sub(16))
            ));
        }
    }

    for event in snapshot.events.iter().rev().take(4).rev() {
        lines.push(format!(
            "{:<10}{}",
            "tool",
            truncate(
                &summarize_event(event.kind.as_str(), &event.payload),
                width.saturating_sub(16)
            )
        ));
    }

    if lines.is_empty() {
        lines.push("Start typing to talk to an agent.".to_string());
        lines.push("Try /new to create an agent.".to_string());
        lines.push("Try /help to see available commands.".to_string());
    }

    build_panel(
        "Conversation",
        &lines,
        width,
        options.focused_pane == Pane::Input,
    )
}

fn render_overlay(options: &DashboardOptions, width: usize, color: bool) -> Option<String> {
    if options.model_picker_open {
        let mut lines = Vec::new();
        for model in &options.model_choices {
            let selected = options.selected_model_id.as_deref() == Some(model.as_str());
            let label = if selected {
                highlight(&format!(" {model} "), color, "bgGreen", "black")
            } else {
                format!("[{model}]")
            };
            lines.push(label);
        }
        lines.push(colorize(
            "arrows move  enter keeps  esc closes",
            "gray",
            color,
        ));
        return Some(build_panel("MODEL PICKER", &lines, width, true));
    }

    if options.create_agent_overlay_open {
        return Some(build_panel(
            "CREATE AGENT",
            &[
                format!(
                    "role {}",
                    options.create_agent_role.as_deref().unwrap_or("worker")
                ),
                format!(
                    "model {}",
                    options.selected_model_id.as_deref().unwrap_or("gpt-5.4")
                ),
                colorize("arrows choose  enter creates  esc closes", "gray", color),
            ],
            width,
            true,
        ));
    }

    if options.swarm_overlay_open {
        return Some(build_panel(
            "LAUNCH SWARM",
            &[
                format!("goal {}", options.swarm_goal),
                format!(
                    "strategy {}",
                    options.swarm_strategy.as_deref().unwrap_or("parallel")
                ),
                colorize(
                    "type goal  arrows choose strategy  enter launches",
                    "gray",
                    color,
                ),
            ],
            width,
            true,
        ));
    }

    None
}

fn render_composer(
    snapshot: &Snapshot,
    options: &DashboardOptions,
    width: usize,
    color: bool,
) -> String {
    let target = current_target(snapshot, options);
    let share_state = share_field(snapshot.share.as_ref(), "state")
        .unwrap_or("idle")
        .to_uppercase();
    let placeholder = match options.input_mode {
        InputMode::SwarmGoal => "Describe the swarm goal",
        InputMode::Prompt => "Type a prompt or /command",
    };
    let prompt = if options.command_buffer.is_empty() {
        placeholder.to_string()
    } else {
        options.command_buffer.clone()
    };

    let target_hint = truncate(&target, width.saturating_sub(24));
    build_panel(
        "Composer",
        &[
            format!(
                "target {}  mode {}  tunnel {}",
                target_hint,
                match options.input_mode {
                    InputMode::Prompt => "prompt",
                    InputMode::SwarmGoal => "swarm-goal",
                },
                share_state
            ),
            format!("{} {}", colorize(">", "brightGreen", color), prompt),
        ],
        width,
        options.focused_pane == Pane::Input,
    )
}

fn render_slash_commands(options: &DashboardOptions, width: usize, color: bool) -> Option<String> {
    let commands = filtered_commands(&options.command_buffer);
    if commands.is_empty() {
        return None;
    }

    let selected_index = options
        .slash_menu_selected_index
        .min(commands.len().saturating_sub(1));
    let mut lines = Vec::new();
    for (index, command) in commands.iter().enumerate() {
        let line = format!(
            "{} {}  {}",
            if index == selected_index { ">" } else { " " },
            command.name,
            command.description
        );
        lines.push(if index == selected_index {
            if color {
                highlight(&line, true, "bgGray", "white")
            } else {
                line
            }
        } else {
            line
        });
    }

    Some(build_panel(
        "Commands",
        &lines,
        width,
        options.focused_pane == Pane::Input,
    ))
}

fn build_panel(title: &str, lines: &[String], width: usize, focused: bool) -> String {
    let inner_width = width.saturating_sub(2).max(12);
    let title_label = if focused {
        format!(">{title}<")
    } else {
        title.to_string()
    };
    let title_text = format!(" {title_label} ");
    let filler_width = inner_width.saturating_sub(title_text.len());
    let left_fill = "-".repeat(filler_width / 2);
    let right_fill = "-".repeat(filler_width - left_fill.len());
    let top = format!("+{}{}{}+", left_fill, title_text, right_fill);
    let mut body = Vec::new();
    if lines.is_empty() {
        body.push(format!("|{}|", " ".repeat(inner_width)));
    } else {
        for line in lines {
            body.extend(wrap_panel_line(line, inner_width));
        }
    }
    let bottom = format!("+{}+", "-".repeat(inner_width));
    std::iter::once(top)
        .chain(body)
        .chain(std::iter::once(bottom))
        .collect::<Vec<_>>()
        .join("\n")
}

fn wrap_panel_line(line: &str, inner_width: usize) -> Vec<String> {
    let visible = visible_length(line);
    if visible <= inner_width {
        return vec![format!("|{}|", fit(line, inner_width))];
    }

    hard_wrap(line, inner_width)
        .into_iter()
        .map(|chunk| format!("|{}|", fit(&chunk, inner_width)))
        .collect()
}

fn combine_columns(left: &str, right: &str, gap: usize) -> Vec<String> {
    let left_lines = left.lines().collect::<Vec<_>>();
    let right_lines = right.lines().collect::<Vec<_>>();
    let left_width = left_lines
        .iter()
        .map(|line| visible_length(line))
        .max()
        .unwrap_or(0);
    let height = left_lines.len().max(right_lines.len());
    let mut output = Vec::with_capacity(height);
    for index in 0..height {
        let left = left_lines.get(index).copied().unwrap_or("");
        let right = right_lines.get(index).copied().unwrap_or("");
        output.push(format!(
            "{}{}{}",
            pad(left, left_width),
            " ".repeat(gap),
            right
        ));
    }
    output
}

fn section_heading(label: &str, focused: bool, color: bool) -> String {
    if focused {
        emphasize(&colorize(label, "brightGreen", color), color)
    } else {
        label.to_string()
    }
}

fn style_row(line: &str, selected: bool, color: bool) -> String {
    if selected {
        highlight(line, color, "bgGreen", "black")
    } else {
        line.to_string()
    }
}

fn active_session<'a>(
    snapshot: &'a Snapshot,
    options: &DashboardOptions,
) -> Option<&'a vorker_core::SessionRecord> {
    snapshot
        .sessions
        .iter()
        .find(|session| options.active_session_id.as_deref() == Some(session.id.as_str()))
        .or_else(|| snapshot.sessions.first())
}

fn active_run_tasks<'a>(snapshot: &'a Snapshot, options: &DashboardOptions) -> &'a [TaskRecord] {
    snapshot
        .runs
        .iter()
        .find(|run| options.active_run_id.as_deref() == Some(run.id.as_str()))
        .or_else(|| snapshot.runs.first())
        .map(|run| run.tasks.as_slice())
        .unwrap_or(&[])
}

fn summarize_event(kind: &str, payload: &Value) -> String {
    match kind {
        "task.updated" => format!(
            "task {} -> {}",
            payload["task"]["title"]
                .as_str()
                .or_else(|| payload["task"]["id"].as_str())
                .unwrap_or("unknown"),
            payload["task"]["status"].as_str().unwrap_or("updated")
        ),
        "task.created" => format!(
            "task {} created",
            payload["task"]["title"]
                .as_str()
                .or_else(|| payload["task"]["id"].as_str())
                .unwrap_or("unknown")
        ),
        "run.updated" => format!(
            "run {} -> {}",
            payload["run"]["name"]
                .as_str()
                .or_else(|| payload["run"]["id"].as_str())
                .unwrap_or("unknown"),
            payload["run"]["status"].as_str().unwrap_or("updated")
        ),
        "run.created" => format!(
            "run {} created",
            payload["run"]["name"]
                .as_str()
                .or_else(|| payload["run"]["id"].as_str())
                .unwrap_or("unknown")
        ),
        "session.registered" => format!(
            "session {} ready",
            payload["session"]["name"]
                .as_str()
                .or_else(|| payload["session"]["id"].as_str())
                .unwrap_or("unknown")
        ),
        "session.prompt.started" => format!(
            "prompt -> {}",
            payload["sessionId"].as_str().unwrap_or("session")
        ),
        "session.prompt.finished" => format!(
            "reply <- {}",
            payload["sessionId"].as_str().unwrap_or("session")
        ),
        "share.updated" => format!(
            "tunnel {}",
            payload["share"]["state"].as_str().unwrap_or("idle")
        ),
        _ => kind.to_string(),
    }
}

fn share_field<'a>(share: Option<&'a Value>, field: &str) -> Option<&'a str> {
    share
        .and_then(|value| value.get(field))
        .and_then(Value::as_str)
}

fn current_target(snapshot: &Snapshot, options: &DashboardOptions) -> String {
    if options.model_picker_open {
        return "model picker".to_string();
    }
    if options.create_agent_overlay_open {
        return "create agent".to_string();
    }
    if options.swarm_overlay_open {
        return "swarm launch".to_string();
    }
    active_session(snapshot, options)
        .map(|session| format!("agent {}", session.name))
        .or_else(|| {
            snapshot
                .runs
                .iter()
                .find(|run| options.active_run_id.as_deref() == Some(run.id.as_str()))
                .or_else(|| snapshot.runs.first())
                .map(|run| format!("run {}", run.name))
        })
        .unwrap_or_else(|| "none".to_string())
}
