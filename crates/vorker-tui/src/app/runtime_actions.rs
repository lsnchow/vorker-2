use super::*;

pub(crate) fn handle_local_session_action(
    runtime: &tokio::runtime::Runtime,
    bridge: &mut AcpBridge,
    app: &mut App,
    prompt_history_store: &mut PromptHistoryStore,
    skill_store: &mut crate::SkillStore,
    cwd: &Path,
    pending_permission_reply: &mut Option<tokio::sync::oneshot::Sender<Option<String>>>,
    action: AppCommand,
) -> io::Result<bool> {
    match action {
        AppCommand::ListPromptHistory => {
            let recent = prompt_history_store.recent(10);
            if recent.is_empty() {
                app.apply_system_notice("No prompt history yet.");
            } else {
                app.apply_system_notice("Prompt history:");
                for entry in recent {
                    app.apply_system_notice(format!("- {}", entry.text));
                }
            }
            Ok(false)
        }
        AppCommand::ListSkills => {
            apply_skill_listing(app);
            Ok(false)
        }
        AppCommand::SetSkillEnabled { name, enabled } => {
            let Some(skill_name) = resolve_skill_name(&app.skills, &name) else {
                app.apply_system_notice(format!("Unknown skill: {name}"));
                return Ok(false);
            };
            skill_store.set_enabled(&skill_name, enabled)?;
            refresh_skill_state(app, cwd, skill_store)?;
            app.apply_system_notice(format!(
                "{} skill {}.",
                if enabled { "Enabled" } else { "Disabled" },
                skill_name
            ));
            Ok(false)
        }
        AppCommand::SetModel { model } => {
            runtime.block_on(bridge.set_model(model))?;
            Ok(false)
        }
        AppCommand::SubmitPrompt {
            display_text,
            prompt_text,
        } => {
            prompt_history_store.append(display_text.clone())?;
            app.record_prompt_history(display_text);
            runtime.block_on(bridge.prompt(prompt_text))?;
            Ok(false)
        }
        AppCommand::CancelPrompt => {
            let _ = runtime.block_on(bridge.cancel());
            Ok(false)
        }
        AppCommand::ResolvePermission { option_id } => {
            if let Some(reply) = pending_permission_reply.take() {
                let _ = reply.send(option_id);
            }
            Ok(false)
        }
        AppCommand::ExitShell => Ok(true),
        _ => Ok(false),
    }
}

pub(crate) fn handle_workflow_action(
    runtime: &tokio::runtime::Runtime,
    bridge: &mut AcpBridge,
    app: &mut App,
    review_job: &mut Option<ReviewJob>,
    side_agent_store: &mut SideAgentStore,
    side_agent_jobs: &mut Vec<SideAgentJob>,
    action: AppCommand,
) -> io::Result<()> {
    match action {
        AppCommand::Stop => {
            let _ = runtime.block_on(bridge.cancel());
            if let Some(job) = review_job.as_mut() {
                let _ = job.child.kill();
            }
            let mut stopped_agents = 0usize;
            for job in side_agent_jobs.iter_mut().filter(|job| !job.completed) {
                let _ = job.child.kill();
                job.completed = true;
                let _ = side_agent_store.mark_finished(&job.id, SideAgentStatus::Stopped);
                stopped_agents += 1;
            }
            *review_job = None;
            app.stop_working_timer();
            let queued = app.queued_prompt_count();
            app.apply_system_notice(format!(
                "Stopped active work. {stopped_agents} side agent(s) stopped; {queued} queued prompt(s) remain."
            ));
        }
        AppCommand::SteerPrompt { prompt_text } => {
            runtime.block_on(bridge.prompt(prompt_text))?;
            app.apply_system_notice("Sent steering guidance.");
        }
        AppCommand::QueuePrompt {
            display_text,
            prompt_text,
        } => {
            app.queue_prompt(display_text, prompt_text);
        }
        AppCommand::ListQueuedPrompts => {
            let queued = app.queued_prompts();
            if queued.is_empty() {
                app.apply_system_notice("Queue is empty.");
            } else {
                app.apply_system_notice(format!("Queued prompts ({})", queued.len()));
                for (index, prompt) in queued.iter().enumerate() {
                    app.apply_system_notice(format!("{}. {}", index + 1, prompt));
                }
            }
        }
        AppCommand::PopQueuedPrompt => {
            if let Some(prompt) = app.pop_queued_prompt() {
                app.apply_system_notice(format!("Removed queued prompt: {prompt}"));
            } else {
                app.apply_system_notice("Queue is empty.");
            }
        }
        AppCommand::ClearQueuedPrompts => {
            let cleared = app.clear_queued_prompts();
            app.apply_system_notice(format!("Cleared {cleared} queued prompt(s)."));
        }
        _ => {}
    }

    Ok(())
}

pub(crate) fn handle_review_runtime_action(
    app: &mut App,
    cwd: &Path,
    review_job: &mut Option<ReviewJob>,
    action: AppCommand,
) -> io::Result<()> {
    match action {
        AppCommand::RunReview {
            focus,
            coach,
            apply,
            popout,
            scope,
        } => {
            if popout {
                let review_model = current_review_model();
                open_review_window(cwd, &review_model, scope.clone(), coach, apply, &focus)?;
                app.apply_system_notice(
                    "Adversarial review started in the review window. Use Esc there to exit review mode."
                        .to_string(),
                );
            } else if review_job.is_some() {
                app.apply_system_notice("A review is already running in this shell.");
            } else {
                app.apply_system_notice(format!(
                    "Running adversarial review{}{}.",
                    if coach { " with coaching" } else { "" },
                    if apply { " and patch follow-up" } else { "" },
                ));
                *review_job = Some(spawn_review_job(
                    cwd,
                    current_review_model(),
                    scope,
                    coach,
                    apply,
                    &focus,
                )?);
                app.working_started_at = Some(Instant::now());
                app.apply_tool_notice("Review job".to_string(), Some("queued".to_string()));
            }
        }
        AppCommand::RunRalph {
            task,
            model,
            no_deslop,
            xhigh,
        } => {
            let selected_model = model.or_else(|| app.selected_model_id().map(str::to_string));
            open_ralph_window(cwd, &task, selected_model.as_deref(), no_deslop, xhigh)?;
            app.apply_system_notice(format!("RALPH started in a new terminal: {task}"));
        }
        AppCommand::SetTheme { theme } => {
            let normalized = match theme.trim().to_ascii_lowercase().as_str() {
                "default" | "green" => "default",
                "review" | "purple" => "review",
                "opencode" | "oc" => "opencode",
                other => {
                    app.apply_system_notice(format!("Unknown theme: {other}"));
                    return Ok(());
                }
            };
            app.shell_theme = normalized.to_string();
            app.apply_system_notice(format!("Theme changed to {normalized}."));
        }
        _ => {}
    }

    Ok(())
}

pub(crate) fn handle_transcript_runtime_action(
    app: &mut App,
    cwd: &Path,
    workspace: &ProjectWorkspace,
    session_event_store: &SessionEventStore,
    side_agent_store: &SideAgentStore,
    action: AppCommand,
) -> io::Result<()> {
    match action {
        AppCommand::ExportTranscript { mode } => {
            let thread = app.thread_record();
            let events = session_event_store.events(&thread.id)?;
            match write_transcript_export(
                &workspace.project_dir().join("exports"),
                &thread,
                Some(&events),
                &mode,
            ) {
                Ok(path) => {
                    app.apply_system_notice(format!("Transcript exported to {}", path.display()))
                }
                Err(error) => app.apply_system_notice(format!("Export failed: {error}")),
            }
        }
        AppCommand::CopyTranscriptMode { mode } => {
            let thread = app.thread_record();
            let events = session_event_store.events(&thread.id)?;
            let markdown = match mode.trim().to_ascii_lowercase().as_str() {
                "events" => crate::render_transcript_markdown_from_events(&thread, &events),
                "rows" => crate::render_transcript_markdown(&thread),
                "brief" => {
                    if events.is_empty() {
                        crate::render_transcript_markdown_with_options(&thread, false, false)
                    } else {
                        crate::render_transcript_markdown_from_events_with_options(
                            &thread, &events, false, false,
                        )
                    }
                }
                _ => {
                    if events.is_empty() {
                        crate::render_transcript_markdown(&thread)
                    } else {
                        crate::render_transcript_markdown_from_events(&thread, &events)
                    }
                }
            };
            match copy_to_clipboard(&markdown) {
                Ok(()) => app.apply_system_notice("Transcript copied to clipboard."),
                Err(error) => app.apply_system_notice(format!("Copy failed: {error}")),
            }
        }
        AppCommand::CopyDiff => match render_working_tree_diff(cwd, 160) {
            Ok(diff) => match copy_to_clipboard(&diff) {
                Ok(()) => app.apply_system_notice("Diff copied to clipboard."),
                Err(error) => app.apply_system_notice(format!("Copy failed: {error}")),
            },
            Err(error) => app.apply_system_notice(format!("Copy failed: {error}")),
        },
        AppCommand::CopyStatus => {
            let jobs = side_agent_store.list_jobs();
            let running_agents = jobs
                .iter()
                .filter(|job| job.status == SideAgentStatus::Running)
                .count();
            let running_agent_names = jobs
                .iter()
                .filter(|job| job.status == SideAgentStatus::Running)
                .map(|job| job.display_name.clone())
                .collect::<Vec<_>>();
            let event_count = session_event_store
                .events(&app.thread_record().id)
                .map(|events| events.len())
                .unwrap_or(0);
            let status = render_status_summary(
                app.selected_model_id().unwrap_or("detecting..."),
                &cwd.display().to_string(),
                &workspace.project_dir().display().to_string(),
                app.approval_mode().label(),
                app.thread_name(),
                &format_thread_duration(app.thread_duration_seconds()),
                app.rows.len(),
                event_count,
                app.queued_prompt_count(),
                jobs.len(),
                running_agents,
                &running_agent_names,
            );
            match copy_to_clipboard(&status) {
                Ok(()) => app.apply_system_notice("Status copied to clipboard."),
                Err(error) => app.apply_system_notice(format!("Copy failed: {error}")),
            }
        }
        AppCommand::CopyTimeline => {
            let timeline = load_timeline_text(session_event_store, &app.thread_record())?;
            match copy_to_clipboard(&timeline) {
                Ok(()) => app.apply_system_notice("Timeline copied to clipboard."),
                Err(error) => app.apply_system_notice(format!("Copy failed: {error}")),
            }
        }
        AppCommand::ShowDiff => match render_working_tree_diff(cwd, 160) {
            Ok(diff) => app.apply_assistant_text(&diff),
            Err(error) => app.apply_system_notice(format!("Diff failed: {error}")),
        },
        AppCommand::ShowStagedDiff => match render_staged_diff(cwd, 160) {
            Ok(diff) => app.apply_assistant_text(&diff),
            Err(error) => app.apply_system_notice(format!("Diff failed: {error}")),
        },
        AppCommand::CompactTranscript => app.compact_transcript(),
        AppCommand::ShowTimeline => {
            let timeline = load_timeline_text(session_event_store, &app.thread_record())?;
            app.apply_assistant_text(&timeline);
        }
        AppCommand::ShowTimelineMode {
            mode,
            filter,
            limit,
        } => {
            let timeline = load_timeline_text_with_mode(
                session_event_store,
                &app.thread_record(),
                &mode,
                filter.as_deref(),
                limit,
            )?;
            app.apply_assistant_text(&timeline);
        }
        AppCommand::ShowStatus => {
            let jobs = side_agent_store.list_jobs();
            let running_agents = jobs
                .iter()
                .filter(|job| job.status == SideAgentStatus::Running)
                .count();
            let running_agent_names = jobs
                .iter()
                .filter(|job| job.status == SideAgentStatus::Running)
                .map(|job| job.display_name.clone())
                .collect::<Vec<_>>();
            let event_count = session_event_store
                .events(&app.thread_record().id)
                .map(|events| events.len())
                .unwrap_or(0);
            app.apply_system_notice(render_status_summary(
                app.selected_model_id().unwrap_or("detecting..."),
                &cwd.display().to_string(),
                &workspace.project_dir().display().to_string(),
                app.approval_mode().label(),
                app.thread_name(),
                &format_thread_duration(app.thread_duration_seconds()),
                app.rows.len(),
                event_count,
                app.queued_prompt_count(),
                jobs.len(),
                running_agents,
                &running_agent_names,
            ));
        }
        _ => {}
    }

    Ok(())
}
