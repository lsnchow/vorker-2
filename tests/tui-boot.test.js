import test from "node:test";
import assert from "node:assert/strict";

import { renderBootFrame } from "../src/tui/boot.js";

test("renderBootFrame shows the new title and multi-agent loading lanes", () => {
  const output = renderBootFrame({
    width: 96,
    tick: 3,
    activeStepId: "worker-pool",
    steps: [
      { id: "event-log", label: "event log", status: "ready", detail: "replayed supervisor journal" },
      { id: "worker-pool", label: "worker-pool", status: "loading", detail: "warming 6 execution lanes" },
      { id: "merge-queue", label: "merge-queue", status: "pending", detail: "syncing reconciler state" },
    ],
  });

  assert.match(output, /██╗   ██╗/);
  assert.match(output, /worker-pool/);
  assert.match(output, /warming 6 execution lanes/);
  assert.match(output, /VORKER CONTROL PLANE/);
});
