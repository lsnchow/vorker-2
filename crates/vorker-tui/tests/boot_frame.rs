use vorker_tui::{BootStep, render_boot_frame};

#[test]
fn render_boot_frame_shows_the_new_title_and_multi_agent_loading_lanes() {
    let output = render_boot_frame(
        96,
        3,
        Some("worker-pool"),
        &[
            BootStep::new(
                "event-log",
                "event log",
                "ready",
                "replayed supervisor journal",
            ),
            BootStep::new(
                "worker-pool",
                "worker-pool",
                "loading",
                "warming 6 execution lanes",
            ),
            BootStep::new(
                "merge-queue",
                "merge-queue",
                "pending",
                "syncing reconciler state",
            ),
        ],
        false,
    );

    assert!(output.contains("██╗   ██╗"), "missing title art:\n{output}");
    assert!(
        output.contains("worker-pool"),
        "missing worker-pool:\n{output}"
    );
    assert!(
        output.contains("warming 6 execution lanes"),
        "missing worker detail:\n{output}"
    );
    assert!(
        output.contains("VORKER CONTROL PLANE"),
        "missing control plane title:\n{output}"
    );
}
