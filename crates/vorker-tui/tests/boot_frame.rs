use vorker_tui::{BootStep, boot_minimum_ticks, render_boot_frame};

#[test]
fn render_boot_frame_shows_the_vorker_banner_and_loading_step() {
    let output = render_boot_frame(
        96,
        5,
        Some("copilot-session"),
        &[
            BootStep::new(
                "copilot-session",
                "copilot",
                "loading",
                "loading model inventory",
            ),
        ],
        false,
    );

    assert!(
        output.contains("██╗   ██╗ ██████╗ ██████╗ ██╗  ██╗███████╗██████╗"),
        "missing first banner line:\n{output}"
    );
    assert!(
        output.contains("╚═══╝   ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝╚══════╝╚═╝  ╚═╝"),
        "missing last banner line:\n{output}"
    );
    assert!(
        output.contains("copilot"),
        "missing step label:\n{output}"
    );
    assert!(
        output.contains("loading model inventory"),
        "missing step detail:\n{output}"
    );
}

#[test]
fn render_boot_frame_reveals_banner_lines_progressively() {
    let early = render_boot_frame(
        96,
        0,
        None,
        &[BootStep::new("copilot-session", "copilot", "loading", "loading model inventory")],
        false,
    );
    let late = render_boot_frame(
        96,
        5,
        None,
        &[BootStep::new("copilot-session", "copilot", "loading", "loading model inventory")],
        false,
    );

    assert!(
        early.contains("██╗   ██╗ ██████╗ ██████╗ ██╗  ██╗███████╗██████╗"),
        "first frame should reveal the first banner line:\n{early}"
    );
    assert!(
        !early.contains("╚═══╝   ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝╚══════╝╚═╝  ╚═╝"),
        "first frame should not reveal the full banner yet:\n{early}"
    );
    assert!(
        late.contains("╚═══╝   ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝╚══════╝╚═╝  ╚═╝"),
        "later frame should reveal the full banner:\n{late}"
    );
}

#[test]
fn boot_animation_lingers_beyond_the_full_banner_reveal() {
    assert_eq!(
        boot_minimum_ticks(),
        9,
        "boot animation should linger a bit after the sixth banner line"
    );
}
