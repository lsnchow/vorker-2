use super::shell_helpers::{load_timeline_text, load_timeline_text_with_mode};
use super::*;
use crate::diff_reports::{copy_to_clipboard, render_staged_diff, render_working_tree_diff};

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
