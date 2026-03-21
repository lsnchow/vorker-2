use serde_json::Value;
use vorker_core::{RunSnapshot, Snapshot, TaskRecord};

use crate::navigation::{ActionItem, NavigationState, Pane};
use crate::theme::{
    TITLE_ART, colorize, emphasize, fit, hard_wrap, highlight, pad, truncate, visible_length,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputMode {
    Prompt,
    SwarmGoal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DashboardOptions {
    pub color: bool,
    pub width: usize,
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
    pub input_mode: InputMode,
}

impl Default for DashboardOptions {
    fn default() -> Self {
        Self {
            color: false,
            width: 120,
            status_line: "Ready.".to_string(),
            focused_pane: Pane::Actions,
            selected_action_id: ActionItem::NewAgent,
            selected_model_id: None,
            model_choices: Vec::new(),
            model_picker_open: false,
            active_session_id: None,
            active_run_id: None,
            selected_task_id: None,
            command_buffer: String::new(),
            input_mode: InputMode::Prompt,
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
    let width = options.width.clamp(80, 160);
    let left_width = if width >= 130 { 46 } else { width * 42 / 100 };
    let right_width = width - left_width - 1;

    let session_panel = render_session_list(snapshot, &options, left_width, color);
    let run_panel = render_run_board(snapshot, &options, left_width, color);
    let active_panel = render_active_session(snapshot, &options, right_width, color);
    let event_panel = render_event_feed(snapshot, &options, right_width, color);

    [
        render_banner(width, color),
        render_action_rail(&options, width, color),
        combine_columns(&session_panel, &active_panel, 1).join("\n"),
        combine_columns(&run_panel, &event_panel, 1).join("\n"),
        render_footer(snapshot, &options, width, color),
    ]
    .join("\n")
}

fn render_banner(width: usize, color: bool) -> String {
    let strapline = format!(
        "{}   {}   {}",
        emphasize(
            &colorize("VORKER CONTROL PLANE", "brightGreen", color),
            color
        ),
        colorize("VORKER-2 supervisor mesh", "green", color),
        colorize("agents left / launch rail top / swarm pink", "gray", color)
    );

    [
        TITLE_ART.iter().map(|line| colorize(line, "brightGreen", color)).collect::<Vec<_>>().join("\n"),
        strapline,
        colorize(
            "Use arrows to pick a model, spawn an agent, or launch a swarm. Enter commits the selection.",
            "gray",
            color,
        ),
        colorize(&"─".repeat(width.clamp(40, 120)), "green", color),
    ]
    .join("\n")
}

fn render_action_rail(options: &DashboardOptions, width: usize, color: bool) -> String {
    let model_chip = render_chip(
        &format!(
            "MODEL {}",
            options.selected_model_id.as_deref().unwrap_or("unset")
        ),
        options.selected_action_id == ActionItem::Model,
        "green",
        color,
    );
    let agent_chip = render_chip(
        "NEW AGENT",
        options.selected_action_id == ActionItem::NewAgent,
        "green",
        color,
    );
    let swarm_chip = render_chip(
        "SWARM",
        options.selected_action_id == ActionItem::Swarm,
        "magenta",
        color,
    );

    let mut lines = vec![format!("{model_chip}  {agent_chip}  {swarm_chip}")];
    if options.model_picker_open {
        let models = options
            .model_choices
            .iter()
            .map(|model| {
                render_chip(
                    model,
                    options.selected_model_id.as_deref() == Some(model),
                    "green",
                    color,
                )
            })
            .collect::<Vec<_>>();
        lines.push(format!("models {}", models.join("  ")));
        lines.push(colorize(
            "Choose a model with arrows. Enter keeps it. Escape closes the picker.",
            "gray",
            color,
        ));
    } else if options.selected_action_id == ActionItem::NewAgent {
        lines.push(colorize(
            &format!(
                "Press Enter to create a new agent on {}.",
                options
                    .selected_model_id
                    .as_deref()
                    .unwrap_or("the selected model")
            ),
            "gray",
            color,
        ));
    } else if options.selected_action_id == ActionItem::Swarm {
        lines.push(format!(
            "{} {}",
            colorize("pink lane", "brightMagenta", color),
            colorize(
                &format!(
                    "Press Enter, type the swarm goal, and Vorker will launch a planner plus workers on {}.",
                    options.selected_model_id.as_deref().unwrap_or("the selected model")
                ),
                "gray",
                color
            )
        ));
    } else {
        lines.push(colorize(
            &format!(
                "Current persistent model: {}.",
                options.selected_model_id.as_deref().unwrap_or("unset")
            ),
            "gray",
            color,
        ));
    }

    build_panel(
        "LAUNCH RAIL",
        &lines,
        width,
        options.focused_pane == Pane::Actions || options.model_picker_open,
    )
}

fn render_session_list(
    snapshot: &Snapshot,
    options: &DashboardOptions,
    width: usize,
    color: bool,
) -> String {
    let lines = if snapshot.sessions.is_empty() {
        vec![colorize(
            "No active agents. Move to NEW AGENT and press Enter.",
            "gray",
            color,
        )]
    } else {
        snapshot
            .sessions
            .iter()
            .map(|session| {
                let selected = options.active_session_id.as_deref() == Some(session.id.as_str());
                let model = session.model.as_deref().unwrap_or("no-model");
                let line = format!(
                    "{} {} {} [{}] {}",
                    if selected { "▶" } else { "•" },
                    session.name,
                    session.status.to_uppercase(),
                    if session.role.is_empty() {
                        "worker"
                    } else {
                        &session.role
                    },
                    model
                );
                style_selectable(&line, selected, color, false)
            })
            .collect()
    };

    build_panel(
        "ACTIVE AGENTS",
        &lines,
        width,
        options.focused_pane == Pane::Sessions,
    )
}

fn render_run_board(
    snapshot: &Snapshot,
    options: &DashboardOptions,
    width: usize,
    color: bool,
) -> String {
    let active_run = snapshot
        .runs
        .iter()
        .find(|run| options.active_run_id.as_deref() == Some(run.id.as_str()))
        .or_else(|| snapshot.runs.first());

    let Some(active_run) = active_run else {
        return build_panel(
            "RUN BOARD",
            &[colorize(
                "No runs yet. Launch a swarm to create one.",
                "gray",
                color,
            )],
            width,
            options.focused_pane == Pane::Runs || options.focused_pane == Pane::Tasks,
        );
    };

    let selected_task = active_run
        .tasks
        .iter()
        .find(|task| options.selected_task_id.as_deref() == Some(task.id.as_str()))
        .or_else(|| active_run.tasks.first());

    let mut lines = vec![
        format!(
            "run {} {}",
            active_run.name,
            active_run.status.to_uppercase()
        ),
        format!(
            "goal {}",
            truncate(&active_run.goal, width.saturating_sub(10))
        ),
        format!(
            "lanes hot={} ready={} done={} fail={}",
            count_tasks(&active_run.tasks, "running"),
            count_tasks(&active_run.tasks, "ready"),
            count_tasks(&active_run.tasks, "completed"),
            count_tasks(&active_run.tasks, "failed")
        ),
        "─".repeat(width.saturating_sub(6).max(10)),
    ];

    if active_run.tasks.is_empty() {
        lines.push(colorize("This swarm has no task lanes yet.", "gray", color));
    } else {
        lines.extend(active_run.tasks.iter().take(7).map(|task| {
            let selected = selected_task.is_some_and(|entry| entry.id == task.id);
            let agent_id = task
                .execution_agent_id
                .as_deref()
                .or(task.assigned_agent_id.as_deref())
                .unwrap_or("queue");
            let line = format!(
                "{} {} {} {}",
                lane_meter(&task.status),
                task.title,
                task.status.to_uppercase(),
                agent_id
            );
            style_selectable(&line, selected, color, false)
        }));
    }

    if let Some(task) = selected_task {
        lines.push("─".repeat(width.saturating_sub(6).max(10)));
        lines.push(colorize("selected lane", "gray", color));
        append_field(&mut lines, "task", &task.title, width, false);
        append_field(
            &mut lines,
            "agent",
            task.execution_agent_id
                .as_deref()
                .or(task.assigned_agent_id.as_deref())
                .unwrap_or("queue"),
            width,
            false,
        );
        if let Some(branch) = &task.branch_name {
            append_field(&mut lines, "branch", branch, width, true);
        }
        if let Some(commit) = &task.commit_sha {
            append_field(
                &mut lines,
                "commit",
                &format!("{commit} ({} files)", task.change_count),
                width,
                false,
            );
        }
    }

    build_panel(
        "RUN BOARD",
        &lines,
        width,
        options.focused_pane == Pane::Runs || options.focused_pane == Pane::Tasks,
    )
}

fn render_active_session(
    snapshot: &Snapshot,
    options: &DashboardOptions,
    width: usize,
    color: bool,
) -> String {
    let session = snapshot
        .sessions
        .iter()
        .find(|entry| options.active_session_id.as_deref() == Some(entry.id.as_str()))
        .or_else(|| snapshot.sessions.first());

    let Some(session) = session else {
        return build_panel(
            "AGENT DETAIL",
            &[colorize("No active agent selected yet.", "gray", color)],
            width,
            false,
        );
    };

    let mut lines = vec![
        format!("name {}", session.name),
        format!(
            "role {}    status {}",
            if session.role.is_empty() {
                "worker"
            } else {
                &session.role
            },
            if session.status.is_empty() {
                "UNKNOWN".to_string()
            } else {
                session.status.to_uppercase()
            }
        ),
        format!("model {}", session.model.as_deref().unwrap_or("unset")),
    ];
    append_field(&mut lines, "cwd", &session.cwd, width, true);
    lines.push("─".repeat(width.saturating_sub(6).max(10)));

    if session.transcript.is_empty() {
        lines.push(colorize("No transcript yet.", "gray", color));
    } else {
        lines.extend(
            session
                .transcript
                .iter()
                .rev()
                .take(6)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .map(|entry| {
                    format!(
                        "{:<9} {}",
                        entry.role,
                        truncate(&entry.text, width.saturating_sub(12))
                    )
                }),
        );
    }

    build_panel("AGENT DETAIL", &lines, width, false)
}

fn render_event_feed(
    snapshot: &Snapshot,
    options: &DashboardOptions,
    width: usize,
    color: bool,
) -> String {
    let lines = snapshot
        .events
        .iter()
        .rev()
        .take(8)
        .map(|event| format!("• {}", summarize_event(event.kind.as_str(), &event.payload)))
        .collect::<Vec<_>>();

    let fallback = [colorize("No supervisor events yet.", "gray", color)];
    build_panel(
        "EVENT FEED",
        if lines.is_empty() { &fallback } else { &lines },
        width,
        options.focused_pane == Pane::Events,
    )
}

fn render_footer(
    snapshot: &Snapshot,
    options: &DashboardOptions,
    width: usize,
    color: bool,
) -> String {
    let share_state = share_field(snapshot.share.as_ref(), "state").unwrap_or("idle");
    let input_label = match options.input_mode {
        InputMode::SwarmGoal => "swarm goal >",
        InputMode::Prompt => "prompt >",
    };
    let input_placeholder = match options.input_mode {
        InputMode::SwarmGoal => "Describe the swarm goal and press Enter",
        InputMode::Prompt => {
            if options.active_session_id.is_some() {
                "Type a prompt for the selected agent and press Enter"
            } else {
                "Create an agent first, then type a prompt"
            }
        }
    };

    let mut lines = vec![
        format!("status {}", options.status_line),
        format!(
            "focus {}    tunnel {}",
            options.focused_pane,
            share_state.to_uppercase()
        ),
    ];
    append_field(
        &mut lines,
        "url",
        share_field(snapshot.share.as_ref(), "publicUrl").unwrap_or("not shared"),
        width,
        true,
    );
    append_field(
        &mut lines,
        input_label,
        if options.command_buffer.is_empty() {
            input_placeholder
        } else {
            &options.command_buffer
        },
        width,
        true,
    );
    lines.push(colorize(
        "arrows move  enter activates  esc cancels picker/prompt  ctrl+c quits",
        "gray",
        color,
    ));
    build_panel("COMMAND DECK", &lines, width, true)
}

fn render_chip(label: &str, selected: bool, tone: &str, color: bool) -> String {
    if selected {
        return highlight(
            &format!(" {label} "),
            color,
            if tone == "magenta" {
                "bgMagenta"
            } else {
                "bgGreen"
            },
            "black",
        );
    }
    format!("[{label}]")
}

fn build_panel(title: &str, lines: &[String], width: usize, focused: bool) -> String {
    let inner_width = width.saturating_sub(2).max(12);
    let plain_title = format!(" {title} ");
    let filler_width = inner_width.saturating_sub(plain_title.len());
    let left_fill = "─".repeat(filler_width / 2);
    let right_fill = "─".repeat(filler_width - (filler_width / 2));
    let top = format!(
        "┌{}{}{}┐",
        left_fill,
        emphasize(&plain_title, focused),
        right_fill
    );
    let body = if lines.is_empty() {
        vec![format!("│{}│", " ".repeat(inner_width))]
    } else {
        lines
            .iter()
            .map(|line| format!("│{}│", fit(line, inner_width)))
            .collect()
    };
    let bottom = format!("└{}┘", "─".repeat(inner_width));

    std::iter::once(top)
        .chain(body)
        .chain(std::iter::once(bottom))
        .collect::<Vec<_>>()
        .join("\n")
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

fn append_field(lines: &mut Vec<String>, label: &str, value: &str, width: usize, stacked: bool) {
    let label_text = format!("  {label}");
    let inline = format!("{label_text} {value}");
    let inner_width = width.saturating_sub(2).max(12);
    if !stacked && visible_length(&inline) <= inner_width {
        lines.push(inline);
        return;
    }

    lines.push(label_text);
    let wrap_width = inner_width.saturating_sub(4).max(8);
    lines.extend(
        hard_wrap(value, wrap_width)
            .into_iter()
            .map(|chunk| format!("    {chunk}")),
    );
}

fn style_selectable(line: &str, selected: bool, color: bool, magenta: bool) -> String {
    if selected {
        highlight(
            line,
            color,
            if magenta { "bgMagenta" } else { "bgGreen" },
            "black",
        )
    } else {
        line.to_string()
    }
}

fn lane_meter(status: &str) -> &'static str {
    match status {
        "completed" | "merged" => "■■■■",
        "running" | "planning" | "starting" => "■■■□",
        "ready" => "■□□□",
        "failed" | "error" | "conflict" => "■■■■",
        _ => "□□□□",
    }
}

fn count_tasks(tasks: &[TaskRecord], status: &str) -> usize {
    tasks.iter().filter(|task| task.status == status).count()
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
        "skills.updated" => format!(
            "skills refreshed ({})",
            payload["skills"]
                .as_array()
                .map(std::vec::Vec::len)
                .unwrap_or(0)
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

#[allow(dead_code)]
fn _active_run<'a>(snapshot: &'a Snapshot, options: &DashboardOptions) -> Option<&'a RunSnapshot> {
    snapshot
        .runs
        .iter()
        .find(|run| options.active_run_id.as_deref() == Some(run.id.as_str()))
        .or_else(|| snapshot.runs.first())
}
