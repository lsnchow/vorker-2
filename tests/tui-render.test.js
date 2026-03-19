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
              executionAgentId: "exec-1",
              branchName: "vorker/task-task-1-wire-event-bus",
              workspacePath: "/repo/.vorker-2/worktrees/task-1",
              commitSha: "abc123def456",
              changeCount: 1,
            },
          ],
        },
      ],
      share: {
        state: "ready",
        publicUrl: "https://example.trycloudflare.com?transport=poll",
      },
      events: [
        { type: "task.updated", payload: { task: { title: "Wire event bus", status: "completed" } } },
        { type: "run.updated", payload: { run: { name: "Bootstrap", status: "running" } } },
      ],
    },
    {
      activeSessionId: "agent-1",
      activeRunId: "run-1",
      width: 100,
      statusLine: "Ready for commands.",
    },
  );

  assert.match(output, /VORKER-2/);
  assert.match(output, /ACTIVE SESSIONS/);
  assert.match(output, /ACTIVE SESSION/);
  assert.match(output, /RUN BOARD/);
  assert.match(output, /EVENT FEED/);
  assert.match(output, /Planner/);
  assert.match(output, /Bootstrap/);
  assert.match(output, /Wire event bus/);
  assert.match(output, /vorker\/task-task-1-wire-event-bus/);
  assert.match(output, /exec-1/);
  assert.match(output, /abc123def456/);
  assert.match(output, /ready/);
  assert.match(output, /Plan ready/);
  assert.match(output, /Ready for commands/);
});

test("parseCommand handles slash commands and plain prompts", () => {
  assert.deepEqual(parseCommand("/agent Planner"), { type: "agent.create", name: "Planner" });
  assert.deepEqual(parseCommand("/use agent-1"), { type: "session.select", sessionId: "agent-1" });
  assert.deepEqual(parseCommand("/share start"), { type: "share.start" });
  assert.deepEqual(parseCommand("/merge"), { type: "run.merge", runId: null });
  assert.deepEqual(parseCommand("/merge-task task-1"), { type: "task.merge", taskId: "task-1" });
  assert.deepEqual(parseCommand("ship it"), { type: "prompt.send", text: "ship it" });
});
