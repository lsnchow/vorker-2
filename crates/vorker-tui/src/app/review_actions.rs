use super::*;

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
