import { randomUUID } from "node:crypto";

export function createSupervisorEvent(type, payload = {}, options = {}) {
  return {
    id: options.id ?? randomUUID(),
    type: String(type),
    timestamp: options.timestamp ?? new Date().toISOString(),
    payload: payload ?? {},
  };
}
