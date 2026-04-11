use vorker_core::{SessionRecord, Snapshot, TranscriptEntry};
use vorker_tui::{DashboardOptions, render_dashboard};

fn sample_snapshot() -> Snapshot {
    Snapshot {
        sessions: vec![SessionRecord {
            id: "agent-1".to_string(),
            name: "Thread 1".to_string(),
            status: "ready".to_string(),
            model: Some("claude-sonnet-4.5".to_string()),
            cwd: "/workspace".to_string(),
            transcript: vec![
                TranscriptEntry {
                    role: "system".to_string(),
                    text: "Model changed to claude-sonnet-4.5".to_string(),
                },
                TranscriptEntry {
                    role: "user".to_string(),
                    text: "hello".to_string(),
                },
                TranscriptEntry {
                    role: "assistant".to_string(),
                    text: "Hello. What do you need help with?".to_string(),
                },
            ],
            ..SessionRecord::default()
        }],
        ..Snapshot::default()
    }
}

#[test]
fn render_dashboard_uses_a_codex_style_shell_layout() {
    let output = render_dashboard(
        &sample_snapshot(),
        DashboardOptions {
            width: 100,
            workspace_path: "/workspace".to_string(),
            selected_model_id: Some("claude-sonnet-4.5".to_string()),
            context_left_label: "100% left".to_string(),
            approval_mode_label: "manual approvals".to_string(),
            thread_duration_label: "0s thread".to_string(),
            ..DashboardOptions::default()
        },
    );

    for needle in [
        ">_ Vorker (v0.1.0)",
        "model:     claude-sonnet-4.5   /model to change",
        "directory: /workspace",
        "Tip: Use /model or /new.",
        "• Model changed to claude-sonnet-4.5",
        "› hello",
        "• Hello. What do you need help with?",
        "› Improve documentation in @filename",
        "claude-sonnet-4.5 · 100% left · /workspace · manual approvals · 0s thread",
    ] {
        assert!(
            output.contains(needle),
            "missing {needle} in output:\n{output}"
        );
    }

    for removed in [
        "Navigation",
        "Conversation",
        "Composer",
        "Chats",
        "Agents",
        "Runs",
        "Tasks",
        "RUN BOARD",
    ] {
        assert!(
            !output.contains(removed),
            "shell should not render {removed}:\n{output}"
        );
    }
}

#[test]
fn render_dashboard_footer_shows_dense_status_when_space_allows() {
    let output = render_dashboard(
        &sample_snapshot(),
        DashboardOptions {
            width: 150,
            workspace_path: "/workspace".to_string(),
            selected_model_id: Some("claude-sonnet-4.5".to_string()),
            context_left_label: "100% left".to_string(),
            approval_mode_label: "manual approvals".to_string(),
            thread_duration_label: "0s thread".to_string(),
            ..DashboardOptions::default()
        },
    );

    assert!(
        output.contains("queue 0"),
        "missing queue status:\n{output}"
    );
    assert!(
        output.contains("idle"),
        "missing activity status:\n{output}"
    );
    assert!(
        output.contains("t:default"),
        "missing theme status:\n{output}"
    );
}

#[test]
fn render_dashboard_shows_working_rows_and_inline_slash_popup() {
    let output = render_dashboard(
        &Snapshot::default(),
        DashboardOptions {
            width: 100,
            workspace_path: "/workspace".to_string(),
            selected_model_id: Some("claude-sonnet-4.5".to_string()),
            context_left_label: "100% left".to_string(),
            approval_mode_label: "manual approvals".to_string(),
            thread_duration_label: "4s thread".to_string(),
            command_buffer: "/".to_string(),
            slash_menu_selected_index: 0,
            working_seconds: Some(4),
            ..DashboardOptions::default()
        },
    );

    assert!(
        output.contains("◦ Working (4s • enter to queue/steer • /stop to interrupt)"),
        "missing working row:\n{output}"
    );
    assert!(output.contains("› /"), "missing composer text:\n{output}");
    assert!(
        output.contains("/model   choose what model to use"),
        "missing slash popup:\n{output}"
    );
    assert!(
        output.contains("/stop   stop the active prompt or review job"),
        "missing busy-safe /stop command in slash popup:\n{output}"
    );
}

#[test]
fn busy_shell_popup_prefers_commands_that_can_run_while_busy() {
    let output = render_dashboard(
        &Snapshot::default(),
        DashboardOptions {
            width: 100,
            workspace_path: "/workspace".to_string(),
            selected_model_id: Some("claude-sonnet-4.5".to_string()),
            context_left_label: "100% left".to_string(),
            approval_mode_label: "manual approvals".to_string(),
            thread_duration_label: "4s thread".to_string(),
            command_buffer: "/".to_string(),
            slash_menu_selected_index: 0,
            working_seconds: Some(4),
            ..DashboardOptions::default()
        },
    );

    assert!(output.contains("/stop"), "missing /stop:\n{output}");
    assert!(output.contains("/steer"), "missing /steer:\n{output}");
    assert!(output.contains("/queue"), "missing /queue:\n{output}");
    assert!(
        !output.contains("/new   start a fresh chat"),
        "busy popup should hide commands that cannot run while busy:\n{output}"
    );
}

#[test]
fn slash_popup_filters_by_command_aliases() {
    let mut options = DashboardOptions {
        command_buffer: "/cl".to_string(),
        ..DashboardOptions::default()
    };

    let output = render_dashboard(&vorker_core::Snapshot::default(), options.clone());
    assert!(
        output.contains("/stop") && output.contains("stop the active prompt"),
        "alias /clean should surface /stop in popup:\n{output}"
    );

    options.command_buffer = "/appr".to_string();
    let output = render_dashboard(&vorker_core::Snapshot::default(), options);
    assert!(
        output.contains("/permissions"),
        "alias /approvals should surface /permissions in popup:\n{output}"
    );
}

#[test]
fn render_dashboard_colors_the_composer_surface() {
    let output = render_dashboard(
        &Snapshot::default(),
        DashboardOptions {
            width: 100,
            workspace_path: "/workspace".to_string(),
            selected_model_id: Some("claude-sonnet-4.5".to_string()),
            context_left_label: "100% left".to_string(),
            approval_mode_label: "manual approvals".to_string(),
            thread_duration_label: "0s thread".to_string(),
            color: true,
            ..DashboardOptions::default()
        },
    );

    assert!(
        output.contains("\u{1b}[48;5;238m"),
        "missing grey composer background:\n{output:?}"
    );
}

#[test]
fn render_dashboard_supports_a_review_theme() {
    let output = render_dashboard(
        &Snapshot::default(),
        DashboardOptions {
            width: 100,
            workspace_path: "/workspace".to_string(),
            selected_model_id: Some("gpt-5.3-codex".to_string()),
            theme_name: "review".to_string(),
            context_left_label: "100% left".to_string(),
            approval_mode_label: "manual approvals".to_string(),
            thread_duration_label: "0s thread".to_string(),
            color: true,
            ..DashboardOptions::default()
        },
    );

    assert!(
        output.contains("\u{1b}[95m") || output.contains("\u{1b}[48;5;237m"),
        "missing review accents:\n{output:?}"
    );
}

#[test]
fn review_theme_uses_a_short_review_command_list() {
    let output = render_dashboard(
        &Snapshot::default(),
        DashboardOptions {
            width: 100,
            workspace_path: "/workspace".to_string(),
            selected_model_id: Some("gpt-5.3-codex".to_string()),
            theme_name: "review".to_string(),
            command_buffer: "/".to_string(),
            tip_line: Some("Tip: Use /model, /coach, or /apply.".to_string()),
            color: false,
            ..DashboardOptions::default()
        },
    );

    assert!(output.contains("/model"));
    assert!(output.contains("/coach"));
    assert!(output.contains("/apply"));
    assert!(output.contains("/exit-review"));
    assert!(output.contains("Tip: Use /model, /coach, or /apply."));
    assert!(!output.contains("/new   start a fresh chat"));
    assert!(!output.contains("/list   list or reopen saved threads"));
    assert!(!output.contains("/cd   change the project directory"));
}

#[test]
fn review_theme_highlights_findings_paths_and_code_quotes() {
    let output = render_dashboard(
        &Snapshot::default(),
        DashboardOptions {
            width: 120,
            workspace_path: "/workspace".to_string(),
            selected_model_id: Some("gpt-5.3-codex".to_string()),
            theme_name: "review".to_string(),
            transcript_rows: vec![vorker_tui::TranscriptRow {
                kind: vorker_tui::RowKind::Tool,
                text: "[HIGH] Failure path lies".to_string(),
                detail: Some(
                    "Location: `pod_api.py`:34-35\nConfidence: 0.99\n\nRecommendation: return `ok: false`\n\n  34 | return {\"ok\": true}\n+ return {\"ok\": false}\n- return {\"ok\": true}".to_string(),
                ),
            }],
            color: true,
            ..DashboardOptions::default()
        },
    );

    assert!(
        output.contains("\u{1b}[1;48;5;130;97m[HIGH]\u{1b}[0m"),
        "missing severity badge:\n{output:?}"
    );
    assert!(
        output.contains("\u{1b}[48;5;238m") && output.contains("`pod_api.py`\u{1b}[0m"),
        "missing highlighted path:\n{output:?}"
    );
    assert!(
        output.contains("\u{1b}[90m  34 |\u{1b}[0m"),
        "missing dimmed code line number:\n{output:?}"
    );
    assert!(
        output.contains("\u{1b}[32m+ return {\"ok\": false}\u{1b}[0m"),
        "missing addition highlighting:\n{output:?}"
    );
    assert!(
        output.contains("\u{1b}[31m- return {\"ok\": true}\u{1b}[0m"),
        "missing removal highlighting:\n{output:?}"
    );
}
