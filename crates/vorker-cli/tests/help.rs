use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

#[test]
fn cli_help_lists_tui_and_serve_commands() {
    let mut cmd = Command::cargo_bin("vorker").expect("binary exists");

    cmd.arg("--help").assert().success().stdout(
        contains("tui")
            .and(contains("serve"))
            .and(contains("--copilot-bin"))
            .and(contains("--no-alt-screen")),
    );
}

#[test]
fn tui_once_renders_a_real_dashboard_frame() {
    let mut cmd = Command::cargo_bin("vorker").expect("binary exists");

    cmd.arg("tui").arg("--once").assert().success().stdout(
        contains("[vorker]")
            .and(contains("ACTIONS"))
            .and(contains("NAVIGATION"))
            .and(contains("INPUT")),
    );
}
