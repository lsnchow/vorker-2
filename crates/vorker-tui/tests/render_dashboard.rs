use vorker_core::{RunSnapshot, SessionRecord, Snapshot, TaskRecord, TranscriptEntry};
use vorker_tui::{DashboardOptions, render_dashboard};

#[test]
fn render_dashboard_prints_sessions_runs_tasks_and_tunnel_status() {
    let output = render_dashboard(
        &Snapshot {
            sessions: vec![SessionRecord {
                id: "agent-1".to_string(),
                name: "Planner".to_string(),
                role: "arbitrator".to_string(),
                status: "ready".to_string(),
                model: Some("gpt-5.4".to_string()),
                transcript: vec![
                    TranscriptEntry {
                        role: "user".to_string(),
                        text: "Plan the work".to_string(),
                    },
                    TranscriptEntry {
                        role: "assistant".to_string(),
                        text: "Plan ready".to_string(),
                    },
                ],
                ..SessionRecord::default()
            }],
            runs: vec![RunSnapshot {
                id: "run-1".to_string(),
                name: "Bootstrap".to_string(),
                goal: "Build the supervisor core".to_string(),
                status: "running".to_string(),
                tasks: vec![TaskRecord {
                    id: "task-1".to_string(),
                    title: "Wire event bus".to_string(),
                    run_id: "run-1".to_string(),
                    status: "completed".to_string(),
                    assigned_agent_id: Some("agent-1".to_string()),
                    execution_agent_id: Some("exec-1".to_string()),
                    branch_name: Some("vorker/task-task-1-wire-event-bus".to_string()),
                    workspace_path: Some("/repo/.vorker-2/worktrees/task-1".to_string()),
                    commit_sha: Some("abc123def456".to_string()),
                    change_count: 1,
                    ..TaskRecord::default()
                }],
                ..RunSnapshot::default()
            }],
            share: Some(serde_json::json!({
                "state": "ready",
                "publicUrl": "https://example.trycloudflare.com?transport=poll"
            })),
            events: vec![
                vorker_core::create_supervisor_event(
                    "task.updated",
                    serde_json::json!({ "task": { "title": "Wire event bus", "status": "completed" } }),
                ),
                vorker_core::create_supervisor_event(
                    "run.updated",
                    serde_json::json!({ "run": { "name": "Bootstrap", "status": "running" } }),
                ),
            ],
            ..Snapshot::default()
        },
        DashboardOptions {
            selected_action_id: "swarm".parse().expect("action"),
            selected_model_id: Some("gpt-5.4".to_string()),
            model_choices: vec!["gpt-5.4".to_string(), "gpt-5".to_string()],
            active_session_id: Some("agent-1".to_string()),
            active_run_id: Some("run-1".to_string()),
            width: 100,
            status_line: "Ready for commands.".to_string(),
            ..DashboardOptions::default()
        },
    );

    for needle in [
        "[vorker]",
        "ACTIONS",
        "NAVIGATION",
        "NEW AGENT",
        "SWARM",
        "gpt-5.4",
        "TRANSCRIPT",
        "TASK INSPECTOR",
        "INPUT",
        "Planner",
        "Bootstrap",
        "Wire event bus",
        "vorker/task-task-1-wire-event-bus",
        "exec-1",
        "abc123def456",
        "ready",
        "Plan ready",
        "Ready for",
    ] {
        assert!(
            output.contains(needle),
            "missing {needle} in output:\n{output}"
        );
    }
}

#[test]
fn render_dashboard_empty_state_uses_a_getting_started_surface() {
    let output = render_dashboard(
        &Snapshot::default(),
        DashboardOptions {
            width: 120,
            ..DashboardOptions::default()
        },
    );

    for needle in [
        "GET STARTED",
        "Create agent",
        "Launch swarm",
        "NAVIGATION",
        "INPUT",
    ] {
        assert!(
            output.contains(needle),
            "missing {needle} in output:\n{output}"
        );
    }
}

#[test]
fn render_dashboard_uses_a_compact_operator_header() {
    let output = render_dashboard(
        &Snapshot::default(),
        DashboardOptions {
            width: 120,
            ..DashboardOptions::default()
        },
    );

    assert!(
        output.contains("[vorker]"),
        "missing compact title bar:\n{output}"
    );
    assert!(
        !output.contains("__     ______"),
        "large ascii masthead should be gone:\n{output}"
    );
}

#[test]
fn render_dashboard_respects_narrow_terminal_widths() {
    let output = render_dashboard(
        &Snapshot::default(),
        DashboardOptions {
            width: 60,
            status_line: "Ready.".to_string(),
            ..DashboardOptions::default()
        },
    );

    let lines = output.lines().collect::<Vec<_>>();
    assert!(
        lines.iter().all(|line| line.chars().count() <= 60),
        "renderer overflowed narrow terminal:\n{output}"
    );
    assert!(
        !lines
            .iter()
            .any(|line| line.contains("AGENTS") && line.contains("DETAIL")),
        "narrow layout should stack panels instead of printing side-by-side:\n{output}"
    );
}

#[test]
fn render_dashboard_uses_ascii_safe_borders() {
    let output = render_dashboard(&Snapshot::default(), DashboardOptions::default());

    assert!(
        !output.contains('┌')
            && !output.contains('┐')
            && !output.contains('└')
            && !output.contains('┘')
            && !output.contains('│')
            && !output.contains('─'),
        "dashboard should avoid unicode border glyphs that break alignment:\n{output}"
    );
}

#[test]
fn render_dashboard_surfaces_create_agent_overlay() {
    let output = render_dashboard(
        &Snapshot::default(),
        DashboardOptions {
            width: 120,
            create_agent_overlay_open: true,
            create_agent_role: Some("planner".to_string()),
            selected_model_id: Some("gpt-5.4".to_string()),
            ..DashboardOptions::default()
        },
    );

    assert!(
        output.contains("CREATE AGENT"),
        "create-agent flow should be rendered as an overlay panel:\n{output}"
    );
    assert!(output.contains("planner"), "missing chosen role:\n{output}");
}

#[test]
fn render_dashboard_surfaces_a_real_model_picker_panel() {
    let output = render_dashboard(
        &Snapshot::default(),
        DashboardOptions {
            width: 120,
            model_picker_open: true,
            selected_action_id: "model".parse().expect("action"),
            selected_model_id: Some("gpt-5.4".to_string()),
            model_choices: vec!["gpt-5.4".to_string(), "gpt-5".to_string()],
            ..DashboardOptions::default()
        },
    );

    assert!(
        output.contains("MODEL PICKER"),
        "picker mode should be rendered as a distinct panel:\n{output}"
    );
}

#[test]
fn render_dashboard_surfaces_swarm_launch_overlay() {
    let output = render_dashboard(
        &Snapshot::default(),
        DashboardOptions {
            width: 120,
            swarm_overlay_open: true,
            swarm_goal: "ship the runtime".to_string(),
            swarm_strategy: Some("parallel".to_string()),
            selected_model_id: Some("gpt-5.4".to_string()),
            ..DashboardOptions::default()
        },
    );

    assert!(
        output.contains("LAUNCH SWARM"),
        "swarm launch flow should be rendered as an overlay panel:\n{output}"
    );
    assert!(
        output.contains("ship the runtime"),
        "missing swarm goal:\n{output}"
    );
}

#[test]
fn render_dashboard_uses_focus_styling_when_color_is_enabled() {
    let output = render_dashboard(
        &Snapshot::default(),
        DashboardOptions {
            width: 120,
            color: true,
            ..DashboardOptions::default()
        },
    );

    assert!(
        output.contains("\u{1b}["),
        "color-enabled render should emit ansi styling for focus affordance:\n{output}"
    );
}

#[test]
fn render_dashboard_surfaces_preflight_stage_and_risk_for_selected_run() {
    let output = render_dashboard(
        &Snapshot {
            runs: vec![RunSnapshot {
                id: "preflight-1".to_string(),
                name: "Preflight octocat/hello-world".to_string(),
                goal: "Vet https://github.com/octocat/Hello-World".to_string(),
                status: "running".to_string(),
                run_type: Some("preflight".to_string()),
                preflight: Some(vorker_core::PreflightRecord {
                    run_id: "preflight-1".to_string(),
                    repo_input: "https://github.com/octocat/Hello-World".to_string(),
                    repo_source_type: "github".to_string(),
                    stage: "setup".to_string(),
                    classification: Some("web app".to_string()),
                    classification_confidence: Some("0.86".to_string()),
                    risk_level: Some("medium".to_string()),
                    sandbox_backend: Some("docker".to_string()),
                    sandbox_state: Some("running".to_string()),
                    latest_failure: Some("missing .env".to_string()),
                    artifacts_dir: Some("/tmp/preflight-1".to_string()),
                    ..vorker_core::PreflightRecord::default()
                }),
                ..RunSnapshot::default()
            }],
            ..Snapshot::default()
        },
        DashboardOptions {
            width: 120,
            active_run_id: Some("preflight-1".to_string()),
            ..DashboardOptions::default()
        },
    );

    for needle in [
        "preflight",
        "setup",
        "web app",
        "medium",
        "docker",
        "missing .env",
        "/tmp/preflight-1",
    ] {
        assert!(
            output.contains(needle),
            "missing {needle} in output:\n{output}"
        );
    }
}

#[test]
fn render_dashboard_only_highlights_input_when_input_pane_is_focused() {
    let input_focused = render_dashboard(
        &Snapshot::default(),
        DashboardOptions {
            width: 120,
            focused_pane: vorker_tui::Pane::Input,
            ..DashboardOptions::default()
        },
    );
    let sessions_focused = render_dashboard(
        &Snapshot::default(),
        DashboardOptions {
            width: 120,
            focused_pane: vorker_tui::Pane::Sessions,
            ..DashboardOptions::default()
        },
    );

    assert!(
        input_focused.contains(">INPUT<"),
        "input pane should show focus when selected:\n{input_focused}"
    );
    assert!(
        !sessions_focused.contains(">INPUT<"),
        "input pane should not appear focused when another pane is selected:\n{sessions_focused}"
    );
}

#[test]
fn render_dashboard_avoids_internal_separator_rows_inside_panels() {
    let output = render_dashboard(
        &Snapshot::default(),
        DashboardOptions {
            width: 120,
            ..DashboardOptions::default()
        },
    );

    assert!(
        !output.lines().any(|line| line.starts_with("|-----")),
        "content panels should not draw fake internal separator rows:\n{output}"
    );
}

#[test]
fn render_dashboard_never_hits_terminal_wrap_column() {
    let width = 120;
    let output = render_dashboard(
        &Snapshot::default(),
        DashboardOptions {
            width,
            ..DashboardOptions::default()
        },
    );

    let lines = output.lines().collect::<Vec<_>>();
    assert!(
        lines.iter().all(|line| line.chars().count() < width),
        "renderer should stay under the terminal width to avoid wrap-pending misalignment:\n{output}"
    );
}

#[test]
fn render_dashboard_leaves_extra_margin_for_real_terminals() {
    let width = 120;
    let output = render_dashboard(
        &Snapshot::default(),
        DashboardOptions {
            width,
            ..DashboardOptions::default()
        },
    );

    let lines = output.lines().collect::<Vec<_>>();
    assert!(
        lines.iter().all(|line| line.chars().count() <= width - 4),
        "renderer should leave a few spare columns to avoid terminal-specific wrapping drift:\n{output}"
    );
}

#[test]
fn render_dashboard_uses_split_navigation_layout_on_roomy_terminals() {
    let output = render_dashboard(
        &Snapshot::default(),
        DashboardOptions {
            width: 120,
            ..DashboardOptions::default()
        },
    );

    assert!(
        output
            .lines()
            .any(|line| line.contains("NAVIGATION") && line.contains("GET STARTED")),
        "120-column terminals should use a left navigation column with a main work surface:\n{output}"
    );
}

#[test]
fn render_dashboard_output_is_ascii_only() {
    let output = render_dashboard(
        &Snapshot::default(),
        DashboardOptions {
            width: 120,
            ..DashboardOptions::default()
        },
    );

    assert!(
        output.is_ascii(),
        "dashboard output should stay ASCII-only for terminal compatibility:\n{output}"
    );
}
