use super::*;
use std::process::Stdio;

pub(crate) fn current_shell_review_scope() -> Option<String> {
    std::env::var("VORKER_REVIEW_SCOPE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn spawn_side_agent(
    cwd: &Path,
    prompt_text: &str,
    store: &mut SideAgentStore,
    agents_dir: &Path,
) -> io::Result<SideAgentJob> {
    let model = current_review_model();
    let record = store.create_job_in_dir(cwd, prompt_text, &model, agents_dir)?;
    let output_path = PathBuf::from(&record.output_path);
    let stderr_path = PathBuf::from(&record.stderr_path);
    let events_path = PathBuf::from(&record.events_path);
    let events = std::fs::File::create(&events_path)?;
    let stderr = std::fs::File::create(&stderr_path)?;
    let mut command = std::process::Command::new("codex");
    command
        .arg("exec")
        .arg("--model")
        .arg(model)
        .arg("--full-auto")
        .arg("--color")
        .arg("never")
        .arg("--json")
        .arg("--skip-git-repo-check")
        .arg("--output-last-message")
        .arg(&output_path)
        .arg("-C")
        .arg(cwd)
        .arg(prompt_text)
        .stdout(Stdio::from(events))
        .stderr(Stdio::from(stderr));

    match command.spawn() {
        Ok(child) => Ok(SideAgentJob {
            id: record.id,
            display_name: record.display_name,
            child,
            output_path,
            stderr_path,
            completed: false,
        }),
        Err(error) => {
            let _ = store.mark_finished(&record.id, SideAgentStatus::Failed);
            Err(error)
        }
    }
}

pub(crate) fn poll_side_agent_jobs(
    app: &mut App,
    jobs: &mut [SideAgentJob],
    store: &mut SideAgentStore,
) -> io::Result<()> {
    for job in jobs.iter_mut().filter(|job| !job.completed) {
        if let Some(status) = job.child.try_wait()? {
            job.completed = true;
            let stored_status = if status.success() {
                SideAgentStatus::Completed
            } else {
                SideAgentStatus::Failed
            };
            store.mark_finished(&job.id, stored_status)?;
            if status.success() {
                app.apply_system_notice(format!("Side agent {} finished with {}.", job.id, status));
            } else {
                let detail = std::fs::read_to_string(&job.stderr_path)
                    .ok()
                    .map(|text| text.trim().to_string())
                    .filter(|text| !text.is_empty())
                    .unwrap_or_else(|| status.to_string());
                app.apply_system_notice(format!("Side agent {} failed: {detail}", job.id));
            }
        }
    }
    Ok(())
}

#[must_use]
pub fn load_bootstrap_snapshot() -> Snapshot {
    Snapshot::default()
}

pub(crate) fn should_redraw_frame(previous: &str, next: &str) -> bool {
    previous != next
}

pub(crate) fn load_timeline_text(
    session_event_store: &SessionEventStore,
    thread: &StoredThread,
) -> io::Result<String> {
    let events = session_event_store.events(&thread.id)?;
    if events.is_empty() {
        Ok(render_thread_timeline(thread))
    } else {
        Ok(render_session_event_timeline_with_mode(
            &thread.name,
            &events,
            "full",
            None,
            None,
        ))
    }
}

pub(crate) fn load_timeline_text_with_mode(
    session_event_store: &SessionEventStore,
    thread: &StoredThread,
    mode: &str,
    filter: Option<&str>,
    limit: Option<usize>,
) -> io::Result<String> {
    let events = session_event_store.events(&thread.id)?;
    if events.is_empty() {
        Ok(render_thread_timeline_with_mode(
            thread, mode, filter, limit,
        ))
    } else {
        Ok(render_session_event_timeline_with_mode(
            &thread.name,
            &events,
            mode,
            filter,
            limit,
        ))
    }
}

pub(crate) fn summarize_transcript_rows(rows: &[TranscriptRow]) -> String {
    let mut lines = vec![format!("Compacted {} row(s).", rows.len())];
    for (index, row) in rows.iter().take(8).enumerate() {
        let kind = match row.kind {
            RowKind::System => "system",
            RowKind::User => "user",
            RowKind::Assistant => "assistant",
            RowKind::Tool => "tool",
        };
        let summary = row
            .text
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .chars()
            .take(100)
            .collect::<String>();
        lines.push(format!("{}. [{}] {}", index + 1, kind, summary));
    }
    if rows.len() > 8 {
        lines.push(format!("… {} more row(s) omitted", rows.len() - 8));
    }
    lines.join("\n")
}

pub(crate) fn current_shell_theme() -> &'static str {
    match std::env::var("VORKER_THEME")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "review" => "review",
        _ => "default",
    }
}

pub(crate) fn current_review_mode() -> bool {
    matches!(
        std::env::var("VORKER_REVIEW_MODE")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "review"
    )
}
