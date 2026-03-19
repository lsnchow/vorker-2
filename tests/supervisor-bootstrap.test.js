import test from "node:test";
import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import { mkdtemp } from "node:fs/promises";

import { EventLog } from "../src/supervisor/event-log.js";
import { createSupervisorEvent } from "../src/supervisor/events.js";
import { restoreDurableSupervisorState } from "../src/supervisor/bootstrap.js";

test("restoreDurableSupervisorState rebuilds runs and tasks without reviving stale sessions", async () => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "vorker-2-bootstrap-"));
  const eventLog = new EventLog({
    rootDir,
    filePath: path.join(rootDir, "supervisor.ndjson"),
  });

  await eventLog.append(
    createSupervisorEvent("run.created", {
      run: {
        id: "run-1",
        name: "Bootstrap",
        goal: "Persist runs",
        status: "running",
        workerAgentIds: [],
        createdAt: "2026-03-19T00:00:00.000Z",
        updatedAt: "2026-03-19T00:00:00.000Z",
      },
    }),
  );
  await eventLog.append(
    createSupervisorEvent("task.created", {
      task: {
        id: "task-1",
        runId: "run-1",
        title: "Persist task",
        description: "Keep it after restart",
        status: "completed",
        branchName: "vorker/task-task-1",
        workspacePath: "/repo/.vorker-2/worktrees/task-1",
        createdAt: "2026-03-19T00:01:00.000Z",
        updatedAt: "2026-03-19T00:01:00.000Z",
      },
    }),
  );
  await eventLog.append(
    createSupervisorEvent("session.registered", {
      session: {
        id: "agent-1",
        name: "Old agent",
      },
    }),
  );

  const hydratedRuns = [];
  const orchestrator = {
    hydrate(runs) {
      hydratedRuns.push(...runs);
    },
  };

  const snapshot = await restoreDurableSupervisorState({
    eventLog,
    orchestrator,
  });

  assert.equal(snapshot.runs.length, 1);
  assert.equal(snapshot.runs[0].tasks.length, 1);
  assert.equal(snapshot.sessions.length, 0);
  assert.equal(snapshot.runs[0].tasks[0].branchName, "vorker/task-task-1");
  assert.equal(hydratedRuns.length, 1);
});
