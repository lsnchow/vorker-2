export const ACTION_ITEMS = ["model", "new-agent", "swarm"];
const PANE_ORDER = ["actions", "sessions", "runs", "tasks", "events"];
const FALLBACK_MODELS = ["gpt-5.4", "gpt-5", "gpt-4.1"];

function moveSelection(ids, currentId, delta) {
  if (ids.length === 0) {
    return null;
  }

  const currentIndex = Math.max(0, ids.indexOf(currentId));
  const nextIndex = Math.max(0, Math.min(ids.length - 1, currentIndex + delta));
  return ids[nextIndex];
}

function moveWrapped(ids, currentId, delta) {
  if (ids.length === 0) {
    return null;
  }

  const currentIndex = Math.max(0, ids.indexOf(currentId));
  const nextIndex = (currentIndex + delta + ids.length) % ids.length;
  return ids[nextIndex];
}

function sessionIds(snapshot) {
  return (snapshot.sessions ?? []).map((session) => session.id);
}

function runIds(snapshot) {
  return (snapshot.runs ?? []).map((run) => run.id);
}

function taskIds(snapshot, runId) {
  const run = (snapshot.runs ?? []).find((entry) => entry.id === runId);
  return (run?.tasks ?? []).map((task) => task.id);
}

function collectModelChoices(snapshot, state) {
  const seen = new Set();
  const choices = [];

  const add = (value) => {
    if (typeof value !== "string") {
      return;
    }
    const normalized = value.trim();
    if (!normalized || seen.has(normalized)) {
      return;
    }
    seen.add(normalized);
    choices.push(normalized);
  };

  for (const value of state.modelChoices ?? []) {
    add(value);
  }
  add(state.defaultModel);
  for (const session of snapshot.sessions ?? []) {
    add(session.model);
    for (const model of session.availableModels ?? []) {
      add(model);
    }
  }
  for (const value of FALLBACK_MODELS) {
    add(value);
  }
  add(state.selectedModelId);

  return choices;
}

function cycleFocus(currentPane, direction) {
  const currentIndex = PANE_ORDER.indexOf(currentPane);
  const nextIndex = (Math.max(0, currentIndex) + direction + PANE_ORDER.length) % PANE_ORDER.length;
  return PANE_ORDER[nextIndex];
}

export function reconcileNavigationState(snapshot, state = {}) {
  const next = {
    focusedPane: PANE_ORDER.includes(state.focusedPane) ? state.focusedPane : "actions",
    selectedActionId: ACTION_ITEMS.includes(state.selectedActionId) ? state.selectedActionId : "new-agent",
    activeSessionId: state.activeSessionId ?? null,
    activeRunId: state.activeRunId ?? null,
    selectedTaskId: state.selectedTaskId ?? null,
    selectedModelId: state.selectedModelId ?? null,
    modelChoices: Array.isArray(state.modelChoices) ? [...state.modelChoices] : [],
    modelPickerOpen: Boolean(state.modelPickerOpen),
    commandBuffer: state.commandBuffer ?? "",
  };

  next.modelChoices = collectModelChoices(snapshot, next);
  next.selectedModelId = next.modelChoices.includes(next.selectedModelId) ? next.selectedModelId : next.modelChoices[0] ?? null;

  const sessions = sessionIds(snapshot);
  next.activeSessionId = sessions.includes(next.activeSessionId) ? next.activeSessionId : sessions[0] ?? null;

  const runs = runIds(snapshot);
  next.activeRunId = runs.includes(next.activeRunId) ? next.activeRunId : runs[0] ?? null;

  const tasks = taskIds(snapshot, next.activeRunId);
  next.selectedTaskId = tasks.includes(next.selectedTaskId) ? next.selectedTaskId : tasks[0] ?? null;

  return next;
}

export function applyNavigationKey(state, snapshot, keyName) {
  const next = reconcileNavigationState(snapshot, state);

  if (next.modelPickerOpen) {
    if (["left", "up", "shift-tab"].includes(keyName)) {
      next.selectedModelId = moveWrapped(next.modelChoices, next.selectedModelId, -1);
    } else if (["right", "down", "tab"].includes(keyName)) {
      next.selectedModelId = moveWrapped(next.modelChoices, next.selectedModelId, 1);
    }
    return reconcileNavigationState(snapshot, next);
  }

  switch (keyName) {
    case "left":
      if (next.focusedPane === "actions") {
        next.selectedActionId = moveWrapped(ACTION_ITEMS, next.selectedActionId, -1);
      } else if (next.focusedPane === "sessions") {
        next.focusedPane = "actions";
      } else if (next.focusedPane === "runs") {
        next.focusedPane = "sessions";
      } else if (next.focusedPane === "tasks") {
        next.focusedPane = "runs";
      } else if (next.focusedPane === "events") {
        next.focusedPane = "tasks";
      }
      break;
    case "right":
      if (next.focusedPane === "actions") {
        next.selectedActionId = moveWrapped(ACTION_ITEMS, next.selectedActionId, 1);
      } else if (next.focusedPane === "sessions") {
        next.focusedPane = "runs";
      } else if (next.focusedPane === "runs") {
        next.focusedPane = "tasks";
      } else if (next.focusedPane === "tasks") {
        next.focusedPane = "events";
      }
      break;
    case "tab":
      next.focusedPane = cycleFocus(next.focusedPane, 1);
      break;
    case "shift-tab":
      next.focusedPane = cycleFocus(next.focusedPane, -1);
      break;
    case "up":
      if (next.focusedPane === "sessions") {
        next.focusedPane = "actions";
      } else if (next.focusedPane === "runs") {
        next.activeRunId = moveSelection(runIds(snapshot), next.activeRunId, -1);
        next.selectedTaskId = null;
      } else if (next.focusedPane === "tasks") {
        next.selectedTaskId = moveSelection(taskIds(snapshot, next.activeRunId), next.selectedTaskId, -1);
      } else if (next.focusedPane === "events") {
        next.focusedPane = "tasks";
      }
      break;
    case "down":
      if (next.focusedPane === "actions") {
        next.focusedPane = "sessions";
      } else if (next.focusedPane === "sessions") {
        next.activeSessionId = moveSelection(sessionIds(snapshot), next.activeSessionId, 1);
      } else if (next.focusedPane === "runs") {
        next.activeRunId = moveSelection(runIds(snapshot), next.activeRunId, 1);
        next.selectedTaskId = null;
      } else if (next.focusedPane === "tasks") {
        next.selectedTaskId = moveSelection(taskIds(snapshot, next.activeRunId), next.selectedTaskId, 1);
      }
      break;
    default:
      break;
  }

  return reconcileNavigationState(snapshot, next);
}
