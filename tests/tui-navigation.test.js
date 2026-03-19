import test from "node:test";
import assert from "node:assert/strict";

import { applyNavigationKey, reconcileNavigationState } from "../src/tui/navigation.js";

function createSnapshot() {
  return {
    sessions: [
      { id: "agent-1", name: "Planner" },
      { id: "agent-2", name: "Reviewer" },
    ],
    runs: [
      {
        id: "run-1",
        name: "Bootstrap",
        tasks: [
          { id: "task-1", title: "Wire event bus" },
          { id: "task-2", title: "Build renderer" },
        ],
      },
      {
        id: "run-2",
        name: "Polish",
        tasks: [{ id: "task-3", title: "Tune theme" }],
      },
    ],
  };
}

test("reconcileNavigationState picks defaults for empty selection state", () => {
  const next = reconcileNavigationState(createSnapshot(), {});

  assert.deepEqual(next, {
    focusedPane: "sessions",
    activeSessionId: "agent-1",
    activeRunId: "run-1",
    selectedTaskId: "task-1",
    commandBuffer: "",
  });
});

test("applyNavigationKey moves focus and selection with arrow keys", () => {
  const snapshot = createSnapshot();
  let state = reconcileNavigationState(snapshot, {});

  state = applyNavigationKey(state, snapshot, "right");
  assert.equal(state.focusedPane, "runs");

  state = applyNavigationKey(state, snapshot, "down");
  assert.equal(state.activeRunId, "run-2");
  assert.equal(state.selectedTaskId, "task-3");

  state = applyNavigationKey(state, snapshot, "right");
  assert.equal(state.focusedPane, "tasks");

  state = applyNavigationKey(state, snapshot, "left");
  state = applyNavigationKey(state, snapshot, "up");
  assert.equal(state.activeRunId, "run-1");
  assert.equal(state.selectedTaskId, "task-1");

  state = applyNavigationKey(state, snapshot, "left");
  state = applyNavigationKey(state, snapshot, "down");
  assert.equal(state.focusedPane, "sessions");
  assert.equal(state.activeSessionId, "agent-2");
});
