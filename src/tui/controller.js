export async function executeCommand(command, context) {
  const { manager, orchestrator, tunnelManager, state, options = {} } = context;

  switch (command.type) {
    case "noop":
      return null;
    case "help":
      state.statusLine =
        "Arrow keys navigate agents, runs, and task lanes. Use /agent <name>, /use <session-id>, /run <name> | <goal>, /run-use <run-id>, /plan, /dispatch, /share start|stop, or type plain text to prompt the active agent.";
      return null;
    case "invalid":
      state.statusLine = command.reason;
      return null;
    case "quit":
      state.statusLine = "Closing TUI.";
      return null;
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
