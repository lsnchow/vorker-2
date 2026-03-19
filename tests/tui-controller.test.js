import test from "node:test";
import assert from "node:assert/strict";

import { executeCommand } from "../src/tui/controller.js";

test("executeCommand creates and selects agents and runs", async () => {
  const calls = [];

  const manager = {
    async createAgent(input) {
      calls.push(["createAgent", input]);
      return { id: "agent-1", name: input.name };
    },
    async promptAgent(agentId, text) {
      calls.push(["promptAgent", agentId, text]);
      return { responseText: "ok" };
    },
  };

  const orchestrator = {
    createRun(input) {
      calls.push(["createRun", input]);
      return { id: "run-1", name: input.name };
    },
    async mergeCompletedTasks(runId) {
      calls.push(["mergeCompletedTasks", runId]);
      return [];
    },
    async mergeTask(taskId) {
      calls.push(["mergeTask", taskId]);
      return { id: taskId, mergeStatus: "merged" };
    },
  };

  const tunnelManager = {
    async start() {
      calls.push(["share.start"]);
    },
  };

  const state = { activeSessionId: null, activeRunId: null, statusLine: "" };

  await executeCommand({ type: "agent.create", name: "Planner" }, { manager, orchestrator, tunnelManager, state });
  await executeCommand({ type: "run.create", name: "Bootstrap", goal: "Build core" }, { manager, orchestrator, tunnelManager, state });
  await executeCommand({ type: "prompt.send", text: "Plan the work" }, { manager, orchestrator, tunnelManager, state });
  await executeCommand({ type: "run.merge", runId: null }, { manager, orchestrator, tunnelManager, state });
  await executeCommand({ type: "task.merge", taskId: "task-1" }, { manager, orchestrator, tunnelManager, state });
  await executeCommand({ type: "share.start" }, { manager, orchestrator, tunnelManager, state });

  assert.equal(state.activeSessionId, "agent-1");
  assert.equal(state.activeRunId, "run-1");
  assert.deepEqual(calls, [
    ["createAgent", { name: "Planner" }],
    ["createRun", { name: "Bootstrap", goal: "Build core" }],
    ["promptAgent", "agent-1", "Plan the work"],
    ["mergeCompletedTasks", "run-1"],
    ["mergeTask", "task-1"],
    ["share.start"],
  ]);
});
