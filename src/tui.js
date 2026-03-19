import path from "node:path";
import process from "node:process";
import readline from "node:readline/promises";
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
    statusLine:
      "Use /agent <name> to create an agent. Tunneling expects the web server to be running separately on the configured host and port.",
  };
  const useAltScreen = process.stdout.isTTY && !options.noAltScreen;

  await restoreDurableSupervisorState({
    eventLog,
    orchestrator,
    store: supervisor.store,
  });
  await supervisor.start();
  await supervisor.refreshSkills();

  const redraw = () => {
    clearScreen();
    process.stdout.write(
      `${renderDashboard(supervisor.snapshot(), {
        activeSessionId: state.activeSessionId,
        activeRunId: state.activeRunId,
        width: process.stdout.columns ?? 100,
        color: process.stdout.isTTY,
        statusLine: state.statusLine,
      })}\n\n`,
    );
  };

  const onSupervisorEvent = () => {
    redraw();
  };

  supervisor.on("event", onSupervisorEvent);

  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });

  try {
    if (useAltScreen) {
      enterAltScreen();
    }
    process.stdout.on("resize", redraw);
    redraw();

    while (true) {
      const line = await rl.question("> ");
      const command = parseCommand(line);
      if (command.type === "quit") {
        break;
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

      redraw();
    }
  } finally {
    process.stdout.off("resize", redraw);
    supervisor.off("event", onSupervisorEvent);
    rl.close();
    await supervisor.close();
    await manager.closeAll();
    if (tunnelManager.child) {
      await tunnelManager.stop();
    }
    if (useAltScreen) {
      exitAltScreen();
    }
  }
}
