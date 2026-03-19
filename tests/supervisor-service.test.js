import test from "node:test";
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";

import { SupervisorService } from "../src/supervisor/service.js";

test("SupervisorService bridges manager, orchestrator, skills, and tunnel events into one store", async () => {
  const manager = new EventEmitter();
  manager.listAgents = () => [];

  const orchestrator = new EventEmitter();
  orchestrator.listRuns = () => [];

  const tunnelManager = new EventEmitter();
  tunnelManager.snapshot = () => ({ state: "idle", publicUrl: null });

  const skillCatalog = {
    listSkills() {
      return [{ id: "repo-map", name: "repo-map" }];
    },
  };

  const appended = [];
  const eventLog = {
    async append(event) {
      appended.push(event);
      return event;
    },
  };

  const supervisor = new SupervisorService({
    manager,
    orchestrator,
    tunnelManager,
    skillCatalog,
    eventLog,
  });

  await supervisor.start();

  manager.emit("agent_created", {
    agent: {
      id: "agent-1",
      name: "Planner",
      role: "arbitrator",
      status: "ready",
      mode: "plan",
      model: "gpt-5.4",
      cwd: "/workspace",
    },
  });

  manager.emit("agent_event", {
    type: "prompt_started",
    agent: {
      id: "agent-1",
      name: "Planner",
      role: "arbitrator",
      status: "running",
      mode: "plan",
      model: "gpt-5.4",
      cwd: "/workspace",
    },
    text: "Plan the work",
  });

  manager.emit("agent_event", {
    type: "prompt_finished",
    agent: {
      id: "agent-1",
      name: "Planner",
      role: "arbitrator",
      status: "ready",
      mode: "plan",
      model: "gpt-5.4",
      cwd: "/workspace",
    },
    responseText: "Plan ready",
  });

  orchestrator.emit("event", {
    type: "run_created",
    run: {
      id: "run-1",
      name: "Bootstrap",
      goal: "Stand up supervisor",
      status: "draft",
      workerAgentIds: [],
      tasks: [],
      createdAt: "2026-03-19T00:00:00.000Z",
      updatedAt: "2026-03-19T00:00:00.000Z",
    },
  });

  tunnelManager.emit("event", {
    type: "share_state",
    share: {
      state: "ready",
      publicUrl: "https://example.trycloudflare.com?transport=poll",
    },
  });

  const snapshot = supervisor.snapshot();
  const session = snapshot.sessions[0];

  assert.equal(snapshot.runs.length, 1);
  assert.equal(snapshot.skills.length, 1);
  assert.equal(snapshot.share.state, "ready");
  assert.equal(session.id, "agent-1");
  assert.equal(session.transcript.length, 2);
  assert.equal(session.transcript[0].role, "user");
  assert.equal(session.transcript[1].text, "Plan ready");
  assert.ok(appended.length >= 5);
});
