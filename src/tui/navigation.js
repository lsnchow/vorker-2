const PANE_ORDER = ["sessions", "runs", "tasks", "events"];

function moveSelection(ids, currentId, delta) {
  if (ids.length === 0) {
    return null;
  }

  const currentIndex = Math.max(0, ids.indexOf(currentId));
  const nextIndex = Math.max(0, Math.min(ids.length - 1, currentIndex + delta));
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

export function reconcileNavigationState(snapshot, state = {}) {
  const next = {
    focusedPane: PANE_ORDER.includes(state.focusedPane) ? state.focusedPane : "sessions",
    activeSessionId: state.activeSessionId ?? null,
    activeRunId: state.activeRunId ?? null,
    selectedTaskId: state.selectedTaskId ?? null,
    commandBuffer: state.commandBuffer ?? "",
  };

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

  switch (keyName) {
    case "left": {
      const currentIndex = PANE_ORDER.indexOf(next.focusedPane);
      next.focusedPane = PANE_ORDER[(currentIndex - 1 + PANE_ORDER.length) % PANE_ORDER.length];
      break;
    }
    case "right":
    case "tab": {
      const currentIndex = PANE_ORDER.indexOf(next.focusedPane);
      next.focusedPane = PANE_ORDER[(currentIndex + 1) % PANE_ORDER.length];
      break;
    }
    case "shift-tab": {
      const currentIndex = PANE_ORDER.indexOf(next.focusedPane);
      next.focusedPane = PANE_ORDER[(currentIndex - 1 + PANE_ORDER.length) % PANE_ORDER.length];
      break;
    }
    case "up":
    case "down": {
      const delta = keyName === "up" ? -1 : 1;
      if (next.focusedPane === "sessions") {
        next.activeSessionId = moveSelection(sessionIds(snapshot), next.activeSessionId, delta);
      } else if (next.focusedPane === "runs") {
        next.activeRunId = moveSelection(runIds(snapshot), next.activeRunId, delta);
        next.selectedTaskId = null;
      } else if (next.focusedPane === "tasks") {
        next.selectedTaskId = moveSelection(taskIds(snapshot, next.activeRunId), next.selectedTaskId, delta);
      }
      break;
    }
    default:
      break;
  }

  return reconcileNavigationState(snapshot, next);
}
