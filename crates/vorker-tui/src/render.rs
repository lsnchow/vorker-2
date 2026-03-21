use serde_json::Value;
use vorker_core::{RunSnapshot, Snapshot, TaskRecord};

use crate::navigation::{ActionItem, NavigationState, Pane};
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
    let stacked_layout = width < 108;
    let left_width = if stacked_layout { width } else { width.clamp(32, 34) };
    let right_width = if stacked_layout {
        width
    } else {
        width - left_width - 1
    };
    let action_panel = render_action_rail(&options, width, color);
    let navigation_panel = render_navigation_panel(snapshot, &options, left_width, color);
    let main_panel = render_main_surface(snapshot, &options, right_width, color);
    let secondary_panel = render_secondary_surface(snapshot, &options, right_width, color);

    if stacked_layout {
        [
            render_header(snapshot, &options, width, color),
            action_panel,
            navigation_panel,
            render_main_surface(snapshot, &options, width, color),
            render_secondary_surface(snapshot, &options, width, color),
            render_footer(snapshot, &options, width, color),
        ]
        .join("\n")
    } else {
        [
            render_header(snapshot, &options, width, color),
            action_panel,
            combine_columns(
                &navigation_panel,
                &[main_panel, secondary_panel].join("\n"),
                1,
            )
            .join("\n"),
            render_footer(snapshot, &options, width, color),
        ]
        .join("\n")
    }
}

fn render_header(
    snapshot: &Snapshot,
    options: &DashboardOptions,
    width: usize,
    color: bool,
) -> String {
    let model = options.selected_model_id.as_deref().unwrap_or("unset");
    let target = current_target(snapshot, options);
    let tunnel = share_field(snapshot.share.as_ref(), "state")
        .unwrap_or("idle")
        .to_uppercase();
    let status = truncate(&options.status_line, 40);
    let line = format!(
        "{}  model {}  target {}  focus {}  tunnel {}  status {}",
        emphasize(&colorize("[vorker]", "brightGreen", color), color),
        model,
        target,
        options.focused_pane,
        tunnel,
        status
    );
    truncate(&line, width)
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
    if options.selected_action_id == ActionItem::NewAgent {
        lines.push(colorize(
            &format!(
                "enter: create agent on {}",
                options
                    .selected_model_id
                    .as_deref()
                    .unwrap_or("selected model")
            ),
            "gray",
            color,
        ));
    } else if options.selected_action_id == ActionItem::Swarm {
        lines.push(colorize(
            &format!(
                "enter: start swarm on {}",
                options
                    .selected_model_id
                    .as_deref()
                    .unwrap_or("selected model")
            ),
            "magenta",
            color,
        ));
    } else {
        lines.push(colorize(
            &format!(
                "enter: change persistent model ({})",
                options.selected_model_id.as_deref().unwrap_or("unset")
            ),
            "gray",
            color,
        ));
    }

    build_panel(
        "ACTIONS",
        &lines,
        width,
        options.focused_pane == Pane::Actions || options.model_picker_open,
    )
}

fn render_model_picker(options: &DashboardOptions, width: usize, color: bool) -> Option<String> {
    if !options.model_picker_open {
        return None;
    }

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

    Some(build_panel(
        "MODEL PICKER",
        &[
            format!("models {}", models.join("  ")),
            colorize("arrows: change  enter: keep  esc: close", "gray", color),
        ],
        width,
        true,
    ))
}

fn render_navigation_panel(
    snapshot: &Snapshot,
    options: &DashboardOptions,
    width: usize,
    color: bool,
) -> String {
    let mut lines = vec![section_label("Agents", options.focused_pane == Pane::Sessions, color)];
    if snapshot.sessions.is_empty() {
        lines.push(colorize("  none yet", "gray", color));
    } else {
        lines.extend(snapshot.sessions.iter().map(|session| {
            let selected = options.active_session_id.as_deref() == Some(session.id.as_str());
            let line = format!(
                "{} {} [{}]",
                if selected { ">" } else { " " },
                session.name,
                session.status
            );
            style_selectable(&line, selected, color, false)
        }));
    }

    lines.push(String::new());
    lines.push(section_label("Runs", options.focused_pane == Pane::Runs, color));
    if snapshot.runs.is_empty() {
        lines.push(colorize("  none yet", "gray", color));
    } else {
        lines.extend(snapshot.runs.iter().map(|run| {
            let selected = options.active_run_id.as_deref() == Some(run.id.as_str());
            let line = format!(
                "{} {} [{}]",
                if selected { ">" } else { " " },
                run.name,
                run.status
            );
            style_selectable(&line, selected, color, false)
        }));
    }

    lines.push(String::new());
    lines.push(section_label("Tasks", options.focused_pane == Pane::Tasks, color));
    let tasks = snapshot
        .runs
        .iter()
        .find(|run| options.active_run_id.as_deref() == Some(run.id.as_str()))
        .map(|run| run.tasks.as_slice())
        .unwrap_or(&[]);
    if tasks.is_empty() {
        lines.push(colorize("  select a run", "gray", color));
    } else {
        lines.extend(tasks.iter().map(|task| {
            let selected = options.selected_task_id.as_deref() == Some(task.id.as_str());
            let line = format!(
                "{} {} [{}]",
                if selected { ">" } else { " " },
                task.title,
                task.status
            );
            style_selectable(&line, selected, color, false)
        }));
    }

    build_panel("NAVIGATION", &lines, width, matches!(options.focused_pane, Pane::Sessions | Pane::Runs | Pane::Tasks))
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
            "RUN OVERVIEW",
            &[colorize("No run selected yet.", "gray", color)],
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
        lines.push(colorize("selected", "gray", color));
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
        "RUN OVERVIEW",
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
            "TRANSCRIPT",
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
    lines.push(colorize("transcript", "gray", color));

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

    build_panel("TRANSCRIPT", &lines, width, false)
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
        .map(|event| format!("* {}", summarize_event(event.kind.as_str(), &event.payload)))
        .collect::<Vec<_>>();

    let fallback = [colorize("No supervisor events yet.", "gray", color)];
    build_panel(
        "ACTIVITY",
        if lines.is_empty() { &fallback } else { &lines },
        width,
        options.focused_pane == Pane::Events,
    )
}

fn render_main_surface(
    snapshot: &Snapshot,
    options: &DashboardOptions,
    width: usize,
    color: bool,
) -> String {
    if options.create_agent_overlay_open {
        return render_create_agent_overlay(options, width, color);
    }
    if options.model_picker_open {
        return render_model_picker(options, width, color)
            .unwrap_or_else(|| build_panel("MODEL PICKER", &[], width, true));
    }
    if options.swarm_overlay_open {
        return render_swarm_overlay(options, width, color);
    }
    if snapshot.sessions.is_empty() && snapshot.runs.is_empty() {
        return render_get_started(width, color);
    }
    if options.focused_pane == Pane::Tasks
        && let Some(panel) = render_task_detail(snapshot, options, width, color)
    {
        return panel;
    }
    if options.focused_pane == Pane::Runs {
        return render_run_board(snapshot, options, width, color);
    }
    render_active_session(snapshot, options, width, color)
}

fn render_secondary_surface(
    snapshot: &Snapshot,
    options: &DashboardOptions,
    width: usize,
    color: bool,
) -> String {
    if let Some(panel) = render_task_inspector(snapshot, options, width, color) {
        return panel;
    }
    render_event_feed(snapshot, options, width, color)
}

fn render_get_started(width: usize, color: bool) -> String {
    build_panel(
        "GET STARTED",
        &[
            emphasize("Create agent", color),
            colorize(
                "Start one operator-facing Copilot worker and land in transcript view.",
                "gray",
                color,
            ),
            String::new(),
            emphasize("Launch swarm", color),
            colorize(
                "Start a run with planning plus worker lanes, then switch into run overview.",
                "gray",
                color,
            ),
        ],
        width,
        false,
    )
}

fn render_create_agent_overlay(options: &DashboardOptions, width: usize, color: bool) -> String {
    build_panel(
        "CREATE AGENT",
        &[
            format!(
                "role {}",
                highlight(
                    options.create_agent_role.as_deref().unwrap_or("worker"),
                    color,
                    "bgGreen",
                    "black",
                )
            ),
            format!(
                "model {}",
                options.selected_model_id.as_deref().unwrap_or("gpt-5.4")
            ),
            colorize("arrows choose role  enter creates  esc closes", "gray", color),
        ],
        width,
        true,
    )
}

fn render_swarm_overlay(options: &DashboardOptions, width: usize, color: bool) -> String {
    build_panel(
        "LAUNCH SWARM",
        &[
            format!("goal {}", options.swarm_goal),
            format!(
                "model {}",
                options.selected_model_id.as_deref().unwrap_or("gpt-5.4")
            ),
            format!(
                "strategy {}",
                options.swarm_strategy.as_deref().unwrap_or("parallel")
            ),
            colorize("type goal  arrows change strategy  enter launches", "gray", color),
        ],
        width,
        true,
    )
}

fn render_task_detail(
    snapshot: &Snapshot,
    options: &DashboardOptions,
    width: usize,
    _color: bool,
) -> Option<String> {
    let run = snapshot
        .runs
        .iter()
        .find(|entry| options.active_run_id.as_deref() == Some(entry.id.as_str()))
        .or_else(|| snapshot.runs.first())?;
    let task = run
        .tasks
        .iter()
        .find(|entry| options.selected_task_id.as_deref() == Some(entry.id.as_str()))
        .or_else(|| run.tasks.first())?;

    let mut lines = vec![
        format!("task {}", task.title),
        format!("status {}", task.status),
        format!(
            "agent {}",
            task.execution_agent_id
                .as_deref()
                .or(task.assigned_agent_id.as_deref())
                .unwrap_or("queue")
        ),
    ];
    if let Some(path) = &task.workspace_path {
        append_field(&mut lines, "workspace", path, width, true);
    }
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

    Some(build_panel("TASK DETAIL", &lines, width, options.focused_pane == Pane::Tasks))
}

fn render_task_inspector(
    snapshot: &Snapshot,
    options: &DashboardOptions,
    width: usize,
    _color: bool,
) -> Option<String> {
    let run = snapshot
        .runs
        .iter()
        .find(|entry| options.active_run_id.as_deref() == Some(entry.id.as_str()))
        .or_else(|| snapshot.runs.first())?;
    let task = run
        .tasks
        .iter()
        .find(|entry| options.selected_task_id.as_deref() == Some(entry.id.as_str()))
        .or_else(|| run.tasks.first())?;

    let mut lines = vec![
        format!("run {}", run.name),
        format!("task {}", task.title),
        format!("status {}", task.status),
    ];
    if let Some(agent) = task
        .execution_agent_id
        .as_deref()
        .or(task.assigned_agent_id.as_deref())
    {
        append_field(&mut lines, "agent", agent, width, false);
    }
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
    if let Some(path) = &task.workspace_path {
        append_field(&mut lines, "workspace", path, width, true);
    }
    Some(build_panel("TASK INSPECTOR", &lines, width, options.focused_pane == Pane::Tasks || options.focused_pane == Pane::Runs || options.focused_pane == Pane::Events))
}

fn render_footer(
    snapshot: &Snapshot,
    options: &DashboardOptions,
    width: usize,
    color: bool,
) -> String {
    let share_state = share_field(snapshot.share.as_ref(), "state")
        .unwrap_or("idle")
        .to_uppercase();
    let target = current_target(snapshot, options);
    let mode = match options.input_mode {
        InputMode::SwarmGoal => "swarm-goal",
        InputMode::Prompt => "prompt",
    };
    let input_placeholder = match options.input_mode {
        InputMode::SwarmGoal => "Describe the swarm goal and press Enter",
        InputMode::Prompt => {
            if options.focused_pane == Pane::Tasks && options.selected_task_id.is_some() {
                "Ask the task agent what to do next"
            } else if options.focused_pane == Pane::Runs && options.active_run_id.is_some() {
                "Inspect the run or move into a task"
            } else if options.active_session_id.is_some() {
                "Type to prompt the selected agent"
            } else {
                "Create an agent first, then type a prompt"
            }
        }
    };

    let mut lines = vec![
        format!(
            "target {}    mode {}    tunnel {}",
            target, mode, share_state
        ),
        format!(
            "{} {}",
            colorize(">", "brightGreen", color),
            if options.command_buffer.is_empty() {
                input_placeholder.to_string()
            } else {
                options.command_buffer.clone()
            }
        ),
    ];
    if let Some(url) = share_field(snapshot.share.as_ref(), "publicUrl") {
        append_field(&mut lines, "share", url, width, true);
    }
    lines.push(colorize(
        "arrows move  enter activates  esc cancels picker/prompt  ctrl+c quits",
        "gray",
        color,
    ));
    build_panel("INPUT", &lines, width, true)
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
    format!("[{}]", colorize(label, tone, color))
}

fn build_panel(title: &str, lines: &[String], width: usize, focused: bool) -> String {
    let inner_width = width.saturating_sub(2).max(12);
    let title_label = if focused {
        format!(">{title}<")
    } else {
        title.to_string()
    };
    let plain_title = format!(" {title_label} ");
    let filler_width = inner_width.saturating_sub(plain_title.len());
    let left_fill = "-".repeat(filler_width / 2);
    let right_fill = "-".repeat(filler_width - (filler_width / 2));
    let top = format!("+{}{}{}+", left_fill, plain_title, right_fill);
    let body = if lines.is_empty() {
        vec![format!("|{}|", " ".repeat(inner_width))]
    } else {
        lines
            .iter()
            .map(|line| format!("|{}|", fit(line, inner_width)))
            .collect()
    };
    let bottom = format!("+{}+", "-".repeat(inner_width));

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

fn section_label(label: &str, focused: bool, color: bool) -> String {
    if focused {
        colorize(&format!("[{label}]"), "brightGreen", color)
    } else {
        colorize(label, "gray", color)
    }
}

fn lane_meter(status: &str) -> &'static str {
    match status {
        "completed" | "merged" => "####",
        "running" | "planning" | "starting" => "###.",
        "ready" => "#...",
        "failed" | "error" | "conflict" => "!!!!",
        _ => "....",
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

fn current_target(snapshot: &Snapshot, options: &DashboardOptions) -> String {
    if options.swarm_overlay_open {
        return "swarm launch".to_string();
    }
    if options.create_agent_overlay_open {
        return "create agent".to_string();
    }
    if options.model_picker_open {
        return "model picker".to_string();
    }
    if options.focused_pane == Pane::Tasks
        && let Some(run) = snapshot
            .runs
            .iter()
            .find(|entry| options.active_run_id.as_deref() == Some(entry.id.as_str()))
            .or_else(|| snapshot.runs.first())
        && let Some(task) = run
            .tasks
            .iter()
            .find(|entry| options.selected_task_id.as_deref() == Some(entry.id.as_str()))
            .or_else(|| run.tasks.first())
    {
        return format!("task {}", task.id);
    }
    if options.focused_pane == Pane::Runs
        && let Some(run) = snapshot
            .runs
            .iter()
            .find(|entry| options.active_run_id.as_deref() == Some(entry.id.as_str()))
            .or_else(|| snapshot.runs.first())
    {
        return format!("run {}", run.name);
    }
    snapshot
        .sessions
        .iter()
        .find(|session| options.active_session_id.as_deref() == Some(session.id.as_str()))
        .or_else(|| snapshot.sessions.first())
        .map(|session| format!("agent {}", session.name))
        .unwrap_or_else(|| "none".to_string())
}

#[allow(dead_code)]
fn _active_run<'a>(snapshot: &'a Snapshot, options: &DashboardOptions) -> Option<&'a RunSnapshot> {
    snapshot
        .runs
        .iter()
        .find(|run| options.active_run_id.as_deref() == Some(run.id.as_str()))
        .or_else(|| snapshot.runs.first())
}
