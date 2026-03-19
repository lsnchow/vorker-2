import path from "node:path";
import process from "node:process";
import readline from "node:readline/promises";
import { CopilotManager } from "./copilot.js";
import { Orchestrator } from "./orchestrator.js";
import { SkillCatalog } from "./skills.js";
import { TunnelManager } from "./tunnel.js";
import { EventLog } from "./supervisor/event-log.js";
import { SupervisorService } from "./supervisor/service.js";
import { parseCommand } from "./tui/commands.js";
import { executeCommand } from "./tui/controller.js";
import { renderDashboard } from "./tui/render.js";

function clearScreen() {
  process.stdout.write("\x1bc");
}

function resolveProtocol(options) {
  return options.tlsKey && options.tlsCert ? "https" : "http";
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
    filePath: path.join(logsDir, `session-${Date.now()}.ndjson`),
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

  await supervisor.start();
  await supervisor.refreshSkills();

  const redraw = () => {
    clearScreen();
    process.stdout.write(
      `${renderDashboard(supervisor.snapshot(), {
        activeSessionId: state.activeSessionId,
        activeRunId: state.activeRunId,
        width: process.stdout.columns ?? 100,
      })}\n\n${state.statusLine}\n`,
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
    supervisor.off("event", onSupervisorEvent);
    rl.close();
    await supervisor.close();
    await manager.closeAll();
    if (tunnelManager.child) {
      await tunnelManager.stop();
    }
  }
}
