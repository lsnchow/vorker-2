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
    process.stdout.write("\x1b[?1049h\x1b[?25l");
  }
}

function exitAltScreen() {
  if (process.stdout.isTTY) {
    process.stdout.write("\x1b[?25h\x1b[?1049l");
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

function describeFocus(snapshot, state) {
  if (state.focusedPane === "sessions") {
    const session = (snapshot.sessions ?? []).find((entry) => entry.id === state.activeSessionId);
    return session ? `Focused agent ${session.name}.` : "Focused agent list.";
  }

  if (state.focusedPane === "runs") {
    const run = (snapshot.runs ?? []).find((entry) => entry.id === state.activeRunId);
    return run ? `Focused run ${run.name}.` : "Focused run board.";
  }

  if (state.focusedPane === "tasks") {
    const run = (snapshot.runs ?? []).find((entry) => entry.id === state.activeRunId);
    const task = (run?.tasks ?? []).find((entry) => entry.id === state.selectedTaskId);
    return task ? `Focused task ${task.title}.` : "Focused task lanes.";
  }

  return "Focused event feed.";
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
  const { redraw, handleLine, state, getSnapshot } = context;

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

      if (key.name === "tab") {
        Object.assign(state, applyNavigationKey(state, getSnapshot(), key.shift ? "shift-tab" : "tab"));
        state.statusLine = describeFocus(getSnapshot(), state);
        redraw();
        return;
      }

      if (["left", "right", "up", "down"].includes(key.name)) {
        Object.assign(state, applyNavigationKey(state, getSnapshot(), key.name));
        state.statusLine = describeFocus(getSnapshot(), state);
        redraw();
        return;
      }

      if (key.name === "escape") {
        state.commandBuffer = "";
        state.statusLine = "Command buffer cleared.";
        redraw();
        return;
      }

      if (key.name === "backspace" || key.name === "delete") {
        state.commandBuffer = Array.from(state.commandBuffer).slice(0, -1).join("");
        redraw();
        return;
      }

      if (key.name === "return") {
        const line = state.commandBuffer.trim();
        state.commandBuffer = "";
        if (!line) {
          redraw();
          return;
        }

        state.statusLine = `Running ${line.slice(0, 48)}${line.length > 48 ? "..." : ""}`;
        redraw();

        enqueue(async () => {
          const shouldContinue = await handleLine(line);
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
    focusedPane: "sessions",
    commandBuffer: "",
    statusLine: "Arrow keys navigate agents, runs, and task lanes. Type a prompt or /command and press Enter.",
  };
  const useAltScreen = process.stdout.isTTY && !options.noAltScreen;

  await restoreDurableSupervisorState({
    eventLog,
    orchestrator,
    store: supervisor.store,
  });
  await supervisor.start();
  await supervisor.refreshSkills();

  Object.assign(state, reconcileNavigationState(supervisor.snapshot(), state));

  const redraw = () => {
    Object.assign(state, reconcileNavigationState(supervisor.snapshot(), state));
    clearScreen();
    process.stdout.write(
      `${renderDashboard(supervisor.snapshot(), {
        activeSessionId: state.activeSessionId,
        activeRunId: state.activeRunId,
        selectedTaskId: state.selectedTaskId,
        focusedPane: state.focusedPane,
        commandBuffer: state.commandBuffer,
        width: process.stdout.columns ?? 100,
        color: process.stdout.isTTY,
        statusLine: state.statusLine,
      })}\n\n`,
    );
  };

  const handleLine = async (line) => {
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

    Object.assign(state, reconcileNavigationState(supervisor.snapshot(), state));
    return true;
  };

  const onSupervisorEvent = () => {
    Object.assign(state, reconcileNavigationState(supervisor.snapshot(), state));
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
