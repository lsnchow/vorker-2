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
