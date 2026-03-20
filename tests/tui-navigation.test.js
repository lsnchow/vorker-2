import test from "node:test";
import assert from "node:assert/strict";

import { ACTION_ITEMS, applyNavigationKey, reconcileNavigationState } from "../src/tui/navigation.js";

function createSnapshot() {
  return {
    sessions: [
      { id: "agent-1", name: "Planner", model: "gpt-5.4", availableModels: ["gpt-5.4", "gpt-5"] },
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
    focusedPane: "actions",
    selectedActionId: "new-agent",
    activeSessionId: "agent-1",
    activeRunId: "run-1",
    selectedTaskId: "task-1",
    selectedModelId: "gpt-5.4",
    modelChoices: ["gpt-5.4", "gpt-5", "gpt-4.1"],
    modelPickerOpen: false,
    commandBuffer: "",
  });
});

test("applyNavigationKey moves across the action rail and pane selections", () => {
  const snapshot = createSnapshot();
  let state = reconcileNavigationState(snapshot, {});

  state = applyNavigationKey(state, snapshot, "right");
  assert.equal(state.focusedPane, "actions");
  assert.equal(state.selectedActionId, "swarm");

  state = applyNavigationKey(state, snapshot, "left");
  assert.equal(state.selectedActionId, "new-agent");

  state = applyNavigationKey(state, snapshot, "down");
  assert.equal(state.focusedPane, "sessions");

  state = applyNavigationKey(state, snapshot, "up");
  assert.equal(state.focusedPane, "actions");

  state = applyNavigationKey(state, snapshot, "down");
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
  assert.equal(state.focusedPane, "sessions");
});

test("applyNavigationKey cycles models while the model picker is open", () => {
  const snapshot = createSnapshot();
  const state = reconcileNavigationState(snapshot, {
    selectedActionId: ACTION_ITEMS[0],
    modelPickerOpen: true,
  });

  const next = applyNavigationKey(state, snapshot, "right");
  assert.equal(next.selectedModelId, "gpt-5");

  const last = applyNavigationKey(next, snapshot, "left");
  assert.equal(last.selectedModelId, "gpt-5.4");
});
