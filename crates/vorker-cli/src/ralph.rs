use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RalphLaunchRequest {
    pub cwd: PathBuf,
    pub user_home: PathBuf,
    pub task: String,
    pub model: Option<String>,
    pub no_deslop: bool,
    pub no_alt_screen: bool,
    pub xhigh: bool,
    pub extra_args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RalphLaunchPlan {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: BTreeMap<String, String>,
}

pub fn build_ralph_launch(request: RalphLaunchRequest) -> io::Result<RalphLaunchPlan> {
    let task = request.task.trim().to_string();
    if task.is_empty() {
        return Err(io::Error::other("ralph requires a task"));
    }

    let codex_home = resolve_ralph_codex_home(&request.cwd, &request.user_home);
    let mut env = BTreeMap::new();
    if let Some(codex_home) = codex_home {
        env.insert(
            "CODEX_HOME".to_string(),
            codex_home.to_string_lossy().to_string(),
        );
    }
    env.insert("TERM".to_string(), "xterm-256color".to_string());

    let mut args = vec!["ralph".to_string()];
    if request.no_deslop {
        args.push("--no-deslop".to_string());
    }
    if request.no_alt_screen {
        args.push("--no-alt-screen".to_string());
    }
    if request.xhigh {
        args.push("--xhigh".to_string());
    }
    if let Some(model) = request.model.filter(|model| !model.trim().is_empty()) {
        args.push("--model".to_string());
        args.push(model);
    }
    args.extend(request.extra_args);
    args.push(task);

    Ok(RalphLaunchPlan {
        program: "omx".to_string(),
        args,
        cwd: request.cwd,
        env,
    })
}

fn resolve_ralph_codex_home(cwd: &Path, user_home: &Path) -> Option<PathBuf> {
    let project_codex = cwd.join(".codex");
    if project_codex.join("auth.json").exists() {
        return Some(project_codex);
    }

    let user_codex = user_home.join(".codex");
    if user_codex.join("auth.json").exists() {
        return Some(user_codex);
    }

    project_codex.exists().then_some(project_codex)
}

pub fn run_ralph_launch(plan: &RalphLaunchPlan) -> io::Result<ExitStatus> {
    Command::new(&plan.program)
        .args(&plan.args)
        .current_dir(&plan.cwd)
        .envs(&plan.env)
        .status()
}
