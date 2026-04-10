use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

#[test]
fn cli_help_lists_tui_and_serve_commands() {
    let mut cmd = Command::cargo_bin("vorker").expect("binary exists");

    cmd.arg("--help").assert().success().stdout(
        contains("tui")
            .and(contains("preflight"))
            .and(contains("adversarial"))
            .and(contains("demo"))
            .and(contains("serve"))
            .and(contains("--provider"))
            .and(contains("--copilot-bin"))
            .and(contains("--codex-bin"))
            .and(contains("--no-alt-screen")),
    );
}

#[test]
fn tui_once_renders_a_real_chat_shell() {
    let mut cmd = Command::cargo_bin("vorker").expect("binary exists");

    cmd.arg("tui").arg("--once").assert().success().stdout(
        contains(">_ Vorker (v0.1.0)")
            .and(contains("model:     claude-opus-4.5   /model to change"))
            .and(contains("› Improve documentation in @filename")),
    );
}

#[test]
fn demo_command_renders_the_hyperloop_mock_screen() {
    let mut cmd = Command::cargo_bin("vorker").expect("binary exists");

    cmd.arg("demo")
        .arg("hyperloop")
        .assert()
        .success()
        .stdout(
            contains("Hyperloop Pod Controls")
                .and(contains("Subagents"))
                .and(contains("Safety envelope verified")),
        );
}
