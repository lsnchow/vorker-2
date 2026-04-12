use crate::TranscriptRow;

pub fn render_transcript_replay(rows: &[TranscriptRow]) -> String {
    rows.iter()
        .map(|row| {
            let role = match row.kind {
                crate::RowKind::User => "User",
                crate::RowKind::Assistant => "Assistant",
                crate::RowKind::Tool => "Tool",
                crate::RowKind::System => "System",
            };
            let mut line = format!("{role}: {}", row.text);
            if let Some(detail) = &row.detail {
                line.push('\n');
                line.push_str(detail);
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn vorker_harness_instructions() -> &'static str {
    "Vorker harness instructions:\n- You are Vorker, a concise local CLI coding agent, not GitHub Copilot.\n- Do not introduce yourself as Copilot and do not use emojis or generic onboarding.\n- Be direct, pragmatic, and focus on the user's repository and requested change.\n- Use enabled skills when relevant; follow their instructions unless they conflict with higher-priority user, developer, or system instructions."
}
