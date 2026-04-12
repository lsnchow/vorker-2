use super::*;

pub(crate) fn format_agent_result(
    id: &str,
    display_name: &str,
    events: &[String],
    output: &str,
) -> String {
    let mut sections = vec![format!("Agent {display_name} ({id}) result:")];
    if !events.is_empty() {
        sections.push("Events:".to_string());
        sections.extend(events.iter().map(|event| format!("- {event}")));
    }
    sections.push("Output:".to_string());
    sections.push(output.to_string());
    sections.join("\n")
}

pub(crate) fn format_agent_log(
    id: &str,
    display_name: &str,
    events: &[String],
    stderr: &str,
) -> String {
    let mut sections = vec![format!("Agent {display_name} ({id}) log:")];
    if events.is_empty() {
        sections.push("Events: none captured".to_string());
    } else {
        sections.push("Events:".to_string());
        sections.extend(events.iter().map(|event| format!("- {event}")));
    }

    let stderr = stderr.trim();
    if stderr.is_empty() {
        sections.push("stderr: empty".to_string());
    } else {
        sections.push("stderr:".to_string());
        sections.push(stderr.to_string());
    }

    sections.join("\n")
}

pub(crate) fn resolve_agent_identifier(
    requested: &str,
    live_jobs: &[SideAgentJob],
    store: &SideAgentStore,
) -> Option<String> {
    if live_jobs.iter().any(|job| job.id == requested) || store.job(requested).is_some() {
        return Some(requested.to_string());
    }

    let lower = requested.to_ascii_lowercase();
    let mut matches = live_jobs
        .iter()
        .map(|job| (job.id.clone(), job.display_name.clone()))
        .chain(
            store
                .list_jobs()
                .into_iter()
                .map(|job| (job.id, job.display_name)),
        )
        .filter(|(_, name)| name.to_ascii_lowercase() == lower)
        .map(|(id, _)| id)
        .collect::<Vec<_>>();
    matches.sort();
    matches.dedup();
    (matches.len() == 1).then(|| matches.remove(0))
}
