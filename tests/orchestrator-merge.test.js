import test from "node:test";
import assert from "node:assert/strict";

import { Orchestrator } from "../src/orchestrator.js";

test("Orchestrator mergeTask records merge metadata for a completed task", async () => {
  const calls = [];
  const orchestrator = new Orchestrator({
    manager: {
      defaults: {},
    },
    workspaceManager: {
      async mergeTaskBranch(input) {
        calls.push(input);
        return {
          status: "merged",
          mergeCommitSha: "def456",
        };
      },
    },
  });

  const run = orchestrator.createRun({
    name: "Merge",
    goal: "Merge a finished task",
  });
  const task = orchestrator.createTask({
    runId: run.id,
    title: "Task",
    description: "Done",
    status: "completed",
    branchName: "vorker/task-task",
    baseBranch: "main",
  });

  const merged = await orchestrator.mergeTask(task.id);

  assert.equal(merged.mergeStatus, "merged");
  assert.equal(merged.mergeCommitSha, "def456");
  assert.deepEqual(calls, [
    {
      branchName: "vorker/task-task",
      baseBranch: "main",
    },
  ]);
});
