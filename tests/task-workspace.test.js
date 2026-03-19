import test from "node:test";
import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import { mkdtemp, writeFile, mkdir } from "node:fs/promises";
import { execFile } from "node:child_process";
import { promisify } from "node:util";

import { TaskWorkspaceManager } from "../src/git/task-workspace.js";

const execFileAsync = promisify(execFile);

async function git(cwd, ...args) {
  const { stdout } = await execFileAsync("git", args, { cwd });
  return stdout.trim();
}

async function createRepo() {
  const repoRoot = await mkdtemp(path.join(os.tmpdir(), "vorker-2-repo-"));
  await git(repoRoot, "init", "-b", "main");
  await git(repoRoot, "config", "user.name", "Vorker Test");
  await git(repoRoot, "config", "user.email", "vorker@example.com");
  await mkdir(path.join(repoRoot, "src"), { recursive: true });
  await writeFile(path.join(repoRoot, "src", "index.js"), "export const value = 1;\n", "utf8");
  await git(repoRoot, "add", ".");
  await git(repoRoot, "commit", "-m", "init");
  return repoRoot;
}

test("TaskWorkspaceManager creates an isolated git worktree and branch for a task", async () => {
  const repoRoot = await createRepo();
  const manager = new TaskWorkspaceManager({ repoRoot });

  const workspace = await manager.ensureTaskWorkspace({
    runId: "run-1",
    taskId: "task-1",
    title: "Implement isolated dispatch",
  });

  const branchName = await git(workspace.workspacePath, "branch", "--show-current");

  assert.match(workspace.branchName, /^vorker\/task-task-1-implement-isolated-dispatch/);
  assert.equal(branchName, workspace.branchName);
  assert.equal(workspace.repoRoot, repoRoot);
  assert.equal(workspace.baseBranch, "main");
});

test("TaskWorkspaceManager reuses an existing task worktree on repeated calls", async () => {
  const repoRoot = await createRepo();
  const manager = new TaskWorkspaceManager({ repoRoot });

  const first = await manager.ensureTaskWorkspace({
    runId: "run-1",
    taskId: "task-2",
    title: "Reuse workspace",
  });
  const second = await manager.ensureTaskWorkspace({
    runId: "run-1",
    taskId: "task-2",
    title: "Reuse workspace",
  });

  assert.equal(second.workspacePath, first.workspacePath);
  assert.equal(second.branchName, first.branchName);
});

test("TaskWorkspaceManager commits task workspace changes into the task branch", async () => {
  const repoRoot = await createRepo();
  const manager = new TaskWorkspaceManager({ repoRoot });
  const workspace = await manager.ensureTaskWorkspace({
    runId: "run-1",
    taskId: "task-3",
    title: "Commit workspace",
  });

  await writeFile(path.join(workspace.workspacePath, "src", "index.js"), "export const value = 2;\n", "utf8");

  const summary = await manager.commitTaskWorkspace({
    workspacePath: workspace.workspacePath,
    taskId: "task-3",
    title: "Commit workspace",
  });
  const lastMessage = await git(workspace.workspacePath, "log", "-1", "--pretty=%s");

  assert.equal(summary.createdCommit, true);
  assert.ok(summary.commitSha);
  assert.deepEqual(summary.changedFiles, ["src/index.js"]);
  assert.match(lastMessage, /task-3/);
});
