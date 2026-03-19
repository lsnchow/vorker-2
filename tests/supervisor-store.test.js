import test from "node:test";
import assert from "node:assert/strict";

import { SupervisorStore } from "../src/supervisor/store.js";
import { createSupervisorEvent } from "../src/supervisor/events.js";

test("SupervisorStore rebuilds run, task, and session state from events", () => {
  const store = new SupervisorStore();

  store.append(
    createSupervisorEvent("run.created", {
      run: {
        id: "run-1",
        name: "Bootstrap",
        goal: "Clone Claude Code ergonomics on top of Copilot",
        status: "draft",
        workerAgentIds: [],
        createdAt: "2026-03-19T00:00:00.000Z",
        updatedAt: "2026-03-19T00:00:00.000Z",
      },
    }),
  );

  store.append(
    createSupervisorEvent("session.registered", {
      session: {
        id: "agent-1",
        name: "Arbitrator",
        role: "arbitrator",
        status: "ready",
        mode: "plan",
        model: "gpt-5.4",
        cwd: "/workspace",
      },
    }),
  );

  store.append(
    createSupervisorEvent("task.created", {
      task: {
        id: "task-1",
        runId: "run-1",
        parentTaskId: null,
        title: "Design the supervisor core",
        description: "Establish the canonical event model and state reducer.",
        status: "ready",
        assignedAgentId: "agent-1",
        createdAt: "2026-03-19T00:01:00.000Z",
        updatedAt: "2026-03-19T00:01:00.000Z",
      },
    }),
  );

  store.append(
    createSupervisorEvent("session.prompt.finished", {
      sessionId: "agent-1",
      message: {
        role: "assistant",
        text: "Supervisor core planned.",
      },
    }),
  );

  const snapshot = store.snapshot();
  const run = snapshot.runs[0];
  const task = snapshot.tasks[0];
  const session = snapshot.sessions[0];

  assert.equal(run.id, "run-1");
  assert.equal(run.tasks.length, 1);
  assert.equal(run.tasks[0].id, "task-1");
  assert.equal(task.assignedAgentId, "agent-1");
  assert.equal(session.transcript.length, 1);
  assert.deepEqual(session.transcript[0], {
    role: "assistant",
    text: "Supervisor core planned.",
  });
});

test("SupervisorStore applies task updates and preserves parent-child task relationships", () => {
  const store = new SupervisorStore();

  store.append(
    createSupervisorEvent("run.created", {
      run: {
        id: "run-2",
        name: "Parallel work",
        goal: "Split orchestration and UI",
        status: "running",
        workerAgentIds: ["agent-2"],
        createdAt: "2026-03-19T00:00:00.000Z",
        updatedAt: "2026-03-19T00:00:00.000Z",
      },
    }),
  );

  store.append(
    createSupervisorEvent("task.created", {
      task: {
        id: "task-parent",
        runId: "run-2",
        parentTaskId: null,
        title: "Parent task",
        description: "Root scope",
        status: "running",
        createdAt: "2026-03-19T00:00:00.000Z",
        updatedAt: "2026-03-19T00:00:00.000Z",
      },
    }),
  );

  store.append(
    createSupervisorEvent("task.created", {
      task: {
        id: "task-child",
        runId: "run-2",
        parentTaskId: "task-parent",
        title: "Child task",
        description: "Leaf scope",
        status: "ready",
        createdAt: "2026-03-19T00:02:00.000Z",
        updatedAt: "2026-03-19T00:02:00.000Z",
      },
    }),
  );

  store.append(
    createSupervisorEvent("task.updated", {
      task: {
        id: "task-child",
        runId: "run-2",
        parentTaskId: "task-parent",
        title: "Child task",
        description: "Leaf scope",
        status: "completed",
        templateAgentId: "template-1",
        executionAgentId: "exec-1",
        workspacePath: "/repo/.vorker-2/worktrees/task-child",
        branchName: "vorker/task-task-child",
        baseBranch: "main",
        commitSha: "abc123",
        changeCount: 1,
        changedFiles: ["src/index.js"],
        outputText: "done",
        createdAt: "2026-03-19T00:02:00.000Z",
        updatedAt: "2026-03-19T00:03:00.000Z",
      },
    }),
  );

  const snapshot = store.snapshot();
  const run = snapshot.runs[0];
  const parent = run.tasks.find((entry) => entry.id === "task-parent");
  const child = run.tasks.find((entry) => entry.id === "task-child");

  assert.equal(parent.parentTaskId, null);
  assert.equal(child.parentTaskId, "task-parent");
  assert.equal(child.status, "completed");
  assert.equal(child.outputText, "done");
  assert.equal(child.templateAgentId, "template-1");
  assert.equal(child.executionAgentId, "exec-1");
  assert.equal(child.workspacePath, "/repo/.vorker-2/worktrees/task-child");
  assert.equal(child.branchName, "vorker/task-task-child");
  assert.equal(child.baseBranch, "main");
  assert.equal(child.commitSha, "abc123");
  assert.equal(child.changeCount, 1);
  assert.deepEqual(child.changedFiles, ["src/index.js"]);
});
