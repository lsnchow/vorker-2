use super::*;
use std::process::Stdio;

pub(crate) fn current_review_model() -> String {
    std::env::var("VORKER_REVIEW_MODEL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "gpt-5.3-codex".to_string())
}

pub(crate) fn env_flag(name: &str) -> bool {
    matches!(
        std::env::var(name)
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes"
    )
}

pub(crate) fn spawn_review_job(
    cwd: &Path,
    model: String,
    scope: Option<String>,
    coach: bool,
    apply: bool,
    focus: &str,
) -> io::Result<ReviewJob> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let report_path = std::env::temp_dir().join(format!("vorker-review-{stamp}.md"));
    let status_path = std::env::temp_dir().join(format!("vorker-review-{stamp}.status"));
    let events_path = std::env::temp_dir().join(format!("vorker-review-{stamp}.events"));
    let stderr_path = std::env::temp_dir().join(format!("vorker-review-{stamp}.stderr"));
    let stderr_file = std::fs::File::create(&stderr_path)?;
    let mut command = std::process::Command::new(std::env::current_exe()?);
    command
        .arg("--cwd")
        .arg(cwd)
        .arg("--model")
        .arg(model)
        .arg("adversarial")
        .arg("--output-report")
        .arg(&report_path)
        .arg("--events-file")
        .arg(&events_path)
        .arg("--status-file")
        .arg(&status_path)
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr_file));
    if let Some(scope) = scope {
        command.arg("--scope").arg(scope);
    }
    if coach {
        command.arg("--coach");
    }
    if apply {
        command.arg("--apply");
    }
    if !focus.trim().is_empty() {
        command.arg(focus);
    }

    let child = command.spawn()?;
    Ok(ReviewJob {
        child,
        report_path,
        status_path,
        events_path,
        stderr_path,
        last_status: None,
        delivered_event_lines: 0,
        streamed_any_rows: false,
    })
}

pub(crate) fn poll_review_job(app: &mut App, review_job: &mut Option<ReviewJob>) -> io::Result<()> {
    let Some(job) = review_job.as_mut() else {
        return Ok(());
    };

    if let Ok(status) = std::fs::read_to_string(&job.status_path) {
        let status = status.trim().to_string();
        if !status.is_empty() && job.last_status.as_deref() != Some(status.as_str()) {
            app.apply_tool_update(status.clone());
            job.last_status = Some(status);
        }
    }

    if let Ok(events) = std::fs::read_to_string(&job.events_path) {
        let lines = events.lines().collect::<Vec<_>>();
        for line in lines.iter().skip(job.delivered_event_lines) {
            if let Ok(row) = serde_json::from_str::<TranscriptRow>(line) {
                app.pending_review_rows.push_back(row);
                job.streamed_any_rows = true;
            }
        }
        job.delivered_event_lines = lines.len();
    }

    if let Some(exit_status) = job.child.try_wait()? {
        app.finish_prompt();
        if exit_status.success() {
            if !job.streamed_any_rows {
                if let Ok(report) = std::fs::read_to_string(&job.report_path)
                    && !report.trim().is_empty()
                {
                    app.apply_review_output(&report);
                } else {
                    app.apply_system_notice("Review finished, but no report was written.");
                }
            } else {
                app.apply_system_notice("Review finished.");
            }
        } else {
            let error = std::fs::read_to_string(&job.stderr_path)
                .ok()
                .map(|text| text.trim().to_string())
                .filter(|text| !text.is_empty())
                .unwrap_or_else(|| "Adversarial review failed.".to_string());
            app.apply_system_notice(error);
        }
        *review_job = None;
    }

    Ok(())
}
