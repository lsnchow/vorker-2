import test from "node:test";
import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import { mkdtemp, readFile } from "node:fs/promises";

import { EventLog } from "../src/supervisor/event-log.js";
import { createSupervisorEvent } from "../src/supervisor/events.js";

test("EventLog appends NDJSON events and replays them in order", async () => {
  const rootDir = await mkdtemp(path.join(os.tmpdir(), "vorker-2-event-log-"));
  const eventLog = new EventLog({ rootDir });

  const events = [
    createSupervisorEvent("run.created", { run: { id: "run-1", name: "Bootstrap" } }),
    createSupervisorEvent("task.created", { task: { id: "task-1", runId: "run-1", title: "First task" } }),
  ];

  for (const event of events) {
    await eventLog.append(event);
  }

  const raw = await readFile(eventLog.filePath, "utf8");
  const replayed = await eventLog.readAll();

  assert.equal(raw.trim().split("\n").length, 2);
  assert.equal(replayed.length, 2);
  assert.equal(replayed[0].type, "run.created");
  assert.equal(replayed[1].payload.task.id, "task-1");
});
