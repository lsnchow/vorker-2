import path from "node:path";
import process from "node:process";
import readline from "node:readline/promises";
import { emitKeypressEvents } from "node:readline";
import { renderBootFrame } from "./tui/boot.js";
import { applyNavigationKey, reconcileNavigationState } from "./tui/navigation.js";
import { CopilotManager } from "./copilot.js";
import { Orchestrator } from "./orchestrator.js";
import { SkillCatalog } from "./skills.js";
import { TunnelManager } from "./tunnel.js";
import { EventLog } from "./supervisor/event-log.js";
import { restoreDurableSupervisorState } from "./supervisor/bootstrap.js";
import { SupervisorService } from "./supervisor/service.js";
import { parseCommand } from "./tui/commands.js";
import { executeCommand } from "./tui/controller.js";
import { renderDashboard } from "./tui/render.js";

function clearScreen() {
  if (process.stdout.isTTY) {
    process.stdout.write("\x1b[2J\x1b[H");
  }
}

function resolveProtocol(options) {
  return options.tlsKey && options.tlsCert ? "https" : "http";
}

function enterAltScreen() {
  if (process.stdout.isTTY) {
    process.stdout.write("\x1b[?1049h\x1b[?25l\x1b[?1h");
  }
}

function exitAltScreen() {
  if (process.stdout.isTTY) {
    process.stdout.write("\x1b[?1l\x1b[?25h\x1b[?1049l");
  }
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function buildBootSteps(snapshot, skillCatalog) {
  const tasks = (snapshot.runs ?? []).flatMap((run) => run.tasks ?? []);
  const activeAgents = snapshot.sessions?.length ?? 0;
  const activeLanes = tasks.filter((task) => ["running", "planning", "starting", "ready"].includes(task.status)).length;
  const completedTasks = tasks.filter((task) => ["completed", "merged"].includes(task.status)).length;

  return [
    {
      id: "event-log",
      label: "event log",
      detail: `replayed ${snapshot.events?.length ?? 0} supervisor events`,
      status: "ready",
    },
    {
      id: "agent-mesh",
      label: "agent-mesh",
      detail: `synced ${activeAgents} agent session${activeAgents === 1 ? "" : "s"}`,
      status: "ready",
    },
    {
      id: "worker-pool",
      label: "worker-pool",
      detail: `warming ${Math.max(4, activeLanes || 0)} execution lanes`,
      status: "ready",
    },
    {
      id: "merge-queue",
      label: "merge-queue",
      detail: `${completedTasks} completed task lane${completedTasks === 1 ? "" : "s"} ready for review`,
      status: completedTasks > 0 ? "ready" : "pending",
    },
    {
      id: "skills",
      label: "skills",
      detail: `loaded ${skillCatalog.listSkills().length} workspace skills`,
      status: "ready",
    },
  ];
}

async function playBootAnimation(snapshot, skillCatalog) {
  if (!process.stdout.isTTY) {
    return;
  }

  const steps = buildBootSteps(snapshot, skillCatalog);
  for (let index = 0; index < steps.length; index += 1) {
    for (let tick = 0; tick < 4; tick += 1) {
      const frameSteps = steps.map((step, stepIndex) => ({
        ...step,
        status: stepIndex < index ? "ready" : stepIndex === index ? "loading" : "pending",
      }));
      clearScreen();
      process.stdout.write(
        `${renderBootFrame({
          width: process.stdout.columns ?? 100,
          tick,
          color: true,
          activeStepId: steps[index].id,
          steps: frameSteps,
        })}\n`,
      );
      await sleep(55);
    }
  }

  clearScreen();
  process.stdout.write(
    `${renderBootFrame({
      width: process.stdout.columns ?? 100,
      tick: 0,
      color: true,
      steps,
    })}\n`,
  );
  await sleep(90);
}

function collectModelChoices(manager, state, options) {
  const choices = [];
  const seen = new Set();

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

  for (const model of state.modelChoices ?? []) {
    add(model);
  }
  add(options.model);
  for (const agent of manager.listAgents?.() ?? []) {
    add(agent.model);
    for (const model of agent.availableModels ?? []) {
      add(model);
    }
  }
  for (const fallback of ["gpt-5.4", "gpt-5", "gpt-4.1"]) {
    add(fallback);
  }
  add(state.selectedModelId);

  return choices;
}

function describeAction(state) {
  if (state.modelPickerOpen) {
    return `Select a model with arrows. Current: ${state.selectedModelId ?? "unset"}.`;
  }
  if (state.selectedActionId === "model") {
    return `Model locked to ${state.selectedModelId ?? "unset"}. Press Enter to change it.`;
  }
  if (state.selectedActionId === "new-agent") {
    return `Press Enter to create a new agent on ${state.selectedModelId ?? "the selected model"}.`;
  }
  if (state.selectedActionId === "swarm") {
    return `Press Enter to launch a swarm on ${state.selectedModelId ?? "the selected model"}.`;
  }
  return "Choose an action.";
}

function describeFocus(snapshot, state) {
  if (state.focusedPane === "actions") {
    return describeAction(state);
  }

  if (state.focusedPane === "sessions") {
    const session = (snapshot.sessions ?? []).find((entry) => entry.id === state.activeSessionId);
    return session ? `Selected agent ${session.name}.` : "No active agents yet.";
  }

  if (state.focusedPane === "runs") {
    const run = (snapshot.runs ?? []).find((entry) => entry.id === state.activeRunId);
    return run ? `Selected run ${run.name}.` : "No swarm runs yet.";
  }

  if (state.focusedPane === "tasks") {
    const run = (snapshot.runs ?? []).find((entry) => entry.id === state.activeRunId);
    const task = (run?.tasks ?? []).find((entry) => entry.id === state.selectedTaskId);
    return task ? `Selected task ${task.title}.` : "No task lanes in the current run.";
  }

  return "Watching the latest supervisor events.";
}

function syncUiState(manager, supervisor, state, options) {
  state.modelChoices = collectModelChoices(manager, state, options);
  state.defaultModel = options.model ?? "gpt-5.4";
  Object.assign(state, reconcileNavigationState(supervisor.snapshot(), state));
}

async function runLineInputLoop(context) {
  const { redraw, handleLine } = context;
  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });

  try {
    redraw();
    while (true) {
      const line = await rl.question("> ");
      const shouldContinue = await handleLine(line);
      redraw();
      if (!shouldContinue) {
        break;
      }
    }
  } finally {
    rl.close();
  }
}

async function runInteractiveInputLoop(context) {
  const { redraw, handleLine, state, getSnapshot, syncState, runAction } = context;

  emitKeypressEvents(process.stdin);
  process.stdin.setRawMode(true);
  process.stdin.resume();

  return new Promise((resolve) => {
    let closing = false;
    let queue = Promise.resolve();

    const enqueue = (task) => {
      queue = queue
        .then(task)
        .catch((error) => {
          state.statusLine = error instanceof Error ? error.message : String(error);
          redraw();
        });
    };

    const finish = () => {
      if (closing) {
        return;
      }

      closing = true;
      process.stdin.off("keypress", onKeypress);
      process.stdin.setRawMode(false);
      resolve();
    };

    const onKeypress = (str, key = {}) => {
      if (closing) {
        return;
      }

      if (key.ctrl && key.name === "c") {
        state.statusLine = "Closing TUI.";
        finish();
        return;
      }

      if (key.name === "escape") {
        if (state.modelPickerOpen) {
          state.modelPickerOpen = false;
          state.statusLine = `Model kept at ${state.selectedModelId ?? "unset"}.`;
        } else if (state.inputMode === "swarm-goal") {
          state.inputMode = "prompt";
          state.commandBuffer = "";
          state.statusLine = "Swarm launch cancelled.";
        } else {
          state.commandBuffer = "";
          state.statusLine = "Command buffer cleared.";
        }
        redraw();
        return;
      }

      if (["left", "right", "up", "down", "tab"].includes(key.name)) {
        if (state.inputMode === "swarm-goal" && !state.modelPickerOpen) {
          return;
        }
        Object.assign(state, applyNavigationKey(state, getSnapshot(), key.shift ? "shift-tab" : key.name));
        state.statusLine = describeFocus(getSnapshot(), state);
        redraw();
        return;
      }

      if (key.name === "backspace" || key.name === "delete") {
        state.commandBuffer = Array.from(state.commandBuffer).slice(0, -1).join("");
        redraw();
        return;
      }

      if (key.name === "return") {
        if (state.modelPickerOpen) {
          state.modelPickerOpen = false;
          state.statusLine = `Model locked to ${state.selectedModelId ?? "unset"}.`;
          redraw();
          return;
        }

        if (state.focusedPane === "actions" && !state.commandBuffer.trim()) {
          if (state.selectedActionId === "model") {
            state.modelPickerOpen = true;
            state.statusLine = `Choose a model. Current: ${state.selectedModelId ?? "unset"}.`;
            redraw();
            return;
          }

          if (state.selectedActionId === "new-agent") {
            enqueue(async () => {
              await runAction({
                type: "agent.quickCreate",
                model: state.selectedModelId,
              });
            });
            return;
          }

          if (state.selectedActionId === "swarm") {
            state.inputMode = "swarm-goal";
            state.commandBuffer = "";
            state.statusLine = `Type the swarm goal for ${state.selectedModelId ?? "the selected model"} and press Enter.`;
            redraw();
            return;
          }
        }

        const line = state.commandBuffer.trim();
        state.commandBuffer = "";
        if (!line) {
          redraw();
          return;
        }

        enqueue(async () => {
          const shouldContinue = await handleLine(line);
          syncState();
          redraw();
          if (!shouldContinue) {
            finish();
          }
        });
        return;
      }

      if (!key.ctrl && !key.meta && str) {
        state.commandBuffer += str;
        redraw();
      }
    };

    process.stdin.on("keypress", onKeypress);
    redraw();
  });
}

export async function runTui(options) {
  const skillCatalog = new SkillCatalog({ cwd: options.cwd });
  await skillCatalog.refresh();

  const manager = new CopilotManager({
    cwd: options.cwd,
    copilotBin: options.copilotBin,
    mode: options.mode,
    model: options.model,
    autoApprove: options.autoApprove,
    debug: options.debug,
    skillCatalog,
  });

  const orchestrator = new Orchestrator({ manager });
  const tunnelManager = new TunnelManager({
    port: options.port,
    host: options.host,
    protocol: resolveProtocol(options),
    cloudflaredBin: options.cloudflaredBin,
    edgeProtocol: options.cloudflaredProtocol,
    edgeIpVersion: options.cloudflaredEdgeIpVersion,
  });

  const logsDir = path.join(options.cwd, ".vorker-2", "logs");
  const eventLog = new EventLog({
    rootDir: logsDir,
    filePath: path.join(logsDir, "supervisor.ndjson"),
  });

  const supervisor = new SupervisorService({
    manager,
    orchestrator,
    tunnelManager,
    skillCatalog,
    eventLog,
  });

  const state = {
    activeSessionId: null,
    activeRunId: null,
    selectedTaskId: null,
    focusedPane: "actions",
    selectedActionId: "new-agent",
    selectedModelId: options.model ?? "gpt-5.4",
    modelChoices: [],
    modelPickerOpen: false,
    commandBuffer: "",
    inputMode: "prompt",
    statusLine: "Use arrows to pick MODEL, NEW AGENT, or SWARM. Enter activates the selection.",
  };
  const useAltScreen = process.stdout.isTTY && !options.noAltScreen;

  await restoreDurableSupervisorState({
    eventLog,
    orchestrator,
    store: supervisor.store,
  });
  await supervisor.start();
  await supervisor.refreshSkills();
  syncUiState(manager, supervisor, state, options);

  const redraw = () => {
    syncUiState(manager, supervisor, state, options);
    clearScreen();
    process.stdout.write(
      `${renderDashboard(supervisor.snapshot(), {
        activeSessionId: state.activeSessionId,
        activeRunId: state.activeRunId,
        selectedTaskId: state.selectedTaskId,
        focusedPane: state.focusedPane,
        selectedActionId: state.selectedActionId,
        selectedModelId: state.selectedModelId,
        modelChoices: state.modelChoices,
        modelPickerOpen: state.modelPickerOpen,
        inputMode: state.inputMode,
        commandBuffer: state.commandBuffer,
        width: process.stdout.columns ?? 100,
        color: process.stdout.isTTY,
        statusLine: state.statusLine,
      })}\n\n`,
    );
  };

  const runAction = async (command) => {
    try {
      await executeCommand(command, {
        manager,
        orchestrator,
        tunnelManager,
        skillCatalog,
        supervisor,
        state,
        options,
      });
      state.inputMode = "prompt";
      syncUiState(manager, supervisor, state, options);
    } catch (error) {
      state.statusLine = error instanceof Error ? error.message : String(error);
    }
    redraw();
  };

  const handleLine = async (line) => {
    if (state.inputMode === "swarm-goal") {
      await runAction({
        type: "swarm.launch",
        goal: line,
        model: state.selectedModelId,
      });
      return true;
    }

    const command = parseCommand(line);
    if (command.type === "quit") {
      state.statusLine = "Closing TUI.";
      return false;
    }

    try {
      await executeCommand(command, {
        manager,
        orchestrator,
        tunnelManager,
        skillCatalog,
        supervisor,
        state,
        options,
      });
    } catch (error) {
      state.statusLine = error instanceof Error ? error.message : String(error);
    }

    syncUiState(manager, supervisor, state, options);
    return true;
  };

  const onSupervisorEvent = () => {
    syncUiState(manager, supervisor, state, options);
    redraw();
  };

  supervisor.on("event", onSupervisorEvent);

  try {
    if (useAltScreen) {
      enterAltScreen();
    }
    process.stdout.on("resize", redraw);
    await playBootAnimation(supervisor.snapshot(), skillCatalog);

    if (process.stdin.isTTY && process.stdout.isTTY) {
      await runInteractiveInputLoop({
        redraw,
        handleLine,
        state,
        getSnapshot: () => supervisor.snapshot(),
        syncState: () => syncUiState(manager, supervisor, state, options),
        runAction,
      });
    } else {
      await runLineInputLoop({
        redraw,
        handleLine,
      });
    }
  } finally {
    process.stdout.off("resize", redraw);
    supervisor.off("event", onSupervisorEvent);
    await supervisor.close();
    await manager.closeAll();
    if (tunnelManager.child) {
      await tunnelManager.stop();
    }
    if (process.stdin.isTTY) {
      process.stdin.setRawMode?.(false);
    }
    if (useAltScreen) {
      exitAltScreen();
    }
  }
}
