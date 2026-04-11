use serde::{Deserialize, Serialize};
use vorker_core::{Snapshot, TranscriptEntry};

use crate::rich_text::{RichContext, style_line};
use crate::slash::{category_label, filtered_commands_for_state};
use crate::theme::{colorize, fit, hard_wrap, highlight, truncate, visible_length};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RowKind {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranscriptRow {
    pub kind: RowKind,
    pub text: String,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PopupItem {
    pub label: String,
    pub description: Option<String>,
    pub selectable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DashboardOptions {
    pub color: bool,
    pub width: usize,
    pub theme_name: String,
    pub workspace_path: String,
    pub selected_model_id: Option<String>,
    pub model_choices: Vec<String>,
    pub model_picker_open: bool,
    pub command_buffer: String,
    pub slash_menu_selected_index: usize,
    pub mention_items: Vec<String>,
    pub mention_selected_index: usize,
    pub permission_title: Option<String>,
    pub permission_items: Vec<PopupItem>,
    pub permission_selected_index: usize,
    pub context_left_label: String,
    pub approval_mode_label: String,
    pub thread_duration_label: String,
    pub queue_label: String,
    pub activity_label: String,
    pub working_seconds: Option<u64>,
    pub transcript_rows: Vec<TranscriptRow>,
    pub tip_line: Option<String>,
    pub composer_placeholder: String,
}

impl Default for DashboardOptions {
    fn default() -> Self {
        Self {
            color: false,
            width: 120,
            theme_name: "default".to_string(),
            workspace_path: ".".to_string(),
            selected_model_id: None,
            model_choices: Vec::new(),
            model_picker_open: false,
            command_buffer: String::new(),
            slash_menu_selected_index: 0,
            mention_items: Vec::new(),
            mention_selected_index: 0,
            permission_title: None,
            permission_items: Vec::new(),
            permission_selected_index: 0,
            context_left_label: "100% left".to_string(),
            approval_mode_label: "manual approvals".to_string(),
            thread_duration_label: "0s thread".to_string(),
            queue_label: "queue 0".to_string(),
            activity_label: "idle".to_string(),
            working_seconds: None,
            transcript_rows: Vec::new(),
            tip_line: Some("Tip: Use /model or /new.".to_string()),
            composer_placeholder: "Improve documentation in @filename".to_string(),
        }
    }
}

pub fn render_dashboard(snapshot: &Snapshot, options: DashboardOptions) -> String {
    let width = options.width.clamp(60, 160);
    let mut lines = Vec::new();

    lines.extend(render_card(&options, width));
    if let Some(tip) = &options.tip_line {
        lines.push(String::new());
        lines.push(truncate(tip, width));
    }

    let transcript = render_transcript(snapshot, &options, width);
    if !transcript.is_empty() {
        lines.push(String::new());
        lines.extend(transcript);
    }

    lines.push(String::new());
    lines.push(render_composer(&options, width));

    let popup = render_popup(&options, width);
    if !popup.is_empty() {
        lines.extend(popup);
    }

    lines.push(String::new());
    lines.push(render_footer(&options, width));

    lines.join("\n")
}

fn render_card(options: &DashboardOptions, width: usize) -> Vec<String> {
    let model = model_label(options);
    let body = [
        format!(" >_ Vorker (v{})", env!("CARGO_PKG_VERSION")),
        " ".to_string(),
        format!(" model:     {model}   /model to change"),
        format!(" directory: {}", options.workspace_path),
    ];

    let inner_width = body
        .iter()
        .map(|line| visible_length(line))
        .max()
        .unwrap_or(0)
        .saturating_add(2)
        .min(width.saturating_sub(2).max(20));
    let horizontal = "─".repeat(inner_width);
    let border_tone = if is_review_theme(options) {
        "brightMagenta"
    } else {
        "brightGreen"
    };
    let mut card = vec![if options.color {
        colorize(&format!("╭{horizontal}╮"), border_tone, true)
    } else {
        format!("╭{horizontal}╮")
    }];

    for line in body {
        let content = fit(&truncate(&line, inner_width), inner_width);
        card.push(if options.color {
            format!(
                "{}{content}{}",
                colorize("│", border_tone, true),
                colorize("│", border_tone, true)
            )
        } else {
            format!("│{content}│")
        });
    }

    card.push(if options.color {
        colorize(&format!("╰{horizontal}╯"), border_tone, true)
    } else {
        format!("╰{horizontal}╯")
    });
    card
}

fn render_transcript(snapshot: &Snapshot, options: &DashboardOptions, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let rows = transcript_rows(snapshot, options);

    for row in rows {
        let prefix_text = match row.kind {
            RowKind::User => "› ",
            RowKind::Assistant | RowKind::System | RowKind::Tool => "• ",
        };
        let prefix = if options.color {
            let tone = match row.kind {
                RowKind::User => {
                    if is_review_theme(options) {
                        "brightMagenta"
                    } else {
                        "brightGreen"
                    }
                }
                RowKind::Assistant => "white",
                RowKind::Tool => {
                    if is_review_theme(options) {
                        "yellow"
                    } else {
                        "brightMagenta"
                    }
                }
                RowKind::System => "gray",
            };
            colorize(prefix_text, tone, true)
        } else {
            prefix_text.to_string()
        };
        let context = if is_review_theme(options) {
            RichContext::Review
        } else {
            RichContext::Normal
        };
        lines.extend(wrap_prefixed(
            &row.text,
            &prefix,
            width,
            options.color,
            context,
        ));
        if let Some(detail) = row.detail {
            let detail_prefix = if options.color {
                colorize(
                    "  └ ",
                    if options.theme_name == "adversarial" {
                        "yellow"
                    } else {
                        "gray"
                    },
                    true,
                )
            } else {
                "  └ ".to_string()
            };
            lines.extend(wrap_prefixed(
                &detail,
                &detail_prefix,
                width,
                options.color,
                context,
            ));
        }
        lines.push(String::new());
    }

    if let Some(seconds) = options.working_seconds {
        lines.extend(wrap_prefixed(
            &format!("Working ({seconds}s • enter to queue/steer • /stop to interrupt)"),
            &if options.color {
                colorize(
                    "◦ ",
                    if is_review_theme(options) {
                        "brightMagenta"
                    } else {
                        "brightGreen"
                    },
                    true,
                )
            } else {
                "◦ ".to_string()
            },
            width,
            options.color,
            RichContext::Normal,
        ));
    }

    while matches!(lines.last(), Some(last) if last.is_empty()) {
        lines.pop();
    }

    lines
}

fn transcript_rows(snapshot: &Snapshot, options: &DashboardOptions) -> Vec<TranscriptRow> {
    if !options.transcript_rows.is_empty() {
        return options.transcript_rows.clone();
    }

    let session = snapshot.sessions.first();
    session
        .map(|entry| entry.transcript.iter().map(transcript_entry_row).collect())
        .unwrap_or_default()
}

fn transcript_entry_row(entry: &TranscriptEntry) -> TranscriptRow {
    let kind = match entry.role.as_str() {
        "user" => RowKind::User,
        "assistant" => RowKind::Assistant,
        "tool" => RowKind::Tool,
        _ => RowKind::System,
    };

    TranscriptRow {
        kind,
        text: entry.text.clone(),
        detail: None,
    }
}

fn render_composer(options: &DashboardOptions, width: usize) -> String {
    let composer_text = if options.command_buffer.trim().is_empty() {
        options.composer_placeholder.clone()
    } else {
        options.command_buffer.clone()
    };
    let plain = fit(&truncate(&format!("› {composer_text}"), width), width);
    if !options.color {
        return plain;
    }

    let mut chars = plain.chars();
    let _ = chars.next();
    let rest = chars.collect::<String>();
    let text_foreground = if options.command_buffer.trim().is_empty() {
        "\u{1b}[38;5;245m"
    } else {
        "\u{1b}[39m"
    };
    let (background, accent) = if is_review_theme(options) {
        ("\u{1b}[48;5;237m", "\u{1b}[38;5;213m")
    } else {
        ("\u{1b}[48;5;238m", "\u{1b}[38;5;117m")
    };

    format!("{background}{accent}›\u{1b}[39m{text_foreground}{rest}\u{1b}[0m")
}

fn render_popup(options: &DashboardOptions, width: usize) -> Vec<String> {
    if let Some(title) = &options.permission_title {
        let mut lines = vec![truncate(&format!("  {title}"), width)];
        for (index, item) in options.permission_items.iter().enumerate() {
            lines.push(render_popup_line(
                item,
                index == options.permission_selected_index,
                width,
                options.color,
            ));
        }
        return lines;
    }

    if options.model_picker_open {
        return options
            .model_choices
            .iter()
            .enumerate()
            .map(|(index, model)| {
                render_popup_line(
                    &PopupItem {
                        label: model.clone(),
                        description: None,
                        selectable: true,
                    },
                    options
                        .selected_model_id
                        .as_deref()
                        .is_some_and(|selected| selected == model)
                        || index == options.slash_menu_selected_index,
                    width,
                    options.color,
                )
            })
            .collect();
    }

    if !options.mention_items.is_empty() {
        return options
            .mention_items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                render_popup_line(
                    &PopupItem {
                        label: item.clone(),
                        description: None,
                        selectable: true,
                    },
                    index == options.mention_selected_index,
                    width,
                    options.color,
                )
            })
            .collect();
    }

    let commands = filtered_commands_for_state(
        &options.command_buffer,
        is_review_theme(options),
        options.working_seconds.is_some(),
    );
    if commands.is_empty() {
        return Vec::new();
    }

    let mut items = Vec::new();
    let mut last_category = None;
    for command in commands {
        if last_category != Some(command.category) {
            items.push(PopupItem {
                label: category_label(command.category).to_string(),
                description: None,
                selectable: false,
            });
            last_category = Some(command.category);
        }
        items.push(PopupItem {
            label: command.name.to_string(),
            description: Some(command.description.to_string()),
            selectable: true,
        });
    }

    let mut selectable_index = 0usize;
    items
        .iter()
        .map(|item| {
            let selected = item.selectable && selectable_index == options.slash_menu_selected_index;
            if item.selectable {
                selectable_index += 1;
            }
            render_popup_line(item, selected, width, options.color)
        })
        .collect()
}

fn render_popup_line(item: &PopupItem, selected: bool, width: usize, color: bool) -> String {
    if !item.selectable {
        let heading = format!("  {}", item.label);
        return if color {
            colorize(&fit(&truncate(&heading, width), width), "gray", true)
        } else {
            fit(&truncate(&heading, width), width)
        };
    }
    let description = item.description.as_deref().unwrap_or_default();
    let base = if description.is_empty() {
        format!("  {}", item.label)
    } else {
        format!("  {}   {}", item.label, description)
    };
    let fitted = fit(&truncate(&base, width), width);
    if selected && color {
        highlight(
            &fitted,
            true,
            if item.label.contains("danger") || item.label.contains("reject") {
                "bgRed"
            } else {
                "bgGray"
            },
            "white",
        )
    } else {
        fitted
    }
}

fn render_footer(options: &DashboardOptions, width: usize) -> String {
    let model = model_label(options);
    let activity = if options.color {
        colorize(
            &options.activity_label,
            activity_tone(&options.activity_label),
            true,
        )
    } else {
        options.activity_label.clone()
    };
    let theme = if options.color {
        colorize(&format!("t:{}", options.theme_name), "gray", true)
    } else {
        format!("t:{}", options.theme_name)
    };
    let queue = if options.color {
        colorize(&options.queue_label, "gray", true)
    } else {
        options.queue_label.clone()
    };
    truncate(
        &format!(
            "{model} · {} · {} · {} · {} · {queue} · {activity} · {theme}",
            options.context_left_label,
            options.workspace_path,
            options.approval_mode_label,
            options.thread_duration_label
        ),
        width,
    )
}

fn model_label(options: &DashboardOptions) -> &str {
    options
        .selected_model_id
        .as_deref()
        .unwrap_or("detecting...")
}

fn activity_tone(label: &str) -> &str {
    if label.contains("review") || label.contains("working") {
        "yellow"
    } else if label.contains("failed") {
        "red"
    } else {
        "gray"
    }
}

fn is_review_theme(options: &DashboardOptions) -> bool {
    matches!(options.theme_name.as_str(), "review" | "opencode")
}

fn wrap_prefixed(
    text: &str,
    prefix: &str,
    width: usize,
    color: bool,
    context: RichContext,
) -> Vec<String> {
    let available = width.saturating_sub(visible_length(prefix)).max(8);
    let mut lines = Vec::new();
    let mut first = true;
    for source_line in text.lines() {
        if source_line.is_empty() {
            lines.push(String::new());
            continue;
        }
        let chunks = hard_wrap(source_line, available);
        for chunk in chunks {
            let styled = style_line(&chunk, context, color);
            if first {
                lines.push(format!("{prefix}{styled}"));
                first = false;
            } else {
                lines.push(format!("{}{}", " ".repeat(visible_length(prefix)), styled));
            }
        }
    }
    if lines.is_empty() {
        lines.push(prefix.trim_end().to_string());
    }
    lines
}
