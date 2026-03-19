import { execFile } from "node:child_process";
import { access, mkdir } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

function slugify(value) {
  return String(value ?? "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 48);
}

function sanitizeSegment(value) {
  const normalized = slugify(value);
  return normalized || "task";
}

async function pathExists(targetPath) {
  try {
    await access(targetPath);
    return true;
  } catch {
    return false;
  }
}

async function runGit(args, options = {}) {
  const { stdout } = await execFileAsync("git", args, {
    cwd: options.cwd,
    env: {
      ...process.env,
      GIT_AUTHOR_NAME: process.env.GIT_AUTHOR_NAME ?? "Vorker",
      GIT_AUTHOR_EMAIL: process.env.GIT_AUTHOR_EMAIL ?? "vorker@local",
      GIT_COMMITTER_NAME: process.env.GIT_COMMITTER_NAME ?? process.env.GIT_AUTHOR_NAME ?? "Vorker",
      GIT_COMMITTER_EMAIL: process.env.GIT_COMMITTER_EMAIL ?? process.env.GIT_AUTHOR_EMAIL ?? "vorker@local",
      ...(options.env ?? {}),
    },
  });
  return stdout.trim();
}

async function gitRefExists(repoRoot, refName) {
  try {
    await execFileAsync("git", ["show-ref", "--verify", "--quiet", refName], {
      cwd: repoRoot,
    });
    return true;
  } catch {
    return false;
  }
}

export class TaskWorkspaceManager {
  constructor(options = {}) {
    this.repoRoot = path.resolve(options.repoRoot ?? process.cwd());
    this.worktreeRoot = path.resolve(options.worktreeRoot ?? path.join(this.repoRoot, ".vorker-2", "worktrees"));
  }

  async detectBaseBranch() {
    const branch = await runGit(["branch", "--show-current"], { cwd: this.repoRoot });
    return branch || "HEAD";
  }

  buildBranchName(taskId, title) {
    const taskSegment = sanitizeSegment(taskId);
    const titleSegment = sanitizeSegment(title);
    return `vorker/task-${taskSegment}${titleSegment ? `-${titleSegment}` : ""}`;
  }

  buildWorkspacePath(taskId, title) {
    const taskSegment = sanitizeSegment(taskId);
    const titleSegment = sanitizeSegment(title);
    const leaf = `${taskSegment}${titleSegment ? `-${titleSegment}` : ""}`.slice(0, 72);
    return path.join(this.worktreeRoot, leaf);
  }

  async ensureTaskWorkspace(input) {
    const baseBranch = await this.detectBaseBranch();
    const branchName = this.buildBranchName(input.taskId, input.title);
    const workspacePath = this.buildWorkspacePath(input.taskId, input.title);

    if (await pathExists(path.join(workspacePath, ".git"))) {
      return {
        repoRoot: this.repoRoot,
        workspacePath,
        branchName,
        baseBranch,
      };
    }

    await mkdir(this.worktreeRoot, { recursive: true });

    const branchExists = await gitRefExists(this.repoRoot, `refs/heads/${branchName}`);
    if (branchExists) {
      await runGit(["worktree", "add", workspacePath, branchName], { cwd: this.repoRoot });
    } else {
      await runGit(["worktree", "add", "-b", branchName, workspacePath, baseBranch], { cwd: this.repoRoot });
    }

    return {
      repoRoot: this.repoRoot,
      workspacePath,
      branchName,
      baseBranch,
    };
  }

  async listChangedFiles(workspacePath) {
    const output = await runGit(["status", "--short"], { cwd: workspacePath });
    if (!output) {
      return [];
    }

    return output
      .split("\n")
      .map((line) => line.replace(/^[ MADRCU?!]{1,2}\s+/, "").trim())
      .filter(Boolean)
      .map((filePath) => filePath.split(" -> ").at(-1) ?? filePath);
  }

  async commitTaskWorkspace(input) {
    const changedFiles = await this.listChangedFiles(input.workspacePath);
    if (changedFiles.length === 0) {
      return {
        createdCommit: false,
        commitSha: null,
        changedFiles: [],
      };
    }

    await runGit(["add", "-A"], { cwd: input.workspacePath });
    await runGit(["commit", "-m", `task(${input.taskId}): ${input.title}`], { cwd: input.workspacePath });
    const commitSha = await runGit(["rev-parse", "HEAD"], { cwd: input.workspacePath });

    return {
      createdCommit: true,
      commitSha,
      changedFiles,
    };
  }
}
