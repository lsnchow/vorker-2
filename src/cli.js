import path from "node:path";
import process from "node:process";
import readline from "node:readline/promises";
import { CopilotSession, chooseAutoPermission } from "./copilot.js";

async function readPrompt(promptParts) {
  if (promptParts.length > 0) {
    return promptParts.join(" ").trim();
  }

  if (process.stdin.isTTY) {
    return "";
  }

  const chunks = [];
  for await (const chunk of process.stdin) {
    chunks.push(chunk);
  }

  return Buffer.concat(chunks).toString("utf8").trim();
}

function attachCliOutput(session, options) {
  let lastMessageId = null;
  let turnHadOutput = false;

  session.on("message_chunk", (event) => {
    const messageChanged = event.messageId && event.messageId !== lastMessageId;
    if (messageChanged && turnHadOutput) {
      process.stdout.write("\n");
    }

    lastMessageId = event.messageId ?? lastMessageId;
    process.stdout.write(event.text);
    turnHadOutput = true;
  });

  session.on("tool_call", (event) => {
    process.stderr.write(`\n[tool] ${event.update.title} (${event.update.status})\n`);
  });

  session.on("tool_call_update", (event) => {
    process.stderr.write(`\n[tool] ${event.update.title ?? event.update.toolCallId} (${event.update.status})\n`);
  });

  session.on("plan", (event) => {
    if (!options.debug) {
      return;
    }

    process.stderr.write("\n[plan]\n");
    for (const entry of event.entries) {
      process.stderr.write(`  - ${entry.status}: ${entry.content}\n`);
    }
  });

  session.on("usage", (event) => {
    if (!options.debug || !event.usage) {
      return;
    }

    process.stderr.write(
      `\n[usage] input=${event.usage.inputTokens ?? "?"} output=${event.usage.outputTokens ?? "?"}\n`,
    );
  });

  session.on("error", (event) => {
    process.stderr.write(`\n[error:${event.stage}] ${event.message}\n`);
  });

  session.on("prompt_finished", () => {
    if (turnHadOutput) {
      process.stdout.write("\n");
    }
    turnHadOutput = false;
    lastMessageId = null;
  });
}

function createPermissionController(options) {
  let rl = null;

  const getReadline = async () => {
    if (!rl) {
      rl = readline.createInterface({
        input: process.stdin,
        output: process.stderr,
      });
    }
    return rl;
  };

  return {
    close() {
      rl?.close();
      rl = null;
    },
    async handler({ request }) {
      process.stderr.write(`\n[permission] ${request.toolCall.title ?? "Tool call"}\n`);
      request.options.forEach((option, index) => {
        process.stderr.write(`  ${index + 1}. ${option.name} [${option.kind}]\n`);
      });

      if (options.autoApprove) {
        const selected = chooseAutoPermission(request.options);
        if (!selected) {
          return { outcome: { outcome: "cancelled" } };
        }

        process.stderr.write(`  -> auto-selected: ${selected.name}\n`);
        return {
          outcome: {
            outcome: "selected",
            optionId: selected.optionId,
          },
        };
      }

      const lineReader = await getReadline();
      while (true) {
        const answer = (await lineReader.question("Select permission option by number, or 'c' to cancel: ")).trim();
        if (answer.toLowerCase() === "c") {
          return { outcome: { outcome: "cancelled" } };
        }

        const index = Number.parseInt(answer, 10) - 1;
        const selected = request.options[index];
        if (selected) {
          return {
            outcome: {
              outcome: "selected",
              optionId: selected.optionId,
            },
          };
        }
      }
    },
  };
}

export async function runChat(options) {
  const promptText = await readPrompt(options.promptParts);
  if (!promptText) {
    throw new Error("No prompt provided.");
  }

  const permissions = createPermissionController(options);
  const session = new CopilotSession({
    cwd: options.cwd,
    copilotBin: options.copilotBin,
    mode: options.mode,
    model: options.model,
    autoApprove: options.autoApprove,
    debug: options.debug,
    permissionHandler: permissions.handler,
  });

  attachCliOutput(session, options);

  try {
    await session.start();
    await session.prompt(promptText);
  } finally {
    permissions.close();
    await session.close();
  }
}

export async function runRepl(options) {
  const permissions = createPermissionController(options);
  const session = new CopilotSession({
    cwd: options.cwd,
    copilotBin: options.copilotBin,
    mode: options.mode,
    model: options.model,
    autoApprove: options.autoApprove,
    debug: options.debug,
    permissionHandler: permissions.handler,
  });

  attachCliOutput(session, options);
  await session.start();

  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stderr,
  });

  try {
    process.stderr.write(`Connected in ${path.resolve(options.cwd)}\n`);
    process.stderr.write(`Agent: ${session.name}\n`);
    process.stderr.write(`Commands: /exit, /mode <id>, /model <id>, /help\n`);

    while (true) {
      const line = (await rl.question("> ")).trim();
      if (!line) {
        continue;
      }

      if (line === "/exit" || line === "/quit") {
        break;
      }

      if (line === "/help") {
        process.stderr.write("Prompt text sends a turn. /mode and /model change session settings.\n");
        continue;
      }

      if (line.startsWith("/mode ")) {
        await session.setMode(line.slice("/mode ".length).trim());
        continue;
      }

      if (line.startsWith("/model ")) {
        await session.setModel(line.slice("/model ".length).trim());
        continue;
      }

      await session.prompt(line);
    }
  } finally {
    rl.close();
    permissions.close();
    await session.close();
  }
}
