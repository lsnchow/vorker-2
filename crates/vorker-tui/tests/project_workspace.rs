use std::fs;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

use vorker_tui::{
    ProjectWorkspace, RowKind, ThreadStore, TranscriptRow, render_project_confirmation,
};

fn unique_temp_dir(name: &str) -> std::path::PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    std::env::temp_dir().join(format!("vorker-project-{name}-{suffix}"))
}

#[test]
fn project_workspace_maps_a_directory_to_a_scoped_store_under_vorker_home() {
    let root = unique_temp_dir("root");
    let cwd = root.join("repos").join("hyperloop-pod");
    fs::create_dir_all(&cwd).expect("create cwd");

    let workspace = ProjectWorkspace::at_root(root.clone(), &cwd).expect("workspace");

    assert!(
        workspace.project_dir().starts_with(root.join("projects")),
        "project dir should live under ~/.vorker/projects:\n{}",
        workspace.project_dir().display()
    );
    assert!(
        workspace.threads_path().ends_with("threads.json"),
        "missing threads.json path: {}",
        workspace.threads_path().display()
    );
    assert!(
        workspace.events_dir().ends_with("events"),
        "missing events dir path: {}",
        workspace.events_dir().display()
    );
    assert!(
        !workspace.is_confirmed(),
        "new workspace should require confirmation"
    );
}

#[test]
fn project_workspace_rejects_corrupt_meta_instead_of_replacing_it() {
    let root = unique_temp_dir("corrupt-meta");
    let cwd = root.join("repos").join("hyperloop-pod");
    fs::create_dir_all(&cwd).expect("create cwd");

    let workspace = ProjectWorkspace::at_root(root.clone(), &cwd).expect("workspace");
    fs::create_dir_all(workspace.project_dir()).expect("project dir");
    fs::write(workspace.meta_path(), "{not-json").expect("corrupt meta");

    let error = match ProjectWorkspace::at_root(root, &cwd) {
        Ok(_) => panic!("corrupt meta should fail"),
        Err(error) => error,
    };

    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn list_all_threads_reads_threads_across_multiple_project_workspaces() {
    let root = unique_temp_dir("aggregate");
    let repo_a = root.join("repos").join("alpha");
    let repo_b = root.join("repos").join("beta");
    fs::create_dir_all(&repo_a).expect("repo a");
    fs::create_dir_all(&repo_b).expect("repo b");

    let workspace_a = ProjectWorkspace::at_root(root.clone(), &repo_a).expect("workspace a");
    let workspace_b = ProjectWorkspace::at_root(root.clone(), &repo_b).expect("workspace b");
    workspace_a.confirm().expect("confirm a");
    workspace_b.confirm().expect("confirm b");

    let mut store_a = ThreadStore::open_at(workspace_a.threads_path()).expect("store a");
    let mut thread_a = store_a.create_thread(&repo_a);
    thread_a.name = "Alpha thread".to_string();
    thread_a.rows.push(TranscriptRow {
        kind: RowKind::User,
        text: "alpha".to_string(),
        detail: None,
    });
    store_a.upsert(thread_a.clone()).expect("save a");

    let mut store_b = ThreadStore::open_at(workspace_b.threads_path()).expect("store b");
    let mut thread_b = store_b.create_thread(&repo_b);
    thread_b.name = "Beta thread".to_string();
    store_b.upsert(thread_b.clone()).expect("save b");

    let threads = ProjectWorkspace::list_all_threads_under(root.clone()).expect("aggregate list");
    assert_eq!(threads.len(), 2);
    assert!(
        threads
            .iter()
            .any(|thread| thread.name == "Alpha thread"
                && thread.cwd == repo_a.display().to_string())
    );
    assert!(
        threads.iter().any(
            |thread| thread.name == "Beta thread" && thread.cwd == repo_b.display().to_string()
        )
    );
}

#[test]
fn confirmation_screen_mentions_directory_and_workspace() {
    let output = render_project_confirmation(
        100,
        "/Users/lucas/Downloads",
        "~/.vorker/projects/downloads-1234",
        false,
    );

    assert!(
        output.contains("Use Vorker in this directory?"),
        "missing confirmation title:\n{output}"
    );
    assert!(
        output.contains("/Users/lucas/Downloads"),
        "missing cwd:\n{output}"
    );
    assert!(
        output.contains("~/.vorker/projects/downloads-1234"),
        "missing workspace path:\n{output}"
    );
    assert!(
        output.contains("Enter to continue"),
        "missing key hint:\n{output}"
    );
}
