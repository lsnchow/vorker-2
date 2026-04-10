use std::fs;

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use vorker_cli::ralph::{RalphLaunchRequest, build_ralph_launch};

#[test]
fn ralph_launch_uses_user_codex_home_when_project_auth_is_missing() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project = temp.path().join("project");
    let user_home = temp.path().join("home");
    fs::create_dir_all(project.join(".codex")).expect("project codex dir");
    fs::create_dir_all(user_home.join(".codex")).expect("user codex dir");
    fs::write(user_home.join(".codex/auth.json"), "{}").expect("auth fixture");

    let launch = build_ralph_launch(RalphLaunchRequest {
        cwd: project.clone(),
        user_home: user_home.clone(),
        task: "finish the Vorker shell".to_string(),
        model: Some("gpt-5.4".to_string()),
        no_deslop: true,
        no_alt_screen: true,
        xhigh: true,
        extra_args: Vec::new(),
    })
    .expect("launch plan");

    assert_eq!(launch.program, "omx");
    assert_eq!(
        launch.env.get("CODEX_HOME").map(String::as_str),
        Some(user_home.join(".codex").to_string_lossy().as_ref())
    );
    assert_eq!(
        launch.env.get("TERM").map(String::as_str),
        Some("xterm-256color")
    );
    assert_eq!(
        launch.args,
        vec![
            "ralph",
            "--no-deslop",
            "--no-alt-screen",
            "--xhigh",
            "--model",
            "gpt-5.4",
            "finish the Vorker shell",
        ]
    );
}

#[test]
fn ralph_launch_prefers_project_codex_home_when_project_auth_exists() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project = temp.path().join("project");
    let user_home = temp.path().join("home");
    fs::create_dir_all(project.join(".codex")).expect("project codex dir");
    fs::write(project.join(".codex/auth.json"), "{}").expect("project auth fixture");
    fs::create_dir_all(user_home.join(".codex")).expect("user codex dir");
    fs::write(user_home.join(".codex/auth.json"), "{}").expect("user auth fixture");

    let launch = build_ralph_launch(RalphLaunchRequest {
        cwd: project.clone(),
        user_home,
        task: "inspect context".to_string(),
        model: None,
        no_deslop: false,
        no_alt_screen: false,
        xhigh: false,
        extra_args: vec!["--search".to_string()],
    })
    .expect("launch plan");

    assert_eq!(
        launch.env.get("CODEX_HOME").map(String::as_str),
        Some(project.join(".codex").to_string_lossy().as_ref())
    );
    assert_eq!(launch.args, vec!["ralph", "--search", "inspect context"]);
}

#[test]
fn ralph_dry_run_prints_safe_launch_command() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project = temp.path().join("project");
    let user_home = temp.path().join("home");
    fs::create_dir_all(project.join(".codex")).expect("project codex dir");
    fs::create_dir_all(user_home.join(".codex")).expect("user codex dir");
    fs::write(user_home.join(".codex/auth.json"), "{}").expect("auth fixture");

    let mut cmd = Command::cargo_bin("vorker").expect("binary exists");
    cmd.env("HOME", &user_home)
        .current_dir(&project)
        .args([
            "ralph",
            "--dry-run",
            "--no-deslop",
            "--xhigh",
            "--model",
            "gpt-5.4",
            "ship",
            "everything",
        ])
        .assert()
        .success()
        .stdout(
            contains("CODEX_HOME=")
                .and(contains(
                    user_home.join(".codex").to_string_lossy().as_ref(),
                ))
                .and(contains("TERM=xterm-256color"))
                .and(contains(
                    "omx ralph --no-deslop --no-alt-screen --xhigh --model gpt-5.4 ship everything",
                )),
        );
}
