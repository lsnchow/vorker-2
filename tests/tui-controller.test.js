import test from "node:test";
import assert from "node:assert/strict";

import { executeCommand } from "../src/tui/controller.js";

test("executeCommand quick-creates an agent with the selected model", async () => {
  const calls = [];

  const manager = {
    listAgents() {
      return [];
    },
    async createAgent(input) {
      calls.push(["createAgent", input]);
      return { id: "agent-1", name: input.name };
    },
  };

  const orchestrator = {};
  const tunnelManager = {};
  const state = { activeSessionId: null, activeRunId: null, statusLine: "" };

  await executeCommand(
    { type: "agent.quickCreate", model: "gpt-5.4" },
    { manager, orchestrator, tunnelManager, state, options: {} },
  );

  assert.equal(state.activeSessionId, "agent-1");
  assert.deepEqual(calls, [["createAgent", { name: "Agent 1", model: "gpt-5.4" }]]);
});

test("executeCommand launches a swarm with the selected model", async () => {
  const calls = [];
  let createdAgents = [];

  const manager = {
    listAgents() {
      return createdAgents;
    },
    async createAgent(input) {
      const agent = { id: `agent-${createdAgents.length + 1}`, name: input.name, model: input.model };
      createdAgents = [...createdAgents, agent];
      calls.push(["createAgent", input]);
      return agent;
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
    updateRun(runId, input) {
      calls.push(["updateRun", runId, input]);
      return { id: runId, ...input };
    },
    async planRun(runId) {
      calls.push(["planRun", runId]);
      return { id: runId };
    },
    async autoDispatchReadyTasks(runId) {
      calls.push(["autoDispatchReadyTasks", runId]);
      return [];
    },
  };

  const tunnelManager = {};
  const state = { activeSessionId: null, activeRunId: null, statusLine: "" };

  await executeCommand(
    { type: "swarm.launch", goal: "Ship the TUI", model: "gpt-5.4" },
    { manager, orchestrator, tunnelManager, state, options: {} },
  );

  assert.equal(state.activeRunId, "run-1");
  assert.equal(state.activeSessionId, "agent-1");
  assert.deepEqual(calls, [
    ["createAgent", { name: "Swarm Planner 1", role: "arbitrator", model: "gpt-5.4" }],
    ["createAgent", { name: "Swarm Worker 1", role: "worker", model: "gpt-5.4" }],
    ["createAgent", { name: "Swarm Worker 2", role: "worker", model: "gpt-5.4" }],
    ["createRun", { name: "Ship the TUI", goal: "Ship the TUI" }],
    [
      "updateRun",
      "run-1",
      {
        arbitratorAgentId: "agent-1",
        workerAgentIds: ["agent-2", "agent-3"],
      },
    ],
    ["planRun", "run-1"],
    ["autoDispatchReadyTasks", "run-1"],
  ]);
});

test("executeCommand still supports prompts, merges, and share control", async () => {
  const calls = [];

  const manager = {
    listAgents() {
      return [];
    },
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
