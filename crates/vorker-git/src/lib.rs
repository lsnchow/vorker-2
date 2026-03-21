use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

type GitResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskWorkspace {
    pub repo_root: String,
    pub workspace_path: String,
    pub branch_name: String,
    pub base_branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitSummary {
    pub created_commit: bool,
    pub commit_sha: Option<String>,
    pub changed_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeSummary {
    pub status: String,
    pub merge_commit_sha: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TaskWorkspaceManager {
    repo_root: PathBuf,
    worktree_root: PathBuf,
}

impl TaskWorkspaceManager {
    #[must_use]
    pub fn new(repo_root: PathBuf, worktree_root: Option<PathBuf>) -> Self {
        let worktree_root =
            worktree_root.unwrap_or_else(|| repo_root.join(".vorker-2").join("worktrees"));
        Self {
            repo_root,
            worktree_root,
        }
    }

    pub fn detect_base_branch(&self) -> GitResult<String> {
        let branch = run_git(&self.repo_root, &["branch", "--show-current"])?;
        if branch.is_empty() {
            Ok("HEAD".to_string())
        } else {
            Ok(branch)
        }
    }

    #[must_use]
    pub fn build_branch_name(&self, task_id: &str, title: &str) -> String {
        let task_segment = sanitize_segment(task_id);
        let title_segment = sanitize_segment(title);
        if title_segment.is_empty() {
            format!("vorker/task-{task_segment}")
        } else {
            format!("vorker/task-{task_segment}-{title_segment}")
        }
    }

    #[must_use]
    pub fn build_workspace_path(&self, task_id: &str, title: &str) -> PathBuf {
        let task_segment = sanitize_segment(task_id);
        let title_segment = sanitize_segment(title);
        let leaf = if title_segment.is_empty() {
            task_segment
        } else {
            format!("{task_segment}-{title_segment}")
        };
        self.worktree_root.join(truncate_chars(&leaf, 72))
    }

    pub fn ensure_task_workspace(&self, task_id: &str, title: &str) -> GitResult<TaskWorkspace> {
        let base_branch = self.detect_base_branch()?;
        let branch_name = self.build_branch_name(task_id, title);
        let workspace_path = self.build_workspace_path(task_id, title);

        if workspace_path.join(".git").exists() {
            return Ok(TaskWorkspace {
                repo_root: self.repo_root.display().to_string(),
                workspace_path: workspace_path.display().to_string(),
                branch_name,
                base_branch,
            });
        }

        fs::create_dir_all(&self.worktree_root)?;

        if git_ref_exists(&self.repo_root, &format!("refs/heads/{branch_name}"))? {
            run_git(
                &self.repo_root,
                &[
                    "worktree",
                    "add",
                    workspace_path.to_string_lossy().as_ref(),
                    branch_name.as_str(),
                ],
            )?;
        } else {
            run_git(
                &self.repo_root,
                &[
                    "worktree",
                    "add",
                    "-b",
                    branch_name.as_str(),
                    workspace_path.to_string_lossy().as_ref(),
                    base_branch.as_str(),
                ],
            )?;
        }

        Ok(TaskWorkspace {
            repo_root: self.repo_root.display().to_string(),
            workspace_path: workspace_path.display().to_string(),
            branch_name,
            base_branch,
        })
    }

    pub fn list_changed_files(&self, workspace_path: &str) -> GitResult<Vec<String>> {
        let output = run_git(Path::new(workspace_path), &["status", "--short"])?;
        if output.is_empty() {
            return Ok(Vec::new());
        }

        Ok(output
            .lines()
            .filter_map(|line| {
                let trimmed =
                    line.trim_start_matches([' ', 'M', 'A', 'D', 'R', 'C', 'U', '?', '!']);
                let normalized = trimmed.trim();
                if normalized.is_empty() {
                    None
                } else {
                    Some(
                        normalized
                            .split(" -> ")
                            .last()
                            .unwrap_or(normalized)
                            .to_string(),
                    )
                }
            })
            .collect())
    }

    pub fn commit_task_workspace(
        &self,
        workspace_path: &str,
        task_id: &str,
        title: &str,
    ) -> GitResult<CommitSummary> {
        let changed_files = self.list_changed_files(workspace_path)?;
        if changed_files.is_empty() {
            return Ok(CommitSummary {
                created_commit: false,
                commit_sha: None,
                changed_files: Vec::new(),
            });
        }

        run_git(Path::new(workspace_path), &["add", "-A"])?;
        run_git(
            Path::new(workspace_path),
            &["commit", "-m", &format!("task({task_id}): {title}")],
        )?;
        let commit_sha = run_git(Path::new(workspace_path), &["rev-parse", "HEAD"])?;

        Ok(CommitSummary {
            created_commit: true,
            commit_sha: Some(commit_sha),
            changed_files,
        })
    }

    pub fn merge_task_branch(
        &self,
        branch_name: &str,
        base_branch: &str,
    ) -> GitResult<MergeSummary> {
        let current_branch = self.detect_base_branch()?;
        if current_branch != base_branch {
            return Err(format!(
                "Cannot merge {branch_name} while repo root is on {current_branch}; expected {base_branch}."
            )
            .into());
        }

        match run_git(
            &self.repo_root,
            &["merge", "--no-ff", "--no-edit", branch_name],
        ) {
            Ok(_) => Ok(MergeSummary {
                status: "merged".to_string(),
                merge_commit_sha: Some(run_git(&self.repo_root, &["rev-parse", "HEAD"])?),
                message: None,
            }),
            Err(error) => {
                let _ = run_git(&self.repo_root, &["merge", "--abort"]);
                Ok(MergeSummary {
                    status: "conflict".to_string(),
                    merge_commit_sha: None,
                    message: Some(error.to_string()),
                })
            }
        }
    }
}

fn git_ref_exists(repo_root: &Path, ref_name: &str) -> GitResult<bool> {
    let output = Command::new("git")
        .args(["show-ref", "--verify", "--quiet", ref_name])
        .current_dir(repo_root)
        .output()?;
    Ok(output.status.success())
}

fn run_git(cwd: &Path, args: &[&str]) -> GitResult<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env(
            "GIT_AUTHOR_NAME",
            env::var("GIT_AUTHOR_NAME").unwrap_or_else(|_| "Vorker".to_string()),
        )
        .env(
            "GIT_AUTHOR_EMAIL",
            env::var("GIT_AUTHOR_EMAIL").unwrap_or_else(|_| "vorker@local".to_string()),
        )
        .env(
            "GIT_COMMITTER_NAME",
            env::var("GIT_COMMITTER_NAME")
                .or_else(|_| env::var("GIT_AUTHOR_NAME"))
                .unwrap_or_else(|_| "Vorker".to_string()),
        )
        .env(
            "GIT_COMMITTER_EMAIL",
            env::var("GIT_COMMITTER_EMAIL")
                .or_else(|_| env::var("GIT_AUTHOR_EMAIL"))
                .unwrap_or_else(|_| "vorker@local".to_string()),
        )
        .output()?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr)
            .trim()
            .to_string()
            .into());
    }

    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn sanitize_segment(value: &str) -> String {
    let normalized = slugify(value);
    if normalized.is_empty() {
        "task".to_string()
    } else {
        normalized
    }
}

fn slugify(value: &str) -> String {
    let mut output = String::new();
    let mut last_dash = false;

    for ch in value.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            output.push(ch);
            last_dash = false;
        } else if !last_dash && !output.is_empty() {
            output.push('-');
            last_dash = true;
        }
        if output.chars().count() >= 48 {
            break;
        }
    }

    output.trim_matches('-').to_string()
}

fn truncate_chars(value: &str, max_len: usize) -> String {
    value.chars().take(max_len).collect()
}
