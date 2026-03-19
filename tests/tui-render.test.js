import test from "node:test";
import assert from "node:assert/strict";

import { renderDashboard } from "../src/tui/render.js";
import { parseCommand } from "../src/tui/commands.js";

test("renderDashboard prints sessions, runs, tasks, and tunnel status", () => {
  const output = renderDashboard(
    {
      sessions: [
        {
          id: "agent-1",
          name: "Planner",
          role: "arbitrator",
          status: "ready",
          model: "gpt-5.4",
          transcript: [
            { role: "user", text: "Plan the work" },
            { role: "assistant", text: "Plan ready" },
          ],
        },
      ],
      runs: [
        {
          id: "run-1",
          name: "Bootstrap",
          goal: "Build the supervisor core",
          status: "running",
          tasks: [
            {
              id: "task-1",
              title: "Wire event bus",
              status: "completed",
              assignedAgentId: "agent-1",
            },
          ],
        },
      ],
      share: {
        state: "ready",
        publicUrl: "https://example.trycloudflare.com?transport=poll",
      },
    },
    {
      activeSessionId: "agent-1",
      activeRunId: "run-1",
      width: 100,
    },
  );

  assert.match(output, /VORKER-2/);
  assert.match(output, /Sessions/);
  assert.match(output, /> Planner/);
  assert.match(output, /Runs/);
  assert.match(output, /Bootstrap/);
  assert.match(output, /Wire event bus/);
  assert.match(output, /Tunnel: ready/);
  assert.match(output, /Plan ready/);
});

test("parseCommand handles slash commands and plain prompts", () => {
  assert.deepEqual(parseCommand("/agent Planner"), { type: "agent.create", name: "Planner" });
  assert.deepEqual(parseCommand("/use agent-1"), { type: "session.select", sessionId: "agent-1" });
  assert.deepEqual(parseCommand("/share start"), { type: "share.start" });
  assert.deepEqual(parseCommand("ship it"), { type: "prompt.send", text: "ship it" });
});
