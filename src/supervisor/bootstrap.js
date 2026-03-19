import { SupervisorStore } from "./store.js";

const DURABLE_EVENT_TYPES = new Set([
  "run.created",
  "run.updated",
  "task.created",
  "task.updated",
]);

export async function restoreDurableSupervisorState(options = {}) {
  const store = options.store ?? new SupervisorStore();
  const events = await options.eventLog.readAll();

  for (const event of events) {
    if (DURABLE_EVENT_TYPES.has(event.type)) {
      store.append(event);
    }
  }

  const snapshot = store.snapshot();
  options.orchestrator?.hydrate?.(snapshot.runs);
  return snapshot;
}
