import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import { EventEmitter } from "node:events";
import { promises as fs } from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { Readable, Writable } from "node:stream";
import tls from "node:tls";
import * as acp from "@agentclientprotocol/sdk";

export const PERMISSION_KINDS = ["allow_always", "allow_once", "reject_once", "reject_always"];
const COPILOT_CA_HOST = "api.individual.githubcopilot.com";
let cachedCopilotExtraCaPath = null;

function mergeUniqueStrings(...groups) {
  const values = [];
  const seen = new Set();

  for (const group of groups) {
    for (const value of group ?? []) {
      if (typeof value !== "string") {
        continue;
      }
      const normalized = value.trim();
      if (!normalized || seen.has(normalized)) {
        continue;
      }
      seen.add(normalized);
      values.push(normalized);
    }
  }

  return values;
}

export function chooseAutoPermission(options) {
  for (const kind of PERMISSION_KINDS) {
    const match = options.find((option) => option.kind === kind);
    if (match) {
      return match;
    }
  }

  return null;
}

function trimToByteLimit(text, byteLimit) {
  if (!byteLimit || Buffer.byteLength(text, "utf8") <= byteLimit) {
    return { text, truncated: false };
  }

  const chars = Array.from(text);
  let total = 0;
  const kept = [];

  for (let index = chars.length - 1; index >= 0; index -= 1) {
    const char = chars[index];
    const bytes = Buffer.byteLength(char, "utf8");
    if (total + bytes > byteLimit) {
      break;
    }
    kept.push(char);
    total += bytes;
  }

  return { text: kept.reverse().join(""), truncated: true };
}

function createDeferred() {
  let resolve;
  let reject;
  const promise = new Promise((nextResolve, nextReject) => {
    resolve = nextResolve;
    reject = nextReject;
  });

  return { promise, resolve, reject };
}

function pemEncode(buffer) {
  const base64 = buffer.toString("base64");
  const lines = base64.match(/.{1,64}/g) ?? [];
  return `-----BEGIN CERTIFICATE-----\n${lines.join("\n")}\n-----END CERTIFICATE-----\n`;
}

async function ensureCopilotExtraCaFile() {
  if (process.env.NODE_EXTRA_CA_CERTS) {
    return process.env.NODE_EXTRA_CA_CERTS;
  }

  if (cachedCopilotExtraCaPath) {
    return cachedCopilotExtraCaPath;
  }

  const certificates = await new Promise((resolve, reject) => {
    const socket = tls.connect({
      host: COPILOT_CA_HOST,
      port: 443,
      servername: COPILOT_CA_HOST,
      rejectUnauthorized: true,
    });

    socket.once("secureConnect", () => {
      try {
        const seen = new Set();
        const buffers = [];
        let certificate = socket.getPeerCertificate(true);

        while (certificate?.raw) {
          const fingerprint = certificate.fingerprint256 ?? certificate.fingerprint ?? certificate.subject?.CN;
          if (fingerprint && seen.has(fingerprint)) {
            break;
          }
          if (fingerprint) {
            seen.add(fingerprint);
          }
          buffers.push(certificate.raw);

          if (!certificate.issuerCertificate || certificate.issuerCertificate === certificate) {
            break;
          }
          certificate = certificate.issuerCertificate;
        }

        socket.end();
        resolve(buffers);
      } catch (error) {
        reject(error);
      }
    });

    socket.once("error", reject);
  });

  const pem = certificates.map((buffer) => pemEncode(buffer)).join("");
  const outputPath = path.join(os.tmpdir(), "vorker-copilot-extra-ca.pem");
  await fs.writeFile(outputPath, pem, "utf8");
  cachedCopilotExtraCaPath = outputPath;
  return outputPath;
}

class CopilotBridgeClient {
  constructor(session) {
    this.session = session;
    this.terminals = new Map();
  }

  async requestPermission(params) {
    return await this.session.handlePermissionRequest(params);
  }

  async sessionUpdate(params) {
    this.session.handleSessionUpdate(params);
  }

  async readTextFile(params) {
    const content = await fs.readFile(params.path, "utf8");
    if (!params.line && !params.limit) {
      return { content };
    }

    const lines = content.split("\n");
    const start = Math.max((params.line ?? 1) - 1, 0);
    const end = params.limit ? start + params.limit : undefined;
    return { content: lines.slice(start, end).join("\n") };
  }

  async writeTextFile(params) {
    await fs.mkdir(path.dirname(params.path), { recursive: true });
    await fs.writeFile(params.path, params.content, "utf8");
    return {};
  }

  async createTerminal(params) {
    const terminalId = randomUUID();
    const env = { ...process.env };

    for (const variable of params.env ?? []) {
      env[variable.name] = variable.value;
    }

    const exitDeferred = createDeferred();
    const terminal = {
      output: "",
      truncated: false,
      exitStatus: null,
      process: null,
      exitDeferred,
    };

    const command = spawn(params.command, params.args ?? [], {
      cwd: params.cwd ?? this.session.cwd,
      env,
      stdio: ["ignore", "pipe", "pipe"],
    });

    terminal.process = command;

    const append = (chunk) => {
      terminal.output += chunk.toString("utf8");
      const trimmed = trimToByteLimit(terminal.output, params.outputByteLimit ?? null);
      terminal.output = trimmed.text;
      terminal.truncated ||= trimmed.truncated;
      this.session.emitEvent("terminal_output", {
        terminalId,
        output: terminal.output,
        truncated: terminal.truncated,
        exitStatus: terminal.exitStatus,
      });
    };

    command.stdout.on("data", append);
    command.stderr.on("data", append);
    command.once("error", (error) => {
      append(Buffer.from(`process error: ${error.message}\n`, "utf8"));
      exitDeferred.reject(error);
    });
    command.once("exit", (exitCode, signal) => {
      terminal.exitStatus = {
        exitCode: typeof exitCode === "number" ? exitCode : null,
        signal: signal ?? null,
      };
      exitDeferred.resolve(terminal.exitStatus);
      this.session.emitEvent("terminal_exit", {
        terminalId,
        exitStatus: terminal.exitStatus,
      });
    });

    this.terminals.set(terminalId, terminal);
    return { terminalId };
  }

  async terminalOutput(params) {
    const terminal = this.requireTerminal(params.terminalId);
    return {
      output: terminal.output,
      truncated: terminal.truncated,
      exitStatus: terminal.exitStatus,
    };
  }

  async waitForTerminalExit(params) {
    const terminal = this.requireTerminal(params.terminalId);
    return await terminal.exitDeferred.promise;
  }

  async killTerminal(params) {
    const terminal = this.requireTerminal(params.terminalId);
    if (terminal.process && !terminal.process.killed) {
      terminal.process.kill("SIGTERM");
    }
    return {};
  }

  async releaseTerminal(params) {
    const terminal = this.requireTerminal(params.terminalId);
    if (terminal.process && !terminal.exitStatus && !terminal.process.killed) {
      terminal.process.kill("SIGTERM");
    }
    this.terminals.delete(params.terminalId);
    return {};
  }

  async shutdown() {
    for (const [terminalId, terminal] of this.terminals.entries()) {
      if (terminal.process && !terminal.process.killed) {
        terminal.process.kill("SIGTERM");
      }
      this.terminals.delete(terminalId);
    }
  }

  requireTerminal(terminalId) {
    const terminal = this.terminals.get(terminalId);
    if (!terminal) {
      throw new Error(`Unknown terminal: ${terminalId}`);
    }
    return terminal;
  }
}

export class CopilotSession extends EventEmitter {
  constructor(options = {}) {
    super();
    this.id = options.id ?? randomUUID();
    this.name = options.name ?? `Agent ${this.id.slice(0, 8)}`;
    this.cwd = path.resolve(options.cwd ?? process.cwd());
    this.copilotBin = options.copilotBin ?? process.env.COPILOT_BIN ?? "copilot";
    this.mode = options.mode ?? null;
    this.model = options.model ?? null;
    this.role = options.role ?? "worker";
    this.notes = typeof options.notes === "string" ? options.notes.trim() : "";
    this.skillIds = mergeUniqueStrings(options.skillIds ?? []);
    this.autoApprove = Boolean(options.autoApprove);
    this.debug = Boolean(options.debug);
    this.permissionHandler = options.permissionHandler ?? null;
    this.createdAt = new Date().toISOString();
    this.status = "created";
    this.busy = false;
    this.queue = Promise.resolve();
    this.child = null;
    this.client = null;
    this.connection = null;
    this.sessionId = null;
    this.title = null;
    this.currentModeId = this.mode;
    this.currentModelId = this.model;
    this.availableModes = [];
    this.availableModels = [];
    this.lastPromptAt = null;
    this.lastResponseAt = null;
    this.currentResponseText = null;
    this.closed = false;
  }

  snapshot() {
    return {
      id: this.id,
      name: this.name,
      cwd: this.cwd,
      status: this.status,
      busy: this.busy,
      sessionId: this.sessionId,
      title: this.title,
      role: this.role,
      notes: this.notes,
      mode: this.currentModeId,
      model: this.currentModelId,
      skillIds: [...this.skillIds],
      availableModes: this.availableModes,
      availableModels: this.availableModels,
      autoApprove: this.autoApprove,
      createdAt: this.createdAt,
      lastPromptAt: this.lastPromptAt,
      lastResponseAt: this.lastResponseAt,
    };
  }

  emitEvent(type, payload = {}) {
    const event = {
      agentId: this.id,
      type,
      ...payload,
    };

    this.emit(type, event);
    this.emit("event", event);
  }

  async start() {
    if (this.connection) {
      return this.snapshot();
    }

    this.status = "starting";
    this.emitEvent("agent_state", { agent: this.snapshot() });

    const extraCaPath = await ensureCopilotExtraCaFile();
    const childEnv = {
      ...process.env,
      NODE_EXTRA_CA_CERTS: process.env.NODE_EXTRA_CA_CERTS ?? extraCaPath,
    };

    const child = spawn(this.copilotBin, ["--acp"], {
      cwd: this.cwd,
      env: childEnv,
      stdio: ["pipe", "pipe", "inherit"],
    });

    const childReady = new Promise((resolve, reject) => {
      child.once("spawn", resolve);
      child.once("error", reject);
    });

    await childReady;

    this.child = child;
    this.client = new CopilotBridgeClient(this);
    const stream = acp.ndJsonStream(Writable.toWeb(child.stdin), Readable.toWeb(child.stdout));
    this.connection = new acp.ClientSideConnection(() => this.client, stream);

    child.once("exit", (code, signal) => {
      this.status = this.closed ? "closed" : "stopped";
      this.emitEvent("process_exit", {
        code,
        signal,
        agent: this.snapshot(),
      });
      this.emitEvent("agent_state", { agent: this.snapshot() });
    });

    try {
      const initResult = await this.connection.initialize({
        protocolVersion: acp.PROTOCOL_VERSION,
        clientInfo: {
          name: "vorker",
          title: "Vorker",
          version: "0.2.0",
        },
        clientCapabilities: {
          fs: {
            readTextFile: true,
            writeTextFile: true,
          },
          terminal: true,
        },
      });

      this.emitEvent("initialized", {
        protocolVersion: initResult.protocolVersion,
        agentInfo: initResult.agentInfo ?? null,
      });

      const session = await this.connection.newSession({
        cwd: this.cwd,
        mcpServers: [],
      });

      this.sessionId = session.sessionId;
      this.availableModes = session.modes?.availableModes ?? [];
      this.currentModeId = session.modes?.currentModeId ?? this.currentModeId;
      this.availableModels = session.models?.availableModels ?? [];
      this.currentModelId = session.models?.currentModelId ?? this.currentModelId;

      if (this.mode && this.connection.setSessionMode) {
        await this.connection.setSessionMode({
          sessionId: this.sessionId,
          modeId: this.mode,
        });
        this.currentModeId = this.mode;
      }

      if (this.model && this.connection.unstable_setSessionModel) {
        await this.connection.unstable_setSessionModel({
          sessionId: this.sessionId,
          modelId: this.model,
        });
        this.currentModelId = this.model;
      }

      this.status = "ready";
      this.emitEvent("agent_state", { agent: this.snapshot() });
      return this.snapshot();
    } catch (error) {
      this.status = "error";
      this.emitEvent("error", {
        stage: "start",
        message: error instanceof Error ? error.message : String(error),
      });
      await this.close();
      throw error;
    }
  }

  updateProfile(updates = {}) {
    if (typeof updates.name === "string" && updates.name.trim()) {
      this.name = updates.name.trim();
    }
    if (typeof updates.role === "string" && updates.role.trim()) {
      this.role = updates.role.trim();
    }
    if (typeof updates.notes === "string") {
      this.notes = updates.notes.trim();
    }
    if (Array.isArray(updates.skillIds)) {
      this.skillIds = mergeUniqueStrings(updates.skillIds);
    }
    if (typeof updates.autoApprove === "boolean") {
      this.autoApprove = updates.autoApprove;
    }

    this.emitEvent("agent_state", { agent: this.snapshot() });
    return this.snapshot();
  }

  async prompt(text, options = {}) {
    if (!this.connection || !this.sessionId) {
      throw new Error("Agent session is not ready.");
    }

    const promptText = String(text ?? "").trim();
    const displayText = String(options.displayText ?? promptText).trim() || promptText;
    if (!promptText) {
      throw new Error("Prompt text is required.");
    }

    const promptId = randomUUID();

    return await this.enqueue(async () => {
      this.busy = true;
      this.lastPromptAt = new Date().toISOString();
      this.currentResponseText = "";
      this.emitEvent("prompt_started", {
        promptId,
        text: displayText,
        agent: this.snapshot(),
      });
      this.emitEvent("agent_state", { agent: this.snapshot() });

      try {
        const result = await this.connection.prompt({
          sessionId: this.sessionId,
          prompt: [
            {
              type: "text",
              text: promptText,
            },
          ],
        });

        const responseText = this.currentResponseText ?? "";
        this.lastResponseAt = new Date().toISOString();
        this.emitEvent("prompt_finished", {
          promptId,
          stopReason: result.stopReason,
          usage: result.usage ?? null,
          responseText,
        });
        return {
          ...result,
          responseText,
        };
      } catch (error) {
        this.emitEvent("error", {
          stage: "prompt",
          promptId,
          message: error instanceof Error ? error.message : String(error),
        });
        throw error;
      } finally {
        this.busy = false;
        this.currentResponseText = null;
        this.emitEvent("agent_state", { agent: this.snapshot() });
      }
    });
  }

  async setMode(modeId) {
    if (!this.connection?.setSessionMode || !this.sessionId) {
      throw new Error("This Copilot session does not expose session modes.");
    }

    await this.connection.setSessionMode({
      sessionId: this.sessionId,
      modeId,
    });
    this.currentModeId = modeId;
    this.emitEvent("mode_changed", {
      modeId,
      agent: this.snapshot(),
    });
    this.emitEvent("agent_state", { agent: this.snapshot() });
  }

  async setModel(modelId) {
    if (!this.connection?.unstable_setSessionModel || !this.sessionId) {
      throw new Error("This Copilot session does not expose model selection.");
    }

    await this.connection.unstable_setSessionModel({
      sessionId: this.sessionId,
      modelId,
    });
    this.currentModelId = modelId;
    this.emitEvent("model_changed", {
      modelId,
      agent: this.snapshot(),
    });
    this.emitEvent("agent_state", { agent: this.snapshot() });
  }

  async close() {
    if (this.closed) {
      return;
    }

    this.closed = true;
    this.status = "closed";

    try {
      await this.client?.shutdown();
    } catch {
      // Ignore cleanup failures.
    }

    if (this.child && !this.child.killed) {
      this.child.kill("SIGTERM");
    }

    this.emitEvent("closed", { agent: this.snapshot() });
    this.emitEvent("agent_state", { agent: this.snapshot() });
  }

  async handlePermissionRequest(params) {
    if (this.autoApprove) {
      const selected = chooseAutoPermission(params.options);
      if (!selected) {
        return { outcome: { outcome: "cancelled" } };
      }

      this.emitEvent("permission_auto_selected", {
        title: params.toolCall.title ?? "Tool call",
        optionId: selected.optionId,
        optionName: selected.name,
      });
      return {
        outcome: {
          outcome: "selected",
          optionId: selected.optionId,
        },
      };
    }

    if (this.permissionHandler) {
      return await this.permissionHandler({
        agent: this,
        request: params,
      });
    }

    this.emitEvent("permission_denied", {
      title: params.toolCall.title ?? "Tool call",
    });
    return { outcome: { outcome: "cancelled" } };
  }

  handleSessionUpdate(params) {
    const { update } = params;

    switch (update.sessionUpdate) {
      case "agent_message_chunk":
        if (update.content.type === "text" && typeof update.content.text === "string" && this.currentResponseText !== null) {
          this.currentResponseText += update.content.text;
        }
        this.emitEvent("message_chunk", {
          messageId: update.messageId ?? null,
          content: update.content,
          text: update.content.type === "text" ? update.content.text : "",
        });
        break;
      case "tool_call":
        this.emitEvent("tool_call", {
          update,
        });
        break;
      case "tool_call_update":
        this.emitEvent("tool_call_update", {
          update,
        });
        break;
      case "plan":
        this.emitEvent("plan", {
          entries: update.entries,
        });
        break;
      case "current_mode_update":
        this.currentModeId = update.currentModeId;
        this.emitEvent("mode_changed", {
          modeId: update.currentModeId,
          agent: this.snapshot(),
        });
        this.emitEvent("agent_state", { agent: this.snapshot() });
        break;
      case "session_info_update":
        this.title = update.title ?? this.title;
        this.emitEvent("session_info", {
          title: update.title ?? null,
          updatedAt: update.updatedAt ?? null,
          agent: this.snapshot(),
        });
        this.emitEvent("agent_state", { agent: this.snapshot() });
        break;
      case "usage_update":
        this.emitEvent("usage", {
          usage: update.usage ?? null,
        });
        break;
      default:
        this.emitEvent("session_update", {
          update,
        });
        break;
    }
  }

  async enqueue(fn) {
    const next = this.queue.then(fn, fn);
    this.queue = next.catch(() => { });
    return await next;
  }
}

export class CopilotManager extends EventEmitter {
  constructor(options = {}) {
    super();
    this.defaults = options;
    this.sessions = new Map();
    this.skillCatalog = options.skillCatalog ?? null;
  }

  async createAgent(options = {}) {
    const session = new CopilotSession({
      ...this.defaults,
      ...options,
    });

    this.sessions.set(session.id, session);
    session.on("event", (event) => {
      this.emit("agent_event", event);
    });
    session.on("closed", () => {
      this.sessions.delete(session.id);
      this.emit("agents_changed", { agents: this.listAgents() });
    });

    await session.start();
    this.emit("agent_created", { agent: session.snapshot() });
    this.emit("agents_changed", { agents: this.listAgents() });
    return session;
  }

  async updateAgent(agentId, updates = {}) {
    const session = this.requireAgent(agentId);
    session.updateProfile(updates);

    if (typeof updates.mode === "string" && updates.mode.trim() && updates.mode.trim() !== session.currentModeId) {
      await session.setMode(updates.mode.trim());
    }

    if (typeof updates.model === "string" && updates.model.trim() && updates.model.trim() !== session.currentModelId) {
      await session.setModel(updates.model.trim());
    }

    this.emit("agents_changed", { agents: this.listAgents() });
    return session;
  }

  async promptAgent(agentId, text, options = {}) {
    const session = this.requireAgent(agentId);
    const finalPrompt = await this.buildPromptForAgent(session, text, options);
    return await session.prompt(finalPrompt, {
      displayText: options.displayText ?? text,
    });
  }

  async buildPromptForAgent(session, text, options = {}) {
    const promptText = String(text ?? "").trim();
    if (!promptText) {
      throw new Error("Prompt text is required.");
    }

    const contextSections = [];
    const role = typeof options.role === "string" && options.role.trim() ? options.role.trim() : session.role;
    const notes = typeof options.notes === "string" ? options.notes.trim() : session.notes;
    const skillIds = mergeUniqueStrings(session.skillIds, options.skillIds ?? []);

    if (role) {
      contextSections.push(`Agent role: ${role}`);
    }

    if (notes) {
      contextSections.push(`Agent notes:\n${notes}`);
    }

    if (this.skillCatalog && skillIds.length > 0) {
      const snippets = await this.skillCatalog.getSkillSnippets(skillIds);
      if (snippets.length > 0) {
        const rendered = snippets
          .map(
            (snippet) =>
              `Skill: ${snippet.name}\nPath: ${snippet.path}\n${snippet.content}${snippet.truncated ? "\n[Skill content truncated]" : ""}`,
          )
          .join("\n\n---\n\n");
        contextSections.push(`Attached skills:\n${rendered}`);
      }
    }

    for (const section of options.contextSections ?? []) {
      if (typeof section !== "string") {
        continue;
      }
      const trimmed = section.trim();
      if (trimmed) {
        contextSections.push(trimmed);
      }
    }

    if (contextSections.length === 0) {
      return promptText;
    }

    return `${contextSections.join("\n\n")}\n\nUser request:\n${promptText}`;
  }

  listAgents() {
    return Array.from(this.sessions.values()).map((session) => session.snapshot());
  }

  getAgent(agentId) {
    return this.sessions.get(agentId) ?? null;
  }

  requireAgent(agentId) {
    const session = this.getAgent(agentId);
    if (!session) {
      throw new Error(`Unknown agent: ${agentId}`);
    }
    return session;
  }

  async closeAgent(agentId) {
    const session = this.requireAgent(agentId);
    await session.close();
  }

  async closeAll() {
    await Promise.all(
      Array.from(this.sessions.values()).map(async (session) => {
        await session.close();
      }),
    );
    this.sessions.clear();
    this.emit("agents_changed", { agents: this.listAgents() });
  }
}
