use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::tempdir;
use vorker_git::TaskWorkspaceManager;

fn git(cwd: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf8")
        .trim()
        .to_string()
}

fn create_repo() -> PathBuf {
    let repo_root = tempdir().expect("tempdir").keep();
    git(&repo_root, &["init", "-b", "main"]);
    git(&repo_root, &["config", "user.name", "Vorker Test"]);
    git(&repo_root, &["config", "user.email", "vorker@example.com"]);
    fs::create_dir_all(repo_root.join("src")).expect("src dir");
    fs::write(repo_root.join("src/index.js"), "export const value = 1;\n").expect("seed file");
    git(&repo_root, &["add", "."]);
    git(&repo_root, &["commit", "-m", "init"]);
    repo_root
}

#[test]
fn creates_an_isolated_git_worktree_and_branch_for_a_task() {
    let repo_root = create_repo();
    let manager = TaskWorkspaceManager::new(repo_root.clone(), None);

    let workspace = manager
        .ensure_task_workspace("task-1", "Implement isolated dispatch")
        .expect("workspace created");

    let branch_name = git(
        Path::new(&workspace.workspace_path),
        &["branch", "--show-current"],
    );

    assert!(
        workspace
            .branch_name
            .starts_with("vorker/task-task-1-implement-isolated-dispatch")
    );
    assert_eq!(branch_name, workspace.branch_name);
    assert_eq!(workspace.repo_root, repo_root.to_string_lossy());
    assert_eq!(workspace.base_branch, "main");
}

#[test]
fn reuses_an_existing_task_worktree_on_repeated_calls() {
    let repo_root = create_repo();
    let manager = TaskWorkspaceManager::new(repo_root, None);

    let first = manager
        .ensure_task_workspace("task-2", "Reuse workspace")
        .expect("first workspace");
    let second = manager
        .ensure_task_workspace("task-2", "Reuse workspace")
        .expect("second workspace");

    assert_eq!(second.workspace_path, first.workspace_path);
    assert_eq!(second.branch_name, first.branch_name);
}

#[test]
fn commits_task_workspace_changes_into_the_task_branch() {
    let repo_root = create_repo();
    let manager = TaskWorkspaceManager::new(repo_root, None);
    let workspace = manager
        .ensure_task_workspace("task-3", "Commit workspace")
        .expect("workspace");

    fs::write(
        Path::new(&workspace.workspace_path).join("src/index.js"),
        "export const value = 2;\n",
    )
    .expect("updated file");

    let summary = manager
        .commit_task_workspace(&workspace.workspace_path, "task-3", "Commit workspace")
        .expect("commit task workspace");
    let last_message = git(
        Path::new(&workspace.workspace_path),
        &["log", "-1", "--pretty=%s"],
    );

    assert!(summary.created_commit);
    assert!(summary.commit_sha.is_some());
    assert_eq!(summary.changed_files, vec!["src/index.js"]);
    assert!(last_message.contains("task-3"));
}

#[test]
fn merges_a_committed_task_branch_back_into_the_base_branch() {
    let repo_root = create_repo();
    let manager = TaskWorkspaceManager::new(repo_root.clone(), None);
    let workspace = manager
        .ensure_task_workspace("task-merge", "Merge task branch")
        .expect("workspace");

    fs::write(
        Path::new(&workspace.workspace_path).join("src/index.js"),
        "export const value = 2;\n",
    )
    .expect("updated file");
    manager
        .commit_task_workspace(&workspace.workspace_path, "task-merge", "Merge task branch")
        .expect("commit");

    let result = manager
        .merge_task_branch(&workspace.branch_name, &workspace.base_branch)
        .expect("merge result");
    let merged_file = fs::read_to_string(repo_root.join("src/index.js")).expect("merged file");

    assert_eq!(result.status, "merged");
    assert!(result.merge_commit_sha.is_some());
    assert!(merged_file.contains("value = 2"));
}
