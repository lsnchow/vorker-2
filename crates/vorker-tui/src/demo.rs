use crate::render::{DashboardOptions, RowKind, TranscriptRow, render_dashboard};
use vorker_core::Snapshot;

#[must_use]
pub fn render_hyperloop_mock(width: usize, color: bool) -> String {
    let shell = render_dashboard(
        &Snapshot::default(),
        DashboardOptions {
            color,
            width,
            workspace_path: "~/projects/hyperloop-pod".to_string(),
            selected_model_id: Some("gpt-5.4 xhigh".to_string()),
            context_left_label: "84% left".to_string(),
            approval_mode_label: "auto approvals".to_string(),
            thread_duration_label: "17m 42s thread".to_string(),
            transcript_rows: vec![
                TranscriptRow {
                    kind: RowKind::System,
                    text: "Hyperloop Pod Controls".to_string(),
                    detail: Some("Completed coding session: redundant levitation, braking, and watchdog control surfaces.".to_string()),
                },
                TranscriptRow {
                    kind: RowKind::User,
                    text: "Implement a redundant controls system for the hyperloop pod.".to_string(),
                    detail: None,
                },
                TranscriptRow {
                    kind: RowKind::Tool,
                    text: "Explored".to_string(),
                    detail: Some("Read control-loop spec, safety cases, and hardware watchdog bindings.".to_string()),
                },
                TranscriptRow {
                    kind: RowKind::Tool,
                    text: "Changed".to_string(),
                    detail: Some("Patched triple-redundant braking arbitration, telemetry failsafes, and actuator saturation limits.".to_string()),
                },
                TranscriptRow {
                    kind: RowKind::Assistant,
                    text: "Safety envelope verified. The pod now degrades into a bounded limp-home mode when a controller diverges, and the watchdog can hard-stop the propulsion rail independently.".to_string(),
                    detail: None,
                },
            ],
            composer_placeholder: "Ask follow-up questions about the control system".to_string(),
            tip_line: Some("Demo: finished coding session view.".to_string()),
            ..DashboardOptions::default()
        },
    );

    format!(
        "{shell}\n\nSubagents\nSelect an agent to watch. ⌥ + ← previous, ⌥ + → next.\n\n› 1. • Main [default] (current) 019d35b1-a188-7f62-8e7d-2d8e3e5523f7\n  2. • Safety reviewer [worker] 019d35b1-a188-7f62-8e7d-2d8e3e5523f8\n  3. • Controls sim [worker] 019d35b1-a188-7f62-8e7d-2d8e3e5523f9"
    )
}
