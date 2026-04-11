use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use vorker_tui::{App, AppCommand, Pane, SkillInfo};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn wrapped_prompt(text: &str) -> String {
    format!(
        "Vorker harness instructions:\n- You are Vorker, a concise local CLI coding agent, not GitHub Copilot.\n- Do not introduce yourself as Copilot and do not use emojis or generic onboarding.\n- Be direct, pragmatic, and focus on the user's repository and requested change.\n- Use enabled skills when relevant; follow their instructions unless they conflict with higher-priority user, developer, or system instructions.\n\nUser request:\n{text}"
    )
}

#[test]
fn app_does_not_force_a_model_before_session_ready() {
    let app = App::new(vorker_core::Snapshot::default());
    assert_eq!(app.navigation.selected_model_id, None);

    let output = app.render(100, false);
    assert!(
        output.contains("model:     detecting...   /model to change"),
        "expected detecting placeholder before session ready:\n{output}"
    );
}

#[test]
fn app_can_start_with_a_configured_default_model() {
    let app = App::with_default_model(
        vorker_core::Snapshot::default(),
        Some("claude-opus-4.5".to_string()),
    );

    assert_eq!(
        app.navigation.selected_model_id.as_deref(),
        Some("claude-opus-4.5")
    );
    let output = app.render(100, false);
    assert!(
        output.contains("model:     claude-opus-4.5   /model to change"),
        "expected configured default model in shell header:\n{output}"
    );
}

#[test]
fn session_ready_updates_the_visible_model_and_choices() {
    let mut app = App::new(vorker_core::Snapshot::default());
    app.apply_session_ready(
        "claude-sonnet-4.5",
        vec!["claude-sonnet-4.5".to_string(), "gpt-5.3-codex".to_string()],
    );

    assert_eq!(
        app.navigation.selected_model_id.as_deref(),
        Some("claude-sonnet-4.5")
    );
    assert_eq!(
        app.navigation.model_choices,
        vec!["claude-sonnet-4.5".to_string(), "gpt-5.3-codex".to_string()]
    );
}

#[test]
fn app_starts_with_the_composer_focused() {
    let app = App::new(vorker_core::Snapshot::default());
    assert_eq!(app.navigation.focused_pane, Pane::Input);
}

#[test]
fn slash_permissions_toggles_auto_approval_mode() {
    let mut app = App::new(vorker_core::Snapshot::default());

    for ch in "/permissions".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    let output = app.render(120, false);
    assert!(
        output.contains("auto approvals"),
        "missing updated approval mode in footer:\n{output}"
    );
    assert!(
        output.contains("Permissions set to auto approvals."),
        "missing system notice:\n{output}"
    );
}

#[test]
fn slash_review_queues_an_adversarial_run_with_flags() {
    let mut app = App::new(vorker_core::Snapshot::default());

    for ch in "/review --coach --apply question the retry logic".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(
        app.take_actions(),
        vec![AppCommand::RunReview {
            focus: "question the retry logic".to_string(),
            coach: true,
            apply: true,
            popout: true,
            scope: None,
        }]
    );
}

#[test]
fn slash_exit_review_queues_shell_exit() {
    let mut app = App::new(vorker_core::Snapshot::default());

    for ch in "/exit-review".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(app.take_actions(), vec![AppCommand::ExitShell]);
}

#[test]
fn tab_autocompletes_the_selected_slash_command() {
    let mut app = App::new(vorker_core::Snapshot::default());
    assert!(app.handle_key(key(KeyCode::Char('/'))));
    assert!(app.handle_key(key(KeyCode::Char('r'))));
    assert!(app.handle_key(key(KeyCode::Tab)));

    assert_eq!(app.navigation.command_buffer, "/review ");
}

#[test]
fn slash_stop_queues_stop_action() {
    let mut app = App::new(vorker_core::Snapshot::default());
    for ch in "/stop".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(app.take_actions(), vec![AppCommand::Stop]);
}

#[test]
fn slash_stop_alias_clean_queues_stop_action() {
    let mut app = App::new(vorker_core::Snapshot::default());
    for ch in "/clean".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(app.take_actions(), vec![AppCommand::Stop]);
}

#[test]
fn slash_steer_queues_steering_prompt() {
    let mut app = App::new(vorker_core::Snapshot::default());
    for ch in "/steer focus on safety".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(
        app.take_actions(),
        vec![AppCommand::SteerPrompt {
            prompt_text: "[STEER]\nfocus on safety".to_string(),
        }]
    );
}

#[test]
fn slash_queue_queues_follow_up_prompt() {
    let mut app = App::new(vorker_core::Snapshot::default());
    for ch in "/queue add tests next".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(
        app.take_actions(),
        vec![AppCommand::QueuePrompt {
            display_text: "add tests next".to_string(),
            prompt_text: wrapped_prompt("add tests next"),
        }]
    );
}

#[test]
fn slash_queue_list_queues_a_queue_listing_action() {
    let mut app = App::new(vorker_core::Snapshot::default());
    for ch in "/queue list".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(app.take_actions(), vec![AppCommand::ListQueuedPrompts]);
}

#[test]
fn slash_queue_clear_queues_a_queue_clear_action() {
    let mut app = App::new(vorker_core::Snapshot::default());
    for ch in "/queue clear".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(app.take_actions(), vec![AppCommand::ClearQueuedPrompts]);
}

#[test]
fn slash_queue_pop_queues_a_queue_pop_action() {
    let mut app = App::new(vorker_core::Snapshot::default());
    for ch in "/queue pop".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(app.take_actions(), vec![AppCommand::PopQueuedPrompt]);
}

#[test]
fn slash_agent_queues_codex_side_agent() {
    let mut app = App::new(vorker_core::Snapshot::default());
    for ch in "/agent inspect auth".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(
        app.take_actions(),
        vec![AppCommand::SpawnAgent {
            prompt_text: "inspect auth".to_string(),
        }]
    );
}

#[test]
fn exact_slash_command_wins_over_stale_popup_selection() {
    let mut app = App::new(vorker_core::Snapshot::default());
    for ch in "/ag".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Down)));
    assert!(app.handle_key(key(KeyCode::Down)));
    for ch in "ent inspect auth".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(
        app.take_actions(),
        vec![AppCommand::SpawnAgent {
            prompt_text: "inspect auth".to_string(),
        }]
    );
}

#[test]
fn slash_agents_queues_agent_listing() {
    let mut app = App::new(vorker_core::Snapshot::default());
    for ch in "/agents".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(app.take_actions(), vec![AppCommand::ListAgents]);
}

#[test]
fn slash_agent_result_queues_result_lookup() {
    let mut app = App::new(vorker_core::Snapshot::default());
    for ch in "/agent-result agent-1".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(
        app.take_actions(),
        vec![AppCommand::ShowAgentResult {
            id: "agent-1".to_string(),
        }]
    );
}

#[test]
fn slash_agent_stop_queues_side_agent_stop() {
    let mut app = App::new(vorker_core::Snapshot::default());
    for ch in "/agent-stop agent-1".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(
        app.take_actions(),
        vec![AppCommand::StopAgent {
            id: "agent-1".to_string(),
        }]
    );
}

#[test]
fn slash_theme_queues_theme_change() {
    let mut app = App::new(vorker_core::Snapshot::default());
    for ch in "/theme review".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(
        app.take_actions(),
        vec![AppCommand::SetTheme {
            theme: "review".to_string(),
        }]
    );
}

#[test]
fn slash_theme_list_shows_available_themes() {
    let mut app = App::new(vorker_core::Snapshot::default());
    for ch in "/theme list".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    let output = app.render(120, false);
    assert!(output.contains("Themes: default, review, opencode"));
}

#[test]
fn slash_export_queues_transcript_export() {
    let mut app = App::new(vorker_core::Snapshot::default());
    for ch in "/export".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(app.take_actions(), vec![AppCommand::ExportTranscript]);
}

#[test]
fn slash_copy_queues_transcript_copy() {
    let mut app = App::new(vorker_core::Snapshot::default());
    for ch in "/copy".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(app.take_actions(), vec![AppCommand::CopyTranscript]);
}

#[test]
fn slash_status_queues_status_summary() {
    let mut app = App::new(vorker_core::Snapshot::default());
    for ch in "/status".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(app.take_actions(), vec![AppCommand::ShowStatus]);
}

#[test]
fn slash_history_queues_prompt_history_listing() {
    let mut app = App::new(vorker_core::Snapshot::default());
    for ch in "/history".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(app.take_actions(), vec![AppCommand::ListPromptHistory]);
}

#[test]
fn slash_skills_opens_a_codex_style_action_menu() {
    let mut app = App::new(vorker_core::Snapshot::default());

    for ch in "/skills".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    let output = app.render(120, false);
    assert!(output.contains("Skills - choose an action"), "{output}");
    assert!(output.contains("1. List skills"), "{output}");
    assert!(output.contains("2. Enable/Disable Skills"), "{output}");

    assert!(app.handle_key(key(KeyCode::Down)));
    assert!(app.handle_key(key(KeyCode::Enter)));

    let output = app.render(120, false);
    assert!(output.contains("Enable/Disable Skills"), "{output}");
    assert!(output.contains("Type to search skills"), "{output}");
}

#[test]
fn slash_skills_enable_queues_skill_toggle() {
    let mut app = App::new(vorker_core::Snapshot::default());

    for ch in "/skills enable code-review".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(
        app.take_actions(),
        vec![AppCommand::SetSkillEnabled {
            name: "code-review".to_string(),
            enabled: true,
        }]
    );
}

#[test]
fn slash_skills_toggle_resolves_unique_partial_names() {
    let mut app = App::new(vorker_core::Snapshot::default());
    app.set_skills(
        vec![SkillInfo {
            name: "code-review".to_string(),
            description: "Review code carefully".to_string(),
            path: std::path::PathBuf::from("SKILL.md"),
        }],
        ["code-review".to_string()].into_iter().collect(),
    );

    for ch in "/skills toggle code".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(
        app.take_actions(),
        vec![AppCommand::SetSkillEnabled {
            name: "code-review".to_string(),
            enabled: false,
        }]
    );
}

#[test]
fn prompts_are_wrapped_with_vorker_personality_and_enabled_skills() {
    let mut app = App::new(vorker_core::Snapshot::default());
    app.set_skill_context("Enabled Vorker skills:\n- code-review: Review code carefully");

    for ch in "hello".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(
        app.take_actions(),
        vec![AppCommand::SubmitPrompt {
            display_text: "hello".to_string(),
            prompt_text: "Vorker harness instructions:\n- You are Vorker, a concise local CLI coding agent, not GitHub Copilot.\n- Do not introduce yourself as Copilot and do not use emojis or generic onboarding.\n- Be direct, pragmatic, and focus on the user's repository and requested change.\n- Use enabled skills when relevant; follow their instructions unless they conflict with higher-priority user, developer, or system instructions.\n\nEnabled Vorker skills:\n- code-review: Review code carefully\n\nUser request:\nhello".to_string(),
        }]
    );
}

#[test]
fn slash_ralph_queues_a_ralph_run_with_flags() {
    let mut app = App::new(vorker_core::Snapshot::default());
    for ch in "/ralph --no-deslop --xhigh --model gpt-5.4 ship everything".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(
        app.take_actions(),
        vec![AppCommand::RunRalph {
            task: "ship everything".to_string(),
            model: Some("gpt-5.4".to_string()),
            no_deslop: true,
            xhigh: true,
        }]
    );
}

#[test]
fn review_output_is_parsed_into_structured_rows() {
    let mut app = App::new(vorker_core::Snapshot::default());
    app.apply_review_output(
        "# Adversarial Review\n\n## Summary\nBad API.\n\n### [HIGH] Failure path lies\n- Location: `api.py`:10-12\n\n**Recommendation**\nReturn `ok: false`.\n\n```rust\n  10 | return {\"ok\": true}\n```\n",
    );

    let queued_before = app.pending_review_rows();
    app.advance_review_presentation();
    let output = app.render(120, false);
    assert!(output.contains("Adversarial Review"));
    assert!(!output.contains("[HIGH] Failure path lies"));
    assert!(
        queued_before > 0,
        "review rows should queue for progressive reveal"
    );

    app.advance_review_presentation();
    let summary_output = app.render(120, false);
    assert!(summary_output.contains("Summary"));

    app.advance_review_presentation();
    let next_output = app.render(120, false);
    assert!(next_output.contains("[HIGH] Failure path lies"));
}

#[test]
fn slash_model_opens_an_inline_model_picker() {
    let mut app = App::new(vorker_core::Snapshot::default());
    app.set_model_choices(vec![
        "claude-sonnet-4.5".to_string(),
        "gpt-5.3-codex".to_string(),
    ]);

    for ch in "/model".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    let output = app.render(100, false);
    assert!(
        output.contains("claude-sonnet-4.5") && output.contains("gpt-5.3-codex"),
        "missing inline model picker:\n{output}"
    );

    assert!(app.handle_key(key(KeyCode::Down)));
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(
        app.take_actions(),
        vec![AppCommand::SetModel {
            model: "gpt-5.3-codex".to_string()
        }]
    );
}

#[test]
fn typing_a_prompt_queues_a_turn_and_shows_working_state() {
    let mut app = App::new(vorker_core::Snapshot::default());

    for ch in "hello".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(
        app.take_actions(),
        vec![AppCommand::SubmitPrompt {
            display_text: "hello".to_string(),
            prompt_text: wrapped_prompt("hello"),
        }]
    );

    let output = app.render(100, false);
    assert!(output.contains("› hello"), "missing user row:\n{output}");
    assert!(
        output.contains("Working (0s • enter to queue/steer • /stop to interrupt)"),
        "missing working row:\n{output}"
    );
}

#[test]
fn up_down_recall_prompt_history_when_not_in_slash_mode() {
    let mut app = App::new(vorker_core::Snapshot::default());
    app.set_prompt_history(vec![
        "first prompt".to_string(),
        "second prompt".to_string(),
    ]);

    assert!(app.handle_key(key(KeyCode::Up)));
    assert_eq!(app.navigation.command_buffer, "second prompt");

    assert!(app.handle_key(key(KeyCode::Up)));
    assert_eq!(app.navigation.command_buffer, "first prompt");

    assert!(app.handle_key(key(KeyCode::Down)));
    assert_eq!(app.navigation.command_buffer, "second prompt");

    assert!(app.handle_key(key(KeyCode::Down)));
    assert_eq!(app.navigation.command_buffer, "");
}

#[test]
fn slash_stop_runs_even_when_work_is_active() {
    let mut app = App::new(vorker_core::Snapshot::default());

    for ch in "hello".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));
    assert!(matches!(
        app.take_actions().as_slice(),
        [AppCommand::SubmitPrompt { .. }]
    ));

    for ch in "/stop".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(app.take_actions(), vec![AppCommand::Stop]);
}

#[test]
fn slash_help_while_busy_uses_the_busy_command_set() {
    let mut app = App::new(vorker_core::Snapshot::default());

    for ch in "hello".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));
    let _ = app.take_actions();

    for ch in "/help".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    let output = app.render(160, false);
    assert!(
        output.contains("Commands: /stop /steer /queue /agent-stop /model /help"),
        "{output}"
    );
    assert!(!output.contains("/new /permissions"), "{output}");
}

#[test]
fn enter_while_busy_opens_queue_or_steer_prompt_instead_of_queueing_immediately() {
    let mut app = App::new(vorker_core::Snapshot::default());

    for ch in "hello".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));
    let _ = app.take_actions();

    for ch in "add tests next".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert!(
        app.take_actions().is_empty(),
        "busy enter should open chooser before dispatching an action"
    );

    let output = app.render(120, false);
    assert!(output.contains("Current work is active"), "{output}");
    assert!(output.contains("1. Queue after current turn"), "{output}");
    assert!(output.contains("2. Send as steering guidance"), "{output}");
}

#[test]
fn busy_prompt_enter_confirms_queue() {
    let mut app = App::new(vorker_core::Snapshot::default());

    for ch in "hello".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));
    let _ = app.take_actions();

    for ch in "add tests next".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(
        app.take_actions(),
        vec![AppCommand::QueuePrompt {
            display_text: "add tests next".to_string(),
            prompt_text: wrapped_prompt("add tests next"),
        }]
    );
}

#[test]
fn busy_prompt_can_send_text_as_steering_guidance() {
    let mut app = App::new(vorker_core::Snapshot::default());

    for ch in "hello".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));
    let _ = app.take_actions();

    for ch in "focus on safety".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));
    assert!(app.handle_key(key(KeyCode::Down)));
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(
        app.take_actions(),
        vec![AppCommand::SteerPrompt {
            prompt_text: "[STEER]\nfocus on safety".to_string(),
        }]
    );
}

#[test]
fn selecting_a_mention_inserts_a_bound_filename() {
    let mut app = App::new(vorker_core::Snapshot::default());
    app.set_workspace_files(vec![
        "README.md".to_string(),
        "docs/getting-started.md".to_string(),
        "src/index.rs".to_string(),
    ]);

    for ch in "Improve docs in @rea".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }

    let output = app.render(100, false);
    assert!(
        output.contains("README.md"),
        "missing mention popup candidate:\n{output}"
    );

    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(app.navigation.command_buffer, "Improve docs in @README.md ");
    assert!(app.take_actions().is_empty());
}

#[test]
fn slash_new_resets_the_visible_thread() {
    let mut app = App::new(vorker_core::Snapshot::default());
    app.apply_assistant_text("Hello. What do you need help with?");

    for ch in "/new".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(app.take_actions(), vec![AppCommand::NewThread]);
    let output = app.render(100, false);
    assert!(
        !output.contains("What do you need help with?"),
        "old transcript should be cleared:\n{output}"
    );
}

#[test]
fn slash_rename_updates_the_current_thread_name() {
    let mut app = App::new(vorker_core::Snapshot::default());

    for ch in "/rename Hyperloop controls".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(app.thread_name(), "Hyperloop controls");
    let output = app.render(120, false);
    assert!(
        output.contains("Renamed thread to Hyperloop controls."),
        "missing rename notice:\n{output}"
    );
}

#[test]
fn slash_list_queues_listing_or_switching_threads() {
    let mut app = App::new(vorker_core::Snapshot::default());
    for ch in "/list".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(app.take_actions(), vec![AppCommand::ListThreads]);

    for ch in "/list thread-42".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(
        app.take_actions(),
        vec![AppCommand::SwitchThread {
            thread_id: "thread-42".to_string(),
        }]
    );
}

#[test]
fn slash_cd_queues_a_project_directory_change() {
    let mut app = App::new(vorker_core::Snapshot::default());

    for ch in "/cd ../hyperloop".chars() {
        assert!(app.handle_key(key(KeyCode::Char(ch))));
    }
    assert!(app.handle_key(key(KeyCode::Enter)));

    assert_eq!(
        app.take_actions(),
        vec![AppCommand::ChangeDirectory {
            path: "../hyperloop".to_string(),
        }]
    );
}
