import test from "node:test";
import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import { mkdtemp, writeFile, mkdir, readFile } from "node:fs/promises";
import { execFile } from "node:child_process";
import { promisify } from "node:util";

import { TaskWorkspaceManager } from "../src/git/task-workspace.js";

const execFileAsync = promisify(execFile);

async function git(cwd, ...args) {
  const { stdout } = await execFileAsync("git", args, { cwd });
  return stdout.trim();
}

async function createRepo() {
  const repoRoot = await mkdtemp(path.join(os.tmpdir(), "vorker-2-merge-"));
  await git(repoRoot, "init", "-b", "main");
  await git(repoRoot, "config", "user.name", "Vorker Test");
  await git(repoRoot, "config", "user.email", "vorker@example.com");
  await mkdir(path.join(repoRoot, "src"), { recursive: true });
  await writeFile(path.join(repoRoot, "src", "index.js"), "export const value = 1;\n", "utf8");
  await git(repoRoot, "add", ".");
  await git(repoRoot, "commit", "-m", "init");
  return repoRoot;
}

test("TaskWorkspaceManager merges a committed task branch back into the base branch", async () => {
  const repoRoot = await createRepo();
  const manager = new TaskWorkspaceManager({ repoRoot });
  const workspace = await manager.ensureTaskWorkspace({
    runId: "run-1",
    taskId: "task-merge",
    title: "Merge task branch",
  });

  await writeFile(path.join(workspace.workspacePath, "src", "index.js"), "export const value = 2;\n", "utf8");
  await manager.commitTaskWorkspace({
    workspacePath: workspace.workspacePath,
    taskId: "task-merge",
    title: "Merge task branch",
  });

  const result = await manager.mergeTaskBranch({
    branchName: workspace.branchName,
    baseBranch: workspace.baseBranch,
  });
  const mergedFile = await readFile(path.join(repoRoot, "src", "index.js"), "utf8");

  assert.equal(result.status, "merged");
  assert.ok(result.mergeCommitSha);
  assert.match(mergedFile, /value = 2/);
});
