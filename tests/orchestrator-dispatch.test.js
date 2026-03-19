import test from "node:test";
import assert from "node:assert/strict";

import { Orchestrator } from "../src/orchestrator.js";

test("Orchestrator dispatchTask provisions an isolated task workspace and task-specific agent", async () => {
  const calls = [];
  const workspaceManager = {
    async ensureTaskWorkspace(input) {
      calls.push(["ensureTaskWorkspace", input]);
      return {
        repoRoot: "/repo",
        workspacePath: "/repo/.vorker-2/worktrees/task-1",
        branchName: "vorker/task-task-1-wire-event-bus",
        baseBranch: "main",
      };
    },
    async commitTaskWorkspace(input) {
      calls.push(["commitTaskWorkspace", input]);
      return {
        createdCommit: true,
        commitSha: "abc123",
        changedFiles: ["src/index.js"],
      };
    },
  };

  const manager = {
    skillCatalog: {
      listSkills() {
        return [];
      },
    },
    listAgents() {
      return [
        {
          id: "template-1",
          name: "Worker 1",
          role: "worker",
          notes: "Template worker",
          skillIds: ["skill-a"],
          autoApprove: true,
          mode: "https://agentclientprotocol.com/protocol/session-modes#agent",
          model: "gpt-5.4",
        },
      ];
    },
    async createAgent(input) {
      calls.push(["createAgent", input]);
      return {
        id: "exec-1",
        snapshot() {
          return { id: "exec-1", ...input };
        },
      };
    },
    async promptAgent(agentId, text, options) {
      calls.push(["promptAgent", agentId, text, options]);
      return { responseText: "done" };
    },
  };

  const orchestrator = new Orchestrator({
    manager,
    workspaceManager,
  });

  const run = orchestrator.createRun({
    name: "Bootstrap",
    goal: "Build isolated execution",
    workerAgentIds: ["template-1"],
  });
  orchestrator.createTask({
    runId: run.id,
    title: "Wire event bus",
    description: "Create the first isolated worker flow.",
    status: "ready",
  });

  const [task] = orchestrator.snapshotRun(run.id).tasks;
  const result = await orchestrator.dispatchTask(task.id);

  assert.equal(result.status, "completed");
  assert.equal(result.templateAgentId, "template-1");
  assert.equal(result.executionAgentId, "exec-1");
  assert.equal(result.workspacePath, "/repo/.vorker-2/worktrees/task-1");
  assert.equal(result.branchName, "vorker/task-task-1-wire-event-bus");
  assert.equal(result.commitSha, "abc123");
  assert.equal(result.changeCount, 1);

  assert.deepEqual(calls[0], [
    "ensureTaskWorkspace",
    {
      runId: run.id,
      taskId: task.id,
      title: "Wire event bus",
    },
  ]);

  assert.equal(calls[1][0], "createAgent");
  assert.equal(calls[1][1].cwd, "/repo/.vorker-2/worktrees/task-1");
  assert.match(calls[1][1].name, /Wire event bus/);

  assert.equal(calls[2][0], "promptAgent");
  assert.equal(calls[2][1], "exec-1");
  assert.match(calls[2][3].contextSections.join("\n"), /Branch: vorker\/task-task-1-wire-event-bus/);
  assert.match(calls[2][3].contextSections.join("\n"), /Workspace: \/repo\/.vorker-2\/worktrees\/task-1/);
  assert.deepEqual(calls[3], [
    "commitTaskWorkspace",
    {
      workspacePath: "/repo/.vorker-2/worktrees/task-1",
      taskId: task.id,
      title: "Wire event bus",
    },
  ]);
});
