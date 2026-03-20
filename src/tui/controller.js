function nextSequentialName(prefix, agents = []) {
  const usedNumbers = new Set();

  for (const agent of agents) {
    const match = String(agent?.name ?? "").match(new RegExp(`^${prefix.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s+(\\d+)$`));
    if (match) {
      usedNumbers.add(Number(match[1]));
    }
  }

  let index = 1;
  while (usedNumbers.has(index)) {
    index += 1;
  }

  return `${prefix} ${index}`;
}

function deriveRunName(goal) {
  const trimmed = String(goal ?? "").trim();
  return trimmed ? trimmed.slice(0, 72) : "New Swarm";
}

async function quickCreateAgent(context, command) {
  const { manager, state, options = {} } = context;
  const name = command.name || nextSequentialName("Agent", manager.listAgents?.() ?? []);
  const agent = await manager.createAgent({
    name,
    ...(command.role ? { role: command.role } : {}),
    ...(command.model ? { model: command.model } : {}),
    ...(options.cwd ? { cwd: options.cwd } : {}),
    ...(options.mode ? { mode: options.mode } : {}),
    ...(typeof options.autoApprove === "boolean" ? { autoApprove: options.autoApprove } : {}),
  });
  state.activeSessionId = agent.id;
  state.statusLine = `Created agent ${agent.name}.`;
  return agent;
}

async function launchSwarm(context, command) {
  const { manager, orchestrator, state } = context;
  const model = typeof command.model === "string" && command.model.trim() ? command.model.trim() : null;
  const goal = String(command.goal ?? "").trim();

  if (!goal) {
    state.statusLine = "Swarm goal is required.";
    return null;
  }

  const createdPlanner = await manager.createAgent({
    name: nextSequentialName("Swarm Planner", manager.listAgents?.() ?? []),
    role: "arbitrator",
    ...(model ? { model } : {}),
  });

  const existingWorkers = (manager.listAgents?.() ?? []).filter((agent) => agent.id !== createdPlanner.id);
  const neededWorkers = Math.max(0, 2 - existingWorkers.length);
  const workerAgents = [...existingWorkers];

  for (let index = 0; index < neededWorkers; index += 1) {
    const worker = await manager.createAgent({
      name: nextSequentialName("Swarm Worker", [...(manager.listAgents?.() ?? []), ...workerAgents]),
      role: "worker",
      ...(model ? { model } : {}),
    });
    workerAgents.push(worker);
  }

  const run = orchestrator.createRun({
    name: deriveRunName(goal),
    goal,
  });

  orchestrator.updateRun(run.id, {
    arbitratorAgentId: createdPlanner.id,
    workerAgentIds: workerAgents.slice(0, Math.max(2, workerAgents.length)).map((agent) => agent.id),
  });

  state.activeSessionId = createdPlanner.id;
  state.activeRunId = run.id;

  await orchestrator.planRun(run.id);
  await orchestrator.autoDispatchReadyTasks(run.id);

  state.statusLine = `Swarm running for ${run.name}.`;
  return run;
}

export async function executeCommand(command, context) {
  const { manager, orchestrator, tunnelManager, state, options = {} } = context;

  switch (command.type) {
    case "noop":
      return null;
    case "help":
      state.statusLine =
        "Use arrows to choose MODEL, NEW AGENT, or SWARM. Enter activates the selected action. Slash commands still work for power use.";
      return null;
    case "invalid":
      state.statusLine = command.reason;
      return null;
    case "quit":
      state.statusLine = "Closing TUI.";
      return null;
    case "agent.quickCreate":
      return await quickCreateAgent(context, command);
    case "swarm.launch":
      return await launchSwarm(context, command);
    case "agent.create": {
      const agent = await manager.createAgent({
        name: command.name,
        ...(options.cwd ? { cwd: options.cwd } : {}),
        ...(options.mode ? { mode: options.mode } : {}),
        ...(options.model ? { model: options.model } : {}),
        ...(typeof options.autoApprove === "boolean" ? { autoApprove: options.autoApprove } : {}),
      });
      state.activeSessionId = agent.id;
      state.statusLine = `Created agent ${agent.name}.`;
      return agent;
    }
    case "session.select":
      state.activeSessionId = command.sessionId;
      state.statusLine = `Selected session ${command.sessionId}.`;
      return null;
    case "run.create": {
      const run = orchestrator.createRun({
        name: command.name,
        goal: command.goal,
        ...(options.cwd ? { workspace: options.cwd } : {}),
      });
      state.activeRunId = run.id;
      state.statusLine = `Created run ${run.name}.`;
      return run;
    }
    case "run.select":
      state.activeRunId = command.runId;
      state.statusLine = `Selected run ${command.runId}.`;
      return null;
    case "run.plan": {
      const runId = command.runId ?? state.activeRunId;
      if (!runId) {
        state.statusLine = "Select a run first.";
        return null;
      }
      await orchestrator.planRun(runId);
      state.statusLine = `Planned run ${runId}.`;
      return null;
    }
    case "run.dispatch": {
      const runId = command.runId ?? state.activeRunId;
      if (!runId) {
        state.statusLine = "Select a run first.";
        return null;
      }
      await orchestrator.autoDispatchReadyTasks(runId);
      state.statusLine = `Dispatched ready tasks for ${runId}.`;
      return null;
    }
    case "run.merge": {
      const runId = command.runId ?? state.activeRunId;
      if (!runId) {
        state.statusLine = "Select a run first.";
        return null;
      }
      await orchestrator.mergeCompletedTasks(runId);
      state.statusLine = `Merged completed tasks for ${runId}.`;
      return null;
    }
    case "task.merge":
      await orchestrator.mergeTask(command.taskId);
      state.statusLine = `Merged task ${command.taskId}.`;
      return null;
    case "share.start":
      await tunnelManager.start();
      state.statusLine = "Cloudflare tunnel started.";
      return null;
    case "share.stop":
      await tunnelManager.stop();
      state.statusLine = "Cloudflare tunnel stopped.";
      return null;
    case "prompt.send":
      if (!state.activeSessionId) {
        state.statusLine = "Create or select an agent first.";
        return null;
      }
      await manager.promptAgent(state.activeSessionId, command.text);
      state.statusLine = `Prompt sent to ${state.activeSessionId}.`;
      return null;
    default:
      state.statusLine = `Unsupported command: ${command.type}`;
      return null;
  }
}
