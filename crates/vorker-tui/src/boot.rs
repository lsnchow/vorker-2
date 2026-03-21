use crate::theme::{TITLE_ART, colorize, emphasize, fit};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootStep {
    pub id: String,
    pub label: String,
    pub status: String,
    pub detail: String,
}

impl BootStep {
    #[must_use]
    pub fn new(id: &str, label: &str, status: &str, detail: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            status: status.to_string(),
            detail: detail.to_string(),
        }
    }
}

const SPINNER: [&str; 4] = ["|", "/", "-", "\\"];

fn meter(status: &str, tick: usize, color: bool) -> String {
    match status {
        "ready" => colorize("[####]", "brightGreen", color),
        "loading" => {
            let fill = (tick % 4) + 1;
            format!("[{}{}]", "#".repeat(fill), ".".repeat(4 - fill))
        }
        "error" => colorize("[!!!!]", "red", color),
        _ => colorize("[....]", "brightBlack", color),
    }
}

pub fn render_boot_frame(
    width: usize,
    tick: usize,
    active_step_id: Option<&str>,
    steps: &[BootStep],
    color: bool,
) -> String {
    let width = width.clamp(72, 120);
    let mut lines = TITLE_ART
        .iter()
        .map(|line| colorize(line, "brightGreen", color))
        .collect::<Vec<_>>();
    lines.push(emphasize(
        &colorize(
            "VORKER CONTROL PLANE // VORKER-2 supervisor mesh",
            "green",
            color,
        ),
        color,
    ));
    lines.push(colorize(
        "Arrow-led operator shell. Booting agent lanes and replaying the supervisor bus.",
        "gray",
        color,
    ));
    lines.push("-".repeat(width.max(40)));

    for step in steps {
        let live_status = if active_step_id == Some(step.id.as_str()) {
            "loading"
        } else {
            step.status.as_str()
        };
        let spinner = if live_status == "loading" {
            format!(" {}", SPINNER[tick % SPINNER.len()])
        } else {
            String::new()
        };
        let status_label = format!("{}{}", live_status.to_uppercase(), spinner);
        let line = format!(
            "{} {:<14} {} {}",
            meter(live_status, tick, color),
            step.label,
            step.detail,
            status_label
        );
        lines.push(fit(&line, width));
    }

    lines.join("\n")
}
