use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

#[test]
fn cli_help_lists_tui_and_serve_commands() {
    let mut cmd = Command::cargo_bin("vorker").expect("binary exists");

    cmd.arg("--help").assert().success().stdout(
        contains("tui")
            .and(contains("preflight"))
            .and(contains("serve"))
            .and(contains("--copilot-bin"))
            .and(contains("--no-alt-screen")),
    );
}

#[test]
fn tui_once_renders_a_real_chat_shell() {
    let mut cmd = Command::cargo_bin("vorker").expect("binary exists");

    cmd.arg("tui").arg("--once").assert().success().stdout(
        contains("[vorker]")
            .and(contains("Navigation"))
            .and(contains("Conversation"))
            .and(contains("Composer")),
    );
}
