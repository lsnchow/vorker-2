function parseSlashCommand(input) {
  const trimmed = input.trim();
  if (!trimmed.startsWith("/")) {
    return null;
  }

  const [command, ...rest] = trimmed.slice(1).split(/\s+/);
  const remainder = rest.join(" ").trim();

  switch (command) {
    case "agent":
      return remainder ? { type: "agent.create", name: remainder } : { type: "invalid", reason: "Agent name is required." };
    case "use":
      return remainder ? { type: "session.select", sessionId: remainder } : { type: "invalid", reason: "Session id is required." };
    case "run": {
      const [name, goal] = remainder.split("|").map((value) => value?.trim()).filter(Boolean);
      return name ? { type: "run.create", name, goal: goal ?? "" } : { type: "invalid", reason: "Run name is required." };
    }
    case "run-use":
      return remainder ? { type: "run.select", runId: remainder } : { type: "invalid", reason: "Run id is required." };
    case "plan":
      return { type: "run.plan", runId: remainder || null };
    case "dispatch":
      return { type: "run.dispatch", runId: remainder || null };
    case "merge":
      return { type: "run.merge", runId: remainder || null };
    case "merge-task":
      return remainder ? { type: "task.merge", taskId: remainder } : { type: "invalid", reason: "Task id is required." };
    case "share":
      if (remainder === "start") {
        return { type: "share.start" };
      }
      if (remainder === "stop") {
        return { type: "share.stop" };
      }
      return { type: "invalid", reason: "Use /share start or /share stop." };
    case "help":
      return { type: "help" };
    case "quit":
    case "exit":
      return { type: "quit" };
    default:
      return { type: "invalid", reason: `Unknown command: /${command}` };
  }
}

export function parseCommand(input) {
  const trimmed = String(input ?? "").trim();
  if (!trimmed) {
    return { type: "noop" };
  }

  return parseSlashCommand(trimmed) ?? { type: "prompt.send", text: trimmed };
}
