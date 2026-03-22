//! Preflight OSS runtime support.

use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tempfile::TempDir;
use toml::Value as TomlValue;
use uuid::Uuid;
use vorker_core::{SupervisorEvent, create_supervisor_event, now_iso};
use walkdir::WalkDir;

type PreflightResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

const HIGH_RISK_TOKENS: &[(&str, &str)] = &[
    ("sudo ", "attempts to use sudo"),
    ("rm -rf /", "contains a destructive root filesystem pattern"),
    ("BEGIN RSA PRIVATE KEY", "contains a private key marker"),
    ("ghp_", "contains a GitHub token marker"),
    ("AKIA", "contains an AWS access key marker"),
];

const MEDIUM_RISK_TOKENS: &[(&str, &str)] = &[
    ("postinstall", "declares a postinstall hook"),
    ("prepare", "declares a prepare hook"),
    ("chmod 777", "contains a broad chmod pattern"),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreflightOutcome {
    #[serde(rename = "Static only")]
    StaticOnly,
    Buildable,
    Runnable,
    Verified,
}

impl std::fmt::Display for PreflightOutcome {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StaticOnly => formatter.write_str("Static only"),
            Self::Buildable => formatter.write_str("Buildable"),
            Self::Runnable => formatter.write_str("Runnable"),
            Self::Verified => formatter.write_str("Verified"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreflightRequest {
    pub repo: String,
    pub artifacts_root: Option<PathBuf>,
    pub approve_high_risk: bool,
}

impl PreflightRequest {
    #[must_use]
    pub fn new(repo: impl Into<String>) -> Self {
        Self {
            repo: repo.into(),
            artifacts_root: None,
            approve_high_risk: false,
        }
    }

    #[must_use]
    pub fn with_artifacts_root(mut self, artifacts_root: impl AsRef<Path>) -> Self {
        self.artifacts_root = Some(artifacts_root.as_ref().to_path_buf());
        self
    }

    #[must_use]
    pub fn approve_high_risk(mut self, approve_high_risk: bool) -> Self {
        self.approve_high_risk = approve_high_risk;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxPhase {
    Setup,
    Build,
    Run,
    Verify,
}

impl std::fmt::Display for SandboxPhase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Setup => formatter.write_str("setup"),
            Self::Build => formatter.write_str("build"),
            Self::Run => formatter.write_str("run"),
            Self::Verify => formatter.write_str("verify"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxInvocation {
    pub phase: SandboxPhase,
    pub image: String,
    pub workdir: PathBuf,
    pub command: String,
    pub network_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl SandboxResult {
    #[must_use]
    pub fn success(stdout: impl Into<String>) -> Self {
        Self {
            success: true,
            stdout: stdout.into(),
            stderr: String::new(),
            exit_code: 0,
        }
    }

    #[must_use]
    pub fn failure(exit_code: i32, stdout: impl Into<String>, stderr: impl Into<String>) -> Self {
        Self {
            success: false,
            stdout: stdout.into(),
            stderr: stderr.into(),
            exit_code,
        }
    }
}

pub trait PreflightSandbox {
    fn backend_name(&self) -> &str;
    fn is_available(&self) -> bool;
    fn run(&self, invocation: SandboxInvocation) -> SandboxResult;
}

#[derive(Debug, Clone)]
pub struct LocalContainerSandbox {
    backend: Option<String>,
}

impl Default for LocalContainerSandbox {
    fn default() -> Self {
        Self::detect()
    }
}

impl LocalContainerSandbox {
    #[must_use]
    pub fn detect() -> Self {
        let backend = ["docker", "podman"]
            .into_iter()
            .find(|candidate| command_exists(candidate))
            .map(str::to_owned);
        Self { backend }
    }
}

impl PreflightSandbox for LocalContainerSandbox {
    fn backend_name(&self) -> &str {
        self.backend.as_deref().unwrap_or("unavailable")
    }

    fn is_available(&self) -> bool {
        self.backend.is_some()
    }

    fn run(&self, invocation: SandboxInvocation) -> SandboxResult {
        let Some(backend) = &self.backend else {
            return SandboxResult::failure(
                127,
                "",
                "No supported sandbox backend detected. Install Docker or Podman.",
            );
        };

        let output = Command::new(backend)
            .args([
                "run",
                "--rm",
                "--pull",
                "never",
                "--workdir",
                "/workspace",
                "--memory",
                "4g",
                "--cpus",
                "2",
                "--pids-limit",
                "256",
                "--security-opt",
                "no-new-privileges",
                "--network",
                if invocation.network_enabled {
                    "bridge"
                } else {
                    "none"
                },
                "-v",
            ])
            .arg(format!("{}:/workspace:rw", invocation.workdir.display()))
            .arg(&invocation.image)
            .args(["sh", "-lc"])
            .arg(&invocation.command)
            .output();

        match output {
            Ok(output) => SandboxResult {
                success: output.status.success(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code().unwrap_or(1),
            },
            Err(error) => SandboxResult::failure(1, "", error.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskReport {
    pub level: String,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutedCommand {
    pub phase: String,
    pub command: String,
    pub success: bool,
    pub exit_code: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxReport {
    pub backend: String,
    pub available: bool,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreflightReport {
    pub run_id: String,
    pub repo_input: String,
    pub repo_source_type: String,
    pub repo_origin: Option<String>,
    pub repo_path: Option<String>,
    pub repo_class: String,
    pub classification_confidence: String,
    pub strategy: String,
    pub runtime_family: String,
    pub package_manager: Option<String>,
    pub risk: RiskReport,
    pub sandbox: SandboxReport,
    pub stage: String,
    pub outcome: PreflightOutcome,
    pub preview_url: Option<String>,
    pub latest_failure: Option<String>,
    pub commands: Vec<ExecutedCommand>,
    pub changed_files: Vec<String>,
    pub artifacts_dir: String,
    pub patch_diff_path: String,
    pub report_path: String,
    pub summary_path: String,
    pub metadata_path: String,
    pub stopped_because: String,
    pub next_steps: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PreflightRunResult {
    pub report: PreflightReport,
    pub artifacts_dir: PathBuf,
    pub events: Vec<SupervisorEvent>,
}

#[derive(Debug, Clone)]
pub struct PreflightRunner<S> {
    sandbox: S,
}

impl<S> PreflightRunner<S>
where
    S: PreflightSandbox,
{
    #[must_use]
    pub fn new(sandbox: S) -> Self {
        Self { sandbox }
    }

    pub fn run(&self, request: PreflightRequest) -> PreflightResult<PreflightRunResult> {
        let run_id = format!("preflight-{}", Uuid::new_v4().simple());
        let artifacts_root = request
            .artifacts_root
            .clone()
            .unwrap_or(default_artifacts_root()?);
        let artifacts_dir = artifacts_root.join(&run_id);
        let logs_dir = artifacts_dir.join("logs");
        fs::create_dir_all(&logs_dir)?;

        let mut recorder = EventRecorder::new();
        recorder.emit(
            "run.created",
            json!({
                "run": {
                    "id": run_id,
                    "name": format!("Preflight {}", request.repo),
                    "goal": format!("Vet {}", request.repo),
                    "status": "running",
                    "type": "preflight",
                    "notes": "preflight",
                    "createdAt": now_iso(),
                    "updatedAt": now_iso()
                },
                "preflight": {
                    "runId": run_id,
                    "repoInput": request.repo,
                    "stage": "intake",
                    "sandboxState": "idle",
                    "sandboxBackend": self.sandbox.backend_name(),
                    "artifactsDir": artifacts_dir.display().to_string()
                }
            }),
        );

        let workspace = TempDir::new()?;
        let repo = prepare_repo_workspace(&request.repo, workspace.path())?;
        recorder.emit(
            "preflight.created",
            json!({
                "run": {
                    "id": run_id,
                    "status": "running",
                    "updatedAt": now_iso()
                },
                "preflight": {
                    "runId": run_id,
                    "repoInput": request.repo,
                    "repoSourceType": repo.source_type,
                    "repoOrigin": repo.origin,
                    "repoPath": repo.repo_path.display().to_string(),
                    "stage": "classify",
                    "sandboxBackend": self.sandbox.backend_name(),
                    "sandboxState": "prepared",
                    "artifactsDir": artifacts_dir.display().to_string()
                }
            }),
        );

        let classification = classify_repo(&repo.repo_path)?;
        recorder.emit(
            "preflight.classified",
            json!({
                "run": {
                    "id": run_id,
                    "status": "running",
                    "updatedAt": now_iso()
                },
                "preflight": {
                    "runId": run_id,
                    "repoInput": request.repo,
                    "repoSourceType": repo.source_type,
                    "repoOrigin": repo.origin,
                    "repoPath": repo.repo_path.display().to_string(),
                    "classification": classification.repo_class,
                    "classificationConfidence": classification.confidence,
                    "strategy": classification.strategy_name,
                    "runtimeFamily": classification.runtime_family,
                    "packageManager": classification.package_manager,
                    "stage": "risk",
                    "sandboxBackend": self.sandbox.backend_name(),
                    "sandboxState": "prepared",
                    "artifactsDir": artifacts_dir.display().to_string()
                }
            }),
        );

        let risk = assess_risk(&repo.repo_path)?;
        write_json(&artifacts_dir.join("risk.json"), &risk)?;
        write_json(&artifacts_dir.join("strategy.json"), &classification)?;
        recorder.emit(
            "preflight.risk_scored",
            json!({
                "run": {
                    "id": run_id,
                    "status": "running",
                    "updatedAt": now_iso()
                },
                "preflight": {
                    "runId": run_id,
                    "riskLevel": risk.level,
                    "riskReasons": risk.reasons,
                    "stage": "repair",
                    "sandboxBackend": self.sandbox.backend_name(),
                    "sandboxState": "prepared",
                    "artifactsDir": artifacts_dir.display().to_string()
                }
            }),
        );

        let changed_files = apply_safe_repairs(&repo.repo_path)?;
        let patch_diff = capture_patch_diff(&repo.repo_path).unwrap_or_default();
        fs::write(artifacts_dir.join("patch.diff"), &patch_diff)?;
        recorder.emit(
            "preflight.patch.generated",
            json!({
                "run": {
                    "id": run_id,
                    "status": "running",
                    "updatedAt": now_iso()
                },
                "preflight": {
                    "runId": run_id,
                    "stage": "setup",
                    "patchDiffPath": artifacts_dir.join("patch.diff").display().to_string(),
                    "artifactsDir": artifacts_dir.display().to_string()
                }
            }),
        );

        let mut command_log = Vec::new();
        let mut stage = "setup".to_string();
        let mut latest_failure = None;
        let mut outcome = PreflightOutcome::StaticOnly;
        let mut sandbox_state = "idle".to_string();
        let preview_url = None;

        if risk.level == "high" && !request.approve_high_risk {
            latest_failure = Some(
                "High-risk repository requires explicit approval before sandbox execution."
                    .to_string(),
            );
        } else if !self.sandbox.is_available() {
            latest_failure = Some(
                "No supported sandbox backend detected. Install Docker or Podman.".to_string(),
            );
        } else if classification.image.is_none() {
            latest_failure = Some(
                "Repository class is currently unsupported for sandbox execution.".to_string(),
            );
        } else {
            recorder.emit(
                "preflight.execution.started",
                json!({
                    "run": {
                        "id": run_id,
                        "status": "running",
                        "updatedAt": now_iso()
                    },
                    "preflight": {
                        "runId": run_id,
                        "stage": "setup",
                        "sandboxBackend": self.sandbox.backend_name(),
                        "sandboxState": "running",
                        "artifactsDir": artifacts_dir.display().to_string()
                    }
                }),
            );

            if let Some(command) = &classification.setup_command {
                let result = self.execute_phase(
                    &repo.repo_path,
                    classification.image.as_deref().expect("image present"),
                    SandboxPhase::Setup,
                    command,
                    true,
                    &logs_dir,
                )?;
                command_log.push(ExecutedCommand {
                    phase: "setup".to_string(),
                    command: command.clone(),
                    success: result.success,
                    exit_code: result.exit_code,
                });
                if !result.success {
                    latest_failure = Some(first_failure(&result));
                }
            }

            if latest_failure.is_none()
                && let Some(command) = &classification.build_command
            {
                let result = self.execute_phase(
                    &repo.repo_path,
                    classification.image.as_deref().expect("image present"),
                    SandboxPhase::Build,
                    command,
                    true,
                    &logs_dir,
                )?;
                command_log.push(ExecutedCommand {
                    phase: "build".to_string(),
                    command: command.clone(),
                    success: result.success,
                    exit_code: result.exit_code,
                });
                if result.success {
                    outcome = PreflightOutcome::Buildable;
                } else {
                    latest_failure = Some(first_failure(&result));
                }
            }

            if latest_failure.is_none()
                && let Some(command) = &classification.run_command
            {
                stage = "run".to_string();
                let result = self.execute_phase(
                    &repo.repo_path,
                    classification.image.as_deref().expect("image present"),
                    SandboxPhase::Run,
                    command,
                    false,
                    &logs_dir,
                )?;
                command_log.push(ExecutedCommand {
                    phase: "run".to_string(),
                    command: command.clone(),
                    success: result.success,
                    exit_code: result.exit_code,
                });
                if result.success {
                    outcome = PreflightOutcome::Runnable;
                } else {
                    latest_failure = Some(first_failure(&result));
                }
            }

            if latest_failure.is_none()
                && let Some(command) = &classification.verify_command
            {
                stage = "verify".to_string();
                let result = self.execute_phase(
                    &repo.repo_path,
                    classification.image.as_deref().expect("image present"),
                    SandboxPhase::Verify,
                    command,
                    false,
                    &logs_dir,
                )?;
                command_log.push(ExecutedCommand {
                    phase: "verify".to_string(),
                    command: command.clone(),
                    success: result.success,
                    exit_code: result.exit_code,
                });
                if result.success {
                    outcome = PreflightOutcome::Verified;
                } else if outcome != PreflightOutcome::Runnable {
                    outcome = PreflightOutcome::Buildable;
                    latest_failure = Some(first_failure(&result));
                } else {
                    latest_failure = Some(first_failure(&result));
                }
            }

            sandbox_state = if latest_failure.is_some() {
                "failed".to_string()
            } else {
                "completed".to_string()
            };
        }

        if latest_failure.is_some() {
            recorder.emit(
                "preflight.execution.failed",
                json!({
                    "run": {
                        "id": run_id,
                        "status": "blocked",
                        "updatedAt": now_iso()
                    },
                    "preflight": {
                        "runId": run_id,
                        "stage": stage,
                        "latestFailure": latest_failure,
                        "sandboxBackend": self.sandbox.backend_name(),
                        "sandboxState": sandbox_state,
                        "artifactsDir": artifacts_dir.display().to_string(),
                        "patchDiffPath": artifacts_dir.join("patch.diff").display().to_string()
                    }
                }),
            );
        }

        if outcome == PreflightOutcome::Verified {
            recorder.emit(
                "preflight.verified",
                json!({
                    "run": {
                        "id": run_id,
                        "status": "completed",
                        "updatedAt": now_iso()
                    },
                    "preflight": {
                        "runId": run_id,
                        "stage": "report",
                        "outcome": outcome.to_string(),
                        "sandboxBackend": self.sandbox.backend_name(),
                        "sandboxState": sandbox_state,
                        "artifactsDir": artifacts_dir.display().to_string(),
                        "patchDiffPath": artifacts_dir.join("patch.diff").display().to_string()
                    }
                }),
            );
        }

        let stopped_because = latest_failure
            .clone()
            .unwrap_or_else(|| format!("Reached {} with repo-class verification.", outcome));
        let report_path = artifacts_dir.join("report.json");
        let summary_path = artifacts_dir.join("summary.md");
        let metadata_path = artifacts_dir.join("metadata.json");
        let report = PreflightReport {
            run_id: run_id.clone(),
            repo_input: request.repo.clone(),
            repo_source_type: repo.source_type.clone(),
            repo_origin: repo.origin.clone(),
            repo_path: Some(repo.repo_path.display().to_string()),
            repo_class: classification.repo_class.clone(),
            classification_confidence: classification.confidence.clone(),
            strategy: classification.strategy_name.clone(),
            runtime_family: classification.runtime_family.clone(),
            package_manager: classification.package_manager.clone(),
            risk,
            sandbox: SandboxReport {
                backend: self.sandbox.backend_name().to_string(),
                available: self.sandbox.is_available(),
                state: sandbox_state.clone(),
            },
            stage: if outcome == PreflightOutcome::Verified {
                "report".to_string()
            } else {
                stage.clone()
            },
            outcome,
            preview_url,
            latest_failure: latest_failure.clone(),
            commands: command_log,
            changed_files,
            artifacts_dir: artifacts_dir.display().to_string(),
            patch_diff_path: artifacts_dir.join("patch.diff").display().to_string(),
            report_path: report_path.display().to_string(),
            summary_path: summary_path.display().to_string(),
            metadata_path: metadata_path.display().to_string(),
            stopped_because,
            next_steps: next_steps(latest_failure.as_deref(), &classification),
        };

        write_json(&report_path, &report)?;
        write_json(
            &metadata_path,
            &json!({
                "runId": run_id,
                "createdAt": now_iso(),
                "repoInput": request.repo,
                "repoPath": repo.repo_path.display().to_string(),
                "artifactDir": artifacts_dir.display().to_string()
            }),
        )?;
        fs::write(&summary_path, render_summary(&report))?;

        recorder.emit(
            "preflight.completed",
            json!({
                "run": {
                    "id": run_id,
                    "status": if report.outcome == PreflightOutcome::Verified { "completed" } else { "blocked" },
                    "updatedAt": now_iso()
                },
                "preflight": {
                    "runId": run_id,
                    "repoInput": report.repo_input,
                    "repoSourceType": report.repo_source_type,
                    "repoOrigin": report.repo_origin,
                    "repoPath": report.repo_path,
                    "classification": report.repo_class,
                    "classificationConfidence": report.classification_confidence,
                    "strategy": report.strategy,
                    "runtimeFamily": report.runtime_family,
                    "packageManager": report.package_manager,
                    "riskLevel": report.risk.level,
                    "riskReasons": report.risk.reasons,
                    "sandboxBackend": report.sandbox.backend,
                    "sandboxState": report.sandbox.state,
                    "stage": "report",
                    "outcome": report.outcome.to_string(),
                    "previewUrl": report.preview_url,
                    "latestFailure": report.latest_failure,
                    "artifactsDir": report.artifacts_dir,
                    "patchDiffPath": report.patch_diff_path,
                    "summaryPath": report.summary_path,
                    "reportPath": report.report_path,
                    "metadataPath": report.metadata_path
                }
            }),
        );

        Ok(PreflightRunResult {
            report,
            artifacts_dir,
            events: recorder.events,
        })
    }

    fn execute_phase(
        &self,
        repo_path: &Path,
        image: &str,
        phase: SandboxPhase,
        command: &str,
        network_enabled: bool,
        logs_dir: &Path,
    ) -> PreflightResult<SandboxResult> {
        validate_command_policy(command)?;
        let result = self.sandbox.run(SandboxInvocation {
            phase: phase.clone(),
            image: image.to_string(),
            workdir: repo_path.to_path_buf(),
            command: command.to_string(),
            network_enabled,
        });
        fs::write(
            logs_dir.join(format!("{phase}.log")),
            format!(
                "$ {command}\n\nSTDOUT:\n{}\n\nSTDERR:\n{}",
                result.stdout, result.stderr
            ),
        )?;
        Ok(result)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RepoClassification {
    repo_class: String,
    confidence: String,
    strategy_name: String,
    runtime_family: String,
    package_manager: Option<String>,
    image: Option<String>,
    setup_command: Option<String>,
    build_command: Option<String>,
    run_command: Option<String>,
    verify_command: Option<String>,
}

#[derive(Debug, Clone)]
struct PreparedRepo {
    source_type: String,
    origin: Option<String>,
    repo_path: PathBuf,
}

#[derive(Debug, Default)]
struct EventRecorder {
    events: Vec<SupervisorEvent>,
}

impl EventRecorder {
    fn new() -> Self {
        Self { events: Vec::new() }
    }

    fn emit(&mut self, kind: &str, payload: Value) {
        self.events.push(create_supervisor_event(kind, payload));
    }
}

fn prepare_repo_workspace(repo: &str, workspace_root: &Path) -> PreflightResult<PreparedRepo> {
    let repo_path = workspace_root.join("repo");
    if is_public_github_url(repo) {
        git_clone(repo, &repo_path, false)?;
        return Ok(PreparedRepo {
            source_type: "github".to_string(),
            origin: Some(repo.to_string()),
            repo_path,
        });
    }

    let input_path = PathBuf::from(repo);
    if !input_path.exists() {
        return Err(format!("Repository path does not exist: {repo}").into());
    }
    ensure_git_work_tree(&input_path)?;
    let canonical = fs::canonicalize(&input_path)?;
    git_clone(canonical.as_os_str(), &repo_path, true)?;
    Ok(PreparedRepo {
        source_type: "local".to_string(),
        origin: Some(canonical.display().to_string()),
        repo_path,
    })
}

fn git_clone(source: impl AsRef<OsStr>, target: &Path, local_clone: bool) -> PreflightResult<()> {
    let mut command = Command::new("git");
    command.arg("clone");
    if local_clone {
        command.args(["--local", "--no-hardlinks"]);
    } else {
        command.args(["--depth", "1"]);
    }
    let output = command.arg(source).arg(target).output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "git clone failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into())
    }
}

fn ensure_git_work_tree(repo_path: &Path) -> PreflightResult<()> {
    let output = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(repo_path)
        .output()?;
    if output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "true" {
        Ok(())
    } else {
        Err(format!(
            "Local input is not a git working tree: {}",
            repo_path.display()
        )
        .into())
    }
}

fn classify_repo(repo_path: &Path) -> PreflightResult<RepoClassification> {
    let package_json = repo_path.join("package.json");
    let cargo_toml = repo_path.join("Cargo.toml");
    let pyproject = repo_path.join("pyproject.toml");
    let requirements = repo_path.join("requirements.txt");

    if package_json.exists() {
        return classify_node_repo(repo_path, &package_json);
    }
    if cargo_toml.exists() {
        return classify_cargo_repo(repo_path, &cargo_toml);
    }
    if pyproject.exists() || requirements.exists() {
        return Ok(classify_python_repo(repo_path));
    }

    Ok(RepoClassification {
        repo_class: "unknown".to_string(),
        confidence: "0.35".to_string(),
        strategy_name: "static-only".to_string(),
        runtime_family: "unknown".to_string(),
        package_manager: None,
        image: None,
        setup_command: None,
        build_command: None,
        run_command: None,
        verify_command: None,
    })
}

fn classify_node_repo(
    repo_path: &Path,
    package_json: &Path,
) -> PreflightResult<RepoClassification> {
    let manifest: Value = serde_json::from_str(&fs::read_to_string(package_json)?)?;
    let scripts = manifest
        .get("scripts")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let deps = manifest
        .get("dependencies")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let dev_deps = manifest
        .get("devDependencies")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let package_manager = detect_node_package_manager(repo_path);
    let tool = package_manager_command(package_manager.as_deref().unwrap_or("npm"));
    let setup_command = Some(match package_manager.as_deref() {
        Some("pnpm") => {
            "corepack enable >/dev/null 2>&1 && pnpm install --frozen-lockfile || (corepack enable >/dev/null 2>&1 && pnpm install)"
                .to_string()
        }
        Some("yarn") => {
            "corepack enable >/dev/null 2>&1 && yarn install --immutable || (corepack enable >/dev/null 2>&1 && yarn install)"
                .to_string()
        }
        Some("bun") => "bun install".to_string(),
        _ => "npm install".to_string(),
    });

    let has_web_stack = deps.keys().chain(dev_deps.keys()).any(|name| {
        matches!(
            name.as_str(),
            "next" | "react" | "vite" | "nuxt" | "@remix-run/dev"
        )
    });
    let has_bin = manifest.get("bin").is_some();
    let has_dev = scripts.contains_key("dev");
    let has_start = scripts.contains_key("start");
    let has_test = scripts.contains_key("test");

    if has_bin {
        return Ok(RepoClassification {
            repo_class: "CLI tool".to_string(),
            confidence: "0.88".to_string(),
            strategy_name: "node-cli".to_string(),
            runtime_family: "node".to_string(),
            package_manager,
            image: Some("node:20-bookworm".to_string()),
            setup_command,
            build_command: scripts
                .contains_key("build")
                .then(|| format!("{tool} run build")),
            run_command: None,
            verify_command: Some("node . --help".to_string()),
        });
    }

    if has_web_stack || has_dev || has_start {
        let start_script = if has_start {
            format!("{tool} run start")
        } else {
            format!(
                "sh -lc '{tool} run dev > /tmp/vorker-runtime.log 2>&1 & app=$!; sleep 8; curl -fsS http://127.0.0.1:3000 || curl -fsS http://127.0.0.1:5173 || curl -fsS http://127.0.0.1:4173; status=$?; kill $app >/dev/null 2>&1 || true; wait $app >/dev/null 2>&1 || true; exit $status'"
            )
        };
        return Ok(RepoClassification {
            repo_class: "web app".to_string(),
            confidence: "0.84".to_string(),
            strategy_name: "node-web".to_string(),
            runtime_family: "node".to_string(),
            package_manager,
            image: Some("node:20-bookworm".to_string()),
            setup_command,
            build_command: scripts
                .contains_key("build")
                .then(|| format!("{tool} run build")),
            run_command: Some(start_script),
            verify_command: None,
        });
    }

    Ok(RepoClassification {
        repo_class: if has_test {
            "library/package".to_string()
        } else {
            "unknown".to_string()
        },
        confidence: if has_test { "0.67" } else { "0.40" }.to_string(),
        strategy_name: if has_test {
            "node-library".to_string()
        } else {
            "node-static".to_string()
        },
        runtime_family: "node".to_string(),
        package_manager,
        image: Some("node:20-bookworm".to_string()),
        setup_command,
        build_command: scripts
            .contains_key("build")
            .then(|| format!("{tool} run build")),
        run_command: None,
        verify_command: has_test.then(|| format!("{tool} test")),
    })
}

fn classify_cargo_repo(repo_path: &Path, cargo_toml: &Path) -> PreflightResult<RepoClassification> {
    let manifest: TomlValue = toml::from_str(&fs::read_to_string(cargo_toml)?)?;
    let has_bin = repo_path.join("src/main.rs").exists()
        || manifest.get("bin").is_some()
        || manifest
            .get("package")
            .and_then(TomlValue::as_table)
            .and_then(|table| table.get("default-run"))
            .is_some();
    let has_tests = repo_path.join("tests").exists();

    Ok(RepoClassification {
        repo_class: if has_bin {
            "CLI tool".to_string()
        } else {
            "library/package".to_string()
        },
        confidence: if has_bin { "0.84" } else { "0.75" }.to_string(),
        strategy_name: if has_bin {
            "cargo-cli".to_string()
        } else {
            "cargo-library".to_string()
        },
        runtime_family: "rust".to_string(),
        package_manager: Some("cargo".to_string()),
        image: Some("rust:1.85-bookworm".to_string()),
        setup_command: Some("cargo fetch".to_string()),
        build_command: Some("cargo build".to_string()),
        run_command: None,
        verify_command: if has_bin {
            Some("cargo run -- --help".to_string())
        } else if has_tests {
            Some("cargo test".to_string())
        } else {
            Some("cargo check".to_string())
        },
    })
}

fn classify_python_repo(repo_path: &Path) -> RepoClassification {
    let readme = read_text_file(&repo_path.join("README.md")).unwrap_or_default();
    let repo_class =
        if readme.to_lowercase().contains("fastapi") || readme.to_lowercase().contains("flask") {
            "service/API"
        } else {
            "library/package"
        };

    RepoClassification {
        repo_class: repo_class.to_string(),
        confidence: "0.62".to_string(),
        strategy_name: if repo_class == "service/API" {
            "python-service".to_string()
        } else {
            "python-library".to_string()
        },
        runtime_family: "python".to_string(),
        package_manager: Some("pip".to_string()),
        image: Some("python:3.11-bookworm".to_string()),
        setup_command: if repo_path.join("requirements.txt").exists() {
            Some("python -m pip install -r requirements.txt".to_string())
        } else {
            Some("python -m pip install -e .".to_string())
        },
        build_command: None,
        run_command: None,
        verify_command: if repo_path.join("tests").exists() {
            Some("pytest".to_string())
        } else {
            Some("python -m pip check".to_string())
        },
    }
}

fn detect_node_package_manager(repo_path: &Path) -> Option<String> {
    if repo_path.join("pnpm-lock.yaml").exists() {
        Some("pnpm".to_string())
    } else if repo_path.join("yarn.lock").exists() {
        Some("yarn".to_string())
    } else if repo_path.join("bun.lockb").exists() || repo_path.join("bun.lock").exists() {
        Some("bun".to_string())
    } else {
        Some("npm".to_string())
    }
}

fn package_manager_command(package_manager: &str) -> &'static str {
    match package_manager {
        "pnpm" => "pnpm",
        "yarn" => "yarn",
        "bun" => "bun",
        _ => "npm",
    }
}

fn assess_risk(repo_path: &Path) -> PreflightResult<RiskReport> {
    let mut high = Vec::new();
    let mut medium = Vec::new();

    for entry in WalkDir::new(repo_path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !should_skip_path(entry.path()))
    {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(contents) = read_text_file(entry.path()) else {
            continue;
        };
        let lower = contents.to_lowercase();

        for (token, reason) in HIGH_RISK_TOKENS {
            if lower.contains(&token.to_lowercase()) {
                high.push(format!(
                    "{reason}: {}",
                    relative_display(repo_path, entry.path())
                ));
            }
        }
        if (lower.contains("curl ") || lower.contains("curl\t")) && lower.contains("| bash") {
            high.push(format!(
                "contains curl | bash remote shell pipe: {}",
                relative_display(repo_path, entry.path())
            ));
        }
        if (lower.contains("wget ") || lower.contains("wget\t")) && lower.contains("| bash") {
            high.push(format!(
                "contains wget | bash remote shell pipe: {}",
                relative_display(repo_path, entry.path())
            ));
        }
        for (token, reason) in MEDIUM_RISK_TOKENS {
            if lower.contains(&token.to_lowercase()) {
                medium.push(format!(
                    "{reason}: {}",
                    relative_display(repo_path, entry.path())
                ));
            }
        }
    }

    if let Some(last_commit) = git_last_commit(repo_path) {
        let stale_year = last_commit
            .split('-')
            .next()
            .and_then(|year| year.parse::<i32>().ok())
            .map(|year| year <= 2022)
            .unwrap_or(false);
        if stale_year {
            medium.push(format!("latest commit looks stale ({last_commit})"));
        }
    }

    let (level, reasons) = if !high.is_empty() {
        let mut reasons = high;
        reasons.extend(medium);
        ("high".to_string(), reasons)
    } else if !medium.is_empty() {
        ("medium".to_string(), medium)
    } else {
        (
            "low".to_string(),
            vec!["no obvious static red flags found".to_string()],
        )
    };

    Ok(RiskReport { level, reasons })
}

fn apply_safe_repairs(repo_path: &Path) -> PreflightResult<Vec<String>> {
    let mut changed = Vec::new();

    for candidate in [
        ".env.example",
        ".env.sample",
        "app.env.example",
        "env.example",
    ] {
        let source = repo_path.join(candidate);
        if !source.exists() {
            continue;
        }
        let target_name = candidate
            .trim_end_matches(".example")
            .trim_end_matches(".sample");
        let target = repo_path.join(target_name);
        if target.exists() {
            continue;
        }
        let stub = build_env_stub(&fs::read_to_string(&source)?);
        fs::write(&target, stub)?;
        changed.push(relative_display(repo_path, &target));
    }

    changed.sort();
    changed.dedup();
    Ok(changed)
}

fn capture_patch_diff(repo_path: &Path) -> Option<String> {
    let _ = Command::new("git")
        .args(["add", "-N", "."])
        .current_dir(repo_path)
        .output();
    let output = Command::new("git")
        .args(["diff", "--no-ext-diff"])
        .current_dir(repo_path)
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

fn validate_command_policy(command: &str) -> PreflightResult<()> {
    let lower = command.to_lowercase();
    for needle in ["sudo ", "/users/", "rm -rf /", "chmod 777 /"] {
        if lower.contains(needle) {
            return Err(format!("sandbox command violates command policy: {needle}").into());
        }
    }
    Ok(())
}

fn build_env_stub(source: &str) -> String {
    source
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || !trimmed.contains('=') {
                return line.to_string();
            }
            let key = trimmed.split('=').next().unwrap_or_default().trim();
            format!("{key}=VORKER_PLACEHOLDER_{}", key.replace('-', "_"))
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn render_summary(report: &PreflightReport) -> String {
    let mut lines = vec![
        format!("# Preflight {}", report.run_id),
        String::new(),
        format!("- Repo: {}", report.repo_input),
        format!("- Class: {}", report.repo_class),
        format!("- Risk: {}", report.risk.level),
        format!("- Outcome: {}", report.outcome),
        format!("- Stage: {}", report.stage),
        format!(
            "- Sandbox: {} ({})",
            report.sandbox.backend, report.sandbox.state
        ),
    ];

    if let Some(failure) = &report.latest_failure {
        lines.push(format!("- Latest failure: {failure}"));
    }

    lines.push(String::new());
    lines.push("## Risks".to_string());
    lines.extend(
        report
            .risk
            .reasons
            .iter()
            .map(|reason| format!("- {reason}")),
    );
    lines.push(String::new());
    lines.push("## Commands".to_string());
    lines.extend(report.commands.iter().map(|command| {
        format!(
            "- [{}] {} (exit {})",
            command.phase, command.command, command.exit_code
        )
    }));
    lines.push(String::new());
    lines.push("## Next steps".to_string());
    lines.extend(report.next_steps.iter().map(|step| format!("- {step}")));
    lines.join("\n")
}

fn next_steps(latest_failure: Option<&str>, classification: &RepoClassification) -> Vec<String> {
    if let Some(failure) = latest_failure {
        return vec![
            format!("Review the failure: {failure}"),
            "Open the generated summary.md and report.json artifacts.".to_string(),
            "Approve execution explicitly if the repo is high risk and you still want to continue."
                .to_string(),
        ];
    }

    match classification.repo_class.as_str() {
        "web app" => vec![
            "Inspect the generated logs to confirm the expected port and startup path."
                .to_string(),
            "Run the app locally again with real secrets if external integrations are required."
                .to_string(),
        ],
        "CLI tool" => vec![
            "Open summary.md for the exact verification command that succeeded.".to_string(),
            "Re-run the CLI in the sandbox with a real sample command if you want deeper validation."
                .to_string(),
        ],
        _ => vec![
            "Review the artifacts and logs for any warnings.".to_string(),
            "Decide whether to invest in deeper manual verification.".to_string(),
        ],
    }
}

fn default_artifacts_root() -> PreflightResult<PathBuf> {
    Ok(env::current_dir()?.join(".vorker-2").join("preflight"))
}

fn write_json(path: &Path, value: &impl Serialize) -> PreflightResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

fn command_exists(program: &str) -> bool {
    Command::new("sh")
        .args(["-lc", &format!("command -v {program} >/dev/null 2>&1")])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn is_public_github_url(candidate: &str) -> bool {
    candidate.starts_with("https://github.com/") || candidate.starts_with("http://github.com/")
}

fn read_text_file(path: &Path) -> Option<String> {
    let metadata = fs::metadata(path).ok()?;
    if metadata.len() > 262_144 {
        return None;
    }
    fs::read_to_string(path).ok()
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn should_skip_path(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component.as_os_str().to_string_lossy().as_ref(),
            ".git" | "node_modules" | "target" | ".venv" | "dist" | "build"
        )
    })
}

fn first_failure(result: &SandboxResult) -> String {
    if !result.stderr.trim().is_empty() {
        result
            .stderr
            .trim()
            .lines()
            .next()
            .unwrap_or("sandbox phase failed")
            .to_string()
    } else if !result.stdout.trim().is_empty() {
        result
            .stdout
            .trim()
            .lines()
            .next()
            .unwrap_or("sandbox phase failed")
            .to_string()
    } else {
        format!("sandbox phase failed with exit code {}", result.exit_code)
    }
}

fn git_last_commit(repo_path: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["log", "-1", "--format=%cI"])
        .current_dir(repo_path)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}
