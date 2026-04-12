use super::shell_helpers::spawn_side_agent;
use super::side_agent_helpers::{format_agent_log, format_agent_result, resolve_agent_identifier};
use super::*;

pub(crate) fn handle_side_agent_action(
    app: &mut App,
    cwd: &Path,
    workspace: &ProjectWorkspace,
    side_agent_store: &mut SideAgentStore,
    side_agent_jobs: &mut Vec<SideAgentJob>,
    action: AppCommand,
) -> io::Result<()> {
    match action {
        AppCommand::SpawnAgent { prompt_text } => {
            match spawn_side_agent(
                cwd,
                &prompt_text,
                side_agent_store,
                &workspace.side_agents_dir(),
            ) {
                Ok(job) => {
                    app.apply_system_notice(format!(
                        "Spawned Codex agent {} ({}).",
                        job.display_name, job.id
                    ));
                    side_agent_jobs.push(job);
                }
                Err(error) => app.apply_system_notice(format!("Failed to spawn agent: {error}")),
            }
        }
        AppCommand::ListAgents => {
            let jobs = side_agent_store.list_jobs();
            if jobs.is_empty() {
                app.apply_system_notice("No side agents in this session.");
            } else {
                app.apply_assistant_text(&render_agent_roster(&jobs));
            }
        }
        AppCommand::StopAgent { id } => {
            let resolved = resolve_agent_identifier(&id, side_agent_jobs, side_agent_store);
            if let Some(job) = side_agent_jobs
                .iter_mut()
                .find(|job| Some(job.id.as_str()) == resolved.as_deref())
            {
                if job.completed {
                    app.apply_system_notice(format!(
                        "Side agent {} ({}) is already finished.",
                        job.display_name, job.id
                    ));
                } else {
                    let _ = job.child.kill();
                    job.completed = true;
                    let _ = side_agent_store.mark_finished(&job.id, SideAgentStatus::Stopped);
                    app.apply_system_notice(format!(
                        "Stopped side agent {} ({}).",
                        job.display_name, job.id
                    ));
                }
            } else if let Some(agent_id) = resolved
                && let Some(job) = side_agent_store.job(&agent_id)
            {
                side_agent_store.mark_finished(&agent_id, SideAgentStatus::Stopped)?;
                app.apply_system_notice(format!(
                    "Marked stored side agent {} ({}) as stopped.",
                    job.display_name, agent_id
                ));
            } else {
                app.apply_system_notice(format!("Unknown agent id: {id}"));
            }
        }
        AppCommand::ShowAgentResult { id } => {
            let resolved = resolve_agent_identifier(&id, side_agent_jobs, side_agent_store);
            if let Some(job) = side_agent_jobs
                .iter()
                .find(|job| Some(job.id.as_str()) == resolved.as_deref())
            {
                let output = std::fs::read_to_string(&job.output_path)
                    .unwrap_or_else(|_| "No output captured yet.".to_string());
                let events = summarize_side_agent_events(
                    &PathBuf::from(
                        side_agent_store
                            .job(&id)
                            .map(|job| job.events_path)
                            .unwrap_or_default(),
                    ),
                    8,
                )
                .unwrap_or_default();
                app.apply_assistant_text(&format_agent_result(
                    &job.id,
                    &job.display_name,
                    &events,
                    &output,
                ));
            } else if let Some(agent_id) = resolved
                && let Some(job) = side_agent_store.job(&agent_id)
            {
                let output = std::fs::read_to_string(&job.output_path)
                    .unwrap_or_else(|_| "No output captured yet.".to_string());
                let events = summarize_side_agent_events(&PathBuf::from(&job.events_path), 8)
                    .unwrap_or_default();
                app.apply_assistant_text(&format_agent_result(
                    &agent_id,
                    &job.display_name,
                    &events,
                    &output,
                ));
            } else {
                app.apply_system_notice(format!("Unknown agent id: {id}"));
            }
        }
        AppCommand::ShowAgentLog { id } => {
            let resolved = resolve_agent_identifier(&id, side_agent_jobs, side_agent_store);
            if let Some(job) = side_agent_jobs
                .iter()
                .find(|job| Some(job.id.as_str()) == resolved.as_deref())
            {
                let stderr = std::fs::read_to_string(&job.stderr_path).unwrap_or_default();
                let events = summarize_side_agent_events(
                    &PathBuf::from(
                        side_agent_store
                            .job(&job.id)
                            .map(|stored| stored.events_path)
                            .unwrap_or_default(),
                    ),
                    16,
                )
                .unwrap_or_default();
                app.apply_assistant_text(&format_agent_log(
                    &job.id,
                    &job.display_name,
                    &events,
                    &stderr,
                ));
            } else if let Some(agent_id) = resolved
                && let Some(job) = side_agent_store.job(&agent_id)
            {
                let stderr = std::fs::read_to_string(&job.stderr_path).unwrap_or_default();
                let events = summarize_side_agent_events(&PathBuf::from(&job.events_path), 16)
                    .unwrap_or_default();
                app.apply_assistant_text(&format_agent_log(
                    &agent_id,
                    &job.display_name,
                    &events,
                    &stderr,
                ));
            } else {
                app.apply_system_notice(format!("Unknown agent id: {id}"));
            }
        }
        AppCommand::ResumeAgent { id } => {
            let resolved = resolve_agent_identifier(&id, side_agent_jobs, side_agent_store);
            if let Some(agent_id) = resolved
                && let Some(job) = side_agent_store.job(&agent_id)
            {
                match spawn_side_agent(
                    Path::new(&job.cwd),
                    &job.prompt,
                    side_agent_store,
                    &workspace.side_agents_dir(),
                ) {
                    Ok(new_job) => {
                        app.apply_system_notice(format!(
                            "Respawned Codex agent {} ({}) from {} ({}).",
                            new_job.display_name, new_job.id, job.display_name, agent_id
                        ));
                        side_agent_jobs.push(new_job);
                    }
                    Err(error) => {
                        app.apply_system_notice(format!("Failed to resume agent: {error}"));
                    }
                }
            } else {
                app.apply_system_notice(format!("Unknown agent id: {id}"));
            }
        }
        _ => {}
    }

    Ok(())
}
