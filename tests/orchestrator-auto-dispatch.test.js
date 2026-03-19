import test from "node:test";
import assert from "node:assert/strict";

import { Orchestrator } from "../src/orchestrator.js";

test("Orchestrator autoDispatchReadyTasks starts ready tasks in parallel and returns settled results", async () => {
  const orchestrator = new Orchestrator({
    manager: {
      defaults: {},
    },
    workspaceManager: {
      async ensureTaskWorkspace() {
        return {
          repoRoot: "/repo",
          workspacePath: "/repo/worktree",
          branchName: "vorker/task",
          baseBranch: "main",
        };
      },
    },
  });

  const run = orchestrator.createRun({
    name: "Parallel",
    goal: "Fan out ready tasks",
    workerAgentIds: ["worker-1", "worker-2"],
  });

  const taskA = orchestrator.createTask({
    runId: run.id,
    title: "Task A",
    description: "A",
    status: "ready",
  });
  const taskB = orchestrator.createTask({
    runId: run.id,
    title: "Task B",
    description: "B",
    status: "ready",
  });

  const started = [];
  orchestrator.dispatchTask = async (taskId) => {
    started.push(taskId);
    await new Promise((resolve) => setTimeout(resolve, taskId === taskA.id ? 30 : 5));
    if (taskId === taskA.id) {
      throw new Error("boom");
    }
    return { id: taskId, status: "completed" };
  };

  const pending = orchestrator.autoDispatchReadyTasks(run.id);
  await new Promise((resolve) => setTimeout(resolve, 1));

  assert.deepEqual(started.sort(), [taskA.id, taskB.id].sort());

  const results = await pending;
  assert.equal(results.length, 2);
  assert.equal(results.filter((entry) => entry.status === "fulfilled").length, 1);
  assert.equal(results.filter((entry) => entry.status === "rejected").length, 1);
});
