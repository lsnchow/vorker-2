"use client";

import { useEffect, useRef, useState, useCallback } from "react";

// ── Types ──────────────────────────────────────────────────────────────

export interface Agent {
  id: string;
  name: string;
  role?: string;
  status?: string;
  mode?: string;
  model?: string;
  cwd?: string;
  notes?: string;
  autoApprove?: boolean;
  skillIds?: string[];
  availableModels?: Array<string | { id?: string; label?: string; name?: string }>;
  availableModes?: Array<string | { id?: string; label?: string; name?: string }>;
}

export interface Task {
  id: string;
  title: string;
  description?: string;
  status?: string;
  assignedAgentId?: string;
  modeId?: string;
  modelId?: string;
  skillIds?: string[];
}

export interface Run {
  id: string;
  name: string;
  goal?: string;
  status?: string;
  notes?: string;
  arbitratorAgentId?: string;
  workerAgentIds: string[];
  tasks: Task[];
  updatedAt: string;
}

export interface Skill {
  id: string;
  name: string;
  description?: string;
}

export interface ShareState {
  state: string;
  publicUrl?: string;
}

export interface PermissionOption {
  optionId: string;
  name: string;
  kind: string;
}

export interface PendingPermission {
  requestId: string;
  toolCall?: { title?: string };
  options: PermissionOption[];
}

export interface ActivityItem {
  id: string;
  timestamp: string;
  summary: string;
}

export interface TranscriptEntry {
  role: "user" | "agent" | "system";
  body: string;
}

export interface VorkerState {
  authenticated: boolean;
  secureTransport: boolean;
  serverCwd: string;
  pairingPassword: string;
  transportMode: "offline" | "websocket" | "polling";
  agents: Agent[];
  runs: Run[];
  skills: Skill[];
  share: ShareState | null;
  activeAgentId: string | null;
  activeRunId: string | null;
  activeTaskId: string | null;
  transcripts: Record<string, TranscriptEntry[]>;
  pendingAgentMessage: Record<string, number>;
  pendingPermission: PendingPermission | null;
  pollCursor: number;
  activity: ActivityItem[];
}

export interface AgentForm {
  name: string;
  role: string;
  cwd: string;
  mode: string;
  model: string;
  notes: string;
  autoApprove: boolean;
  skillIds: string[];
}

export interface RunForm {
  name: string;
  goal: string;
  notes: string;
  arbitratorAgentId: string;
  workerAgentIds: string[];
}

export interface TaskForm {
  title: string;
  description: string;
  assignedAgentId: string;
  modeId: string;
  modelId: string;
  skillIds: string[];
}

export interface ShareForm {
  cloudflaredBin: string;
  edgeProtocol: string;
  edgeIpVersion: string;
}

const COPILOT_MODEL_CHOICES = [
  "claude-sonnet-4.6",
  "claude-sonnet-4.5",
  "claude-haiku-4.5",
  "claude-opus-4.6",
  "claude-opus-4.6-fast",
  "claude-opus-4.5",
  "claude-sonnet-4",
  "gemini-3-pro-preview",
  "gpt-5.4",
  "gpt-5.3-codex",
  "gpt-5.2-codex",
  "gpt-5.2",
  "gpt-5.1-codex-max",
  "gpt-5.1-codex",
  "gpt-5.1",
  "gpt-5.1-codex-mini",
  "gpt-5-mini",
  "gpt-4.1",
] as const;

const ACP_MODE_CHOICES = [
  "https://agentclientprotocol.com/protocol/session-modes#agent",
  "https://agentclientprotocol.com/protocol/session-modes#autopilot",
  "https://agentclientprotocol.com/protocol/session-modes#plan",
] as const;

// ── Helpers ────────────────────────────────────────────────────────────

function initialState(): VorkerState {
  return {
    authenticated: false,
    secureTransport: false,
    serverCwd: "",
    pairingPassword: "",
    transportMode: "offline",
    agents: [],
    runs: [],
    skills: [],
    share: null,
    activeAgentId: null,
    activeRunId: null,
    activeTaskId: null,
    transcripts: {},
    pendingAgentMessage: {},
    pendingPermission: null,
    pollCursor: 0,
    activity: [],
  };
}

export function emptyAgentForm(): AgentForm {
  return { name: "", role: "worker", cwd: "", mode: "", model: "", notes: "", autoApprove: false, skillIds: [] };
}

export function emptyRunForm(): RunForm {
  return { name: "", goal: "", notes: "", arbitratorAgentId: "", workerAgentIds: [] };
}

export function emptyTaskForm(): TaskForm {
  return { title: "", description: "", assignedAgentId: "", modeId: "", modelId: "", skillIds: [] };
}

function upsertAgent(agents: Agent[], agent: Agent): Agent[] {
  const next = [...agents];
  const idx = next.findIndex((a) => a.id === agent.id);
  if (idx === -1) next.push(agent);
  else next[idx] = agent;
  return next;
}

function upsertRun(runs: Run[], run: Run): Run[] {
  const next = [...runs];
  const idx = next.findIndex((r) => r.id === run.id);
  if (idx === -1) next.push(run);
  else next[idx] = run;
  return next.sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
}

function addActivity(activity: ActivityItem[], summary: string): ActivityItem[] {
  return [
    { id: `${Date.now()}-${Math.random().toString(16).slice(2, 8)}`, timestamp: new Date().toISOString(), summary },
    ...activity,
  ].slice(0, 120);
}

function summarizeEvent(msg: any): string | null {
  switch (msg.type) {
    case "agent_created": return `Agent created: ${msg.agent?.name ?? msg.agent?.id}`;
    case "agent_state": return `Agent ${msg.agent?.name ?? msg.agent?.id} is ${msg.agent?.status}`;
    case "prompt_started": return `Prompt started on ${msg.agentId}`;
    case "prompt_finished": return `Prompt finished on ${msg.agentId}`;
    case "run_created": return `Run created: ${msg.run?.name}`;
    case "run_updated": return `Run updated: ${msg.run?.name} (${msg.run?.status})`;
    case "task_created": return `Task created: ${msg.task?.title}`;
    case "task_updated": return `Task updated: ${msg.task?.title} (${msg.task?.status})`;
    case "share_state": return `Share state: ${msg.share?.state}`;
    case "share_log": return msg.entry?.line ?? null;
    case "skills_updated": return `Skills refreshed (${msg.skills?.length ?? 0})`;
    case "error":
    case "agent_error": return `Error: ${msg.message}`;
    default: return null;
  }
}

export function transportLabel(app: VorkerState): string {
  const channel = app.transportMode === "websocket" ? "WebSocket" : app.transportMode === "polling" ? "Polling" : "Offline";
  return `${app.secureTransport ? "HTTPS" : "HTTP"} \u2022 ${channel}`;
}

export function getActiveAgent(app: VorkerState): Agent | null {
  return app.agents.find((a) => a.id === app.activeAgentId) ?? null;
}

export function getActiveRun(app: VorkerState): Run | null {
  return app.runs.find((r) => r.id === app.activeRunId) ?? null;
}

export function getActiveTask(app: VorkerState): Task | null {
  const run = getActiveRun(app);
  return run?.tasks.find((t) => t.id === app.activeTaskId) ?? null;
}

export function uniqueValues(values: (string | undefined | null)[]): string[] {
  return [...new Set(values.filter((v): v is string => v != null && v !== "" && typeof v === "string"))].sort((a, b) => a.localeCompare(b));
}

function extractChoiceValue(value: string | { id?: string; label?: string; name?: string } | undefined | null): string | null {
  if (typeof value === "string") return value.trim() || null;
  if (!value || typeof value !== "object") return null;
  return value.id?.trim() || value.label?.trim() || value.name?.trim() || null;
}

// ── Hook ───────────────────────────────────────────────────────────────

export function useVorkerState() {
  const [app, setApp] = useState<VorkerState>(initialState);
  const [loginPassword, setLoginPassword] = useState("");
  const [authError, setAuthError] = useState("");
  const [bootError, setBootError] = useState("");
  const [booting, setBooting] = useState(true);
  const [clientOrigin, setClientOrigin] = useState("");
  const [inspectorTab, setInspectorTab] = useState<"tasks" | "agent" | "create">("tasks");

  const [createAgentForm, setCreateAgentForm] = useState<AgentForm>(emptyAgentForm);
  const [agentEditorForm, setAgentEditorForm] = useState<AgentForm>(emptyAgentForm);
  const [createRunForm, setCreateRunForm] = useState<RunForm>(emptyRunForm);
  const [taskForm, setTaskForm] = useState<TaskForm>(emptyTaskForm);
  const [shareForm, setShareForm] = useState<ShareForm>({ cloudflaredBin: "cloudflared", edgeProtocol: "http2", edgeIpVersion: "auto" });

  const wsRef = useRef<WebSocket | null>(null);
  const pollAbortRef = useRef<AbortController | null>(null);
  const appRef = useRef(app);
  const bootedRef = useRef(false);
  const csrfTokenRef = useRef("");

  useEffect(() => { appRef.current = app; }, [app]);
  useEffect(() => { setClientOrigin(window.location.origin); }, []);

  // ── Fetch helper ─────────────────────────────────────────────────

  const fetchJson = useCallback(async (url: string, options: RequestInit = {}) => {
    const method = String(options.method ?? "GET").toUpperCase();
    const headers = new Headers(options.headers ?? {});
    if (method !== "GET" && method !== "HEAD" && method !== "OPTIONS") {
      headers.set("X-Vorker-Requested-With", "browser");
      if (csrfTokenRef.current) {
        headers.set("X-Vorker-CSRF", csrfTokenRef.current);
      }
    }

    const res = await fetch(url, { credentials: "same-origin", cache: "no-store", ...options, headers });
    const body = await res.json().catch(() => ({}));
    if (!res.ok) throw new Error(body.error ?? `Request failed (${res.status})`);
    return body;
  }, []);

  // ── Event reducer ────────────────────────────────────────────────

  const applyEvent = useCallback((message: any, replay = false) => {
    setApp((cur) => {
      let next = cur;

      switch (message.type) {
        case "hello":
          next = { ...cur, secureTransport: Boolean(message.secureTransport), agents: message.agents ?? cur.agents, runs: message.runs ?? cur.runs, skills: message.skills ?? cur.skills, share: message.share ?? cur.share };
          break;
        case "agents":
          next = { ...cur, agents: message.agents ?? [], activeAgentId: message.agents?.find((a: Agent) => a.id === cur.activeAgentId)?.id ?? message.agents?.[0]?.id ?? null };
          break;
        case "create_agent_ok":
        case "agent_created":
        case "agent_state": {
          const agents = upsertAgent(cur.agents, message.agent);
          next = { ...cur, agents, activeAgentId: cur.activeAgentId ?? message.agent.id };
          break;
        }
        case "runs":
          next = { ...cur, runs: message.runs ?? [], activeRunId: message.runs?.find((r: Run) => r.id === cur.activeRunId)?.id ?? message.runs?.[0]?.id ?? null };
          break;
        case "run_created":
        case "run_updated": {
          const runs = upsertRun(cur.runs, message.run);
          const activeRunId = cur.activeRunId ?? message.run.id;
          const activeRun = runs.find((r) => r.id === activeRunId) ?? runs[0] ?? null;
          next = { ...cur, runs, activeRunId: activeRun?.id ?? null, activeTaskId: activeRun?.tasks.find((t) => t.id === cur.activeTaskId)?.id ?? activeRun?.tasks[0]?.id ?? null };
          break;
        }
        case "task_created":
        case "task_updated": {
          const runs = message.run ? upsertRun(cur.runs, message.run) : cur.runs;
          const activeRun = runs.find((r) => r.id === cur.activeRunId) ?? runs[0] ?? null;
          next = { ...cur, runs, activeRunId: activeRun?.id ?? cur.activeRunId ?? null, activeTaskId: cur.activeTaskId ?? message.task?.id ?? activeRun?.tasks[0]?.id ?? null };
          break;
        }
        case "skills_updated":
          next = { ...cur, skills: message.skills ?? [] };
          break;
        case "share_state":
          next = { ...cur, share: message.share ?? cur.share };
          break;
        case "message_chunk": {
          const transcript = [...(cur.transcripts[message.agentId] ?? [])];
          const pending = cur.pendingAgentMessage[message.agentId];
          const pendingAgentMessage = { ...cur.pendingAgentMessage };
          if (pending != null) {
            transcript[pending] = { ...transcript[pending], body: `${transcript[pending].body}${message.text ?? ""}` };
          } else {
            transcript.push({ role: "agent", body: message.text ?? "" });
            pendingAgentMessage[message.agentId] = transcript.length - 1;
          }
          next = { ...cur, transcripts: { ...cur.transcripts, [message.agentId]: transcript }, pendingAgentMessage };
          break;
        }
        case "prompt_started": {
          const transcript = [...(cur.transcripts[message.agentId] ?? []), { role: "user" as const, body: message.text }];
          const pendingAgentMessage = { ...cur.pendingAgentMessage };
          delete pendingAgentMessage[message.agentId];
          next = { ...cur, transcripts: { ...cur.transcripts, [message.agentId]: transcript }, pendingAgentMessage };
          break;
        }
        case "prompt_finished": {
          const transcript = [...(cur.transcripts[message.agentId] ?? []), { role: "system" as const, body: `Prompt finished: ${message.stopReason ?? "unknown"}` }];
          const pendingAgentMessage = { ...cur.pendingAgentMessage };
          delete pendingAgentMessage[message.agentId];
          next = { ...cur, transcripts: { ...cur.transcripts, [message.agentId]: transcript }, pendingAgentMessage };
          break;
        }
        case "tool_call":
        case "tool_call_update": {
          const transcript = [...(cur.transcripts[message.agentId] ?? []), { role: "system" as const, body: `${message.type === "tool_call" ? "Tool" : "Tool update"}: ${message.update?.title ?? message.update?.toolCallId ?? "tool"}` }];
          next = { ...cur, transcripts: { ...cur.transcripts, [message.agentId]: transcript } };
          break;
        }
        case "permission_request":
          next = { ...cur, pendingPermission: message };
          break;
        case "permission_expired":
          next = { ...cur, pendingPermission: cur.pendingPermission?.requestId === message.requestId ? null : cur.pendingPermission };
          break;
        case "error":
        case "agent_error":
          next = {
            ...cur,
            transcripts: message.agentId
              ? { ...cur.transcripts, [message.agentId]: [...(cur.transcripts[message.agentId] ?? []), { role: "system" as const, body: `Error: ${message.message}` }] }
              : cur.transcripts,
          };
          break;
        default:
          next = cur;
          break;
      }

      if (message.id != null) {
        next = { ...next, pollCursor: Math.max(next.pollCursor, Number(message.id)) };
      }

      if (!replay) {
        const summary = summarizeEvent(message);
        if (summary) next = { ...next, activity: addActivity(next.activity, summary) };
      }

      return next;
    });
  }, []);

  // ── Transport ────────────────────────────────────────────────────

  const startPolling = useCallback(async (resetCursor = false) => {
    const existingWs = wsRef.current;
    wsRef.current = null;
    existingWs?.close();
    pollAbortRef.current?.abort();
    const controller = new AbortController();
    pollAbortRef.current = controller;
    let cursor = resetCursor ? 0 : appRef.current.pollCursor;

    setApp((cur) => ({ ...cur, transportMode: "polling", pollCursor: cursor }));

    while (!controller.signal.aborted) {
      try {
        const body = await fetchJson(`/api/events?since=${encodeURIComponent(String(cursor))}&timeoutMs=25000`, { signal: controller.signal });
        setApp((cur) => ({ ...cur, secureTransport: Boolean(body.secureTransport) }));
        for (const event of body.events ?? []) applyEvent(event, false);
        cursor = Number.isFinite(body.cursor) ? body.cursor : cursor;
      } catch (error: any) {
        if (controller.signal.aborted) return;
        setApp((cur) => ({ ...cur, activity: addActivity(cur.activity, `Polling error: ${error.message}`) }));
        await new Promise((r) => setTimeout(r, 1500));
      }
    }
  }, [fetchJson, applyEvent]);

  const connectTransport = useCallback(async () => {
    if (typeof window !== "undefined" && new URLSearchParams(window.location.search).get("transport") === "poll") {
      await startPolling(false);
      return;
    }

    try {
      const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
      const query = csrfTokenRef.current ? `?csrf=${encodeURIComponent(csrfTokenRef.current)}` : "";
      const ws = new WebSocket(`${protocol}//${window.location.host}/ws${query}`);
      wsRef.current = ws;

      ws.addEventListener("open", () => { setApp((cur) => ({ ...cur, transportMode: "websocket" })); });
      ws.addEventListener("message", (event) => { applyEvent(JSON.parse(event.data), false); });
      ws.addEventListener("close", () => {
        if (wsRef.current !== ws) return;
        wsRef.current = null;
        if (appRef.current.authenticated) {
          setApp((cur) => ({ ...cur, activity: addActivity(cur.activity, "WebSocket disconnected. Falling back to polling.") }));
          void startPolling(false);
        }
      });
      ws.addEventListener("error", () => {
        if (wsRef.current === ws) wsRef.current = null;
        void startPolling(false);
      });
    } catch {
      await startPolling(false);
    }
  }, [applyEvent, startPolling]);

  // ── Bootstrap & Auth ─────────────────────────────────────────────

  const bootstrap = useCallback(async (signal?: AbortSignal) => {
    const body = await fetchJson("/api/bootstrap", { signal });
    if (signal?.aborted) return;
    csrfTokenRef.current = typeof body.csrfToken === "string" ? body.csrfToken : csrfTokenRef.current;
    setApp((cur) => ({
      ...initialState(),
      authenticated: Boolean(body.authenticated),
      secureTransport: Boolean(body.secureTransport),
      serverCwd: body.cwd ?? "",
      pairingPassword: body.pairingPassword ?? "",
      transportMode: cur.transportMode,
      agents: body.agents ?? [],
      runs: body.runs ?? [],
      skills: body.skills ?? [],
      share: body.share ?? null,
      activeAgentId: body.agents?.[0]?.id ?? null,
      activeRunId: body.runs?.[0]?.id ?? null,
      activeTaskId: body.runs?.[0]?.tasks?.[0]?.id ?? null,
    }));
    setCreateAgentForm((cur) => ({ ...cur, cwd: body.cwd ?? cur.cwd }));
    for (const event of body.events ?? []) applyEvent(event, true);
  }, [fetchJson, applyEvent]);

  const initializeSession = useCallback(async ({ signal }: { signal?: AbortSignal } = {}) => {
    const existingWs = wsRef.current;
    wsRef.current = null;
    existingWs?.close();
    pollAbortRef.current?.abort();
    setBooting(true);
    setBootError("");

    const body = await fetchJson("/api/me", { signal });
    if (signal?.aborted) return;
    csrfTokenRef.current = typeof body.csrfToken === "string" ? body.csrfToken : "";

    setApp((cur) => ({ ...cur, authenticated: Boolean(body.authenticated), secureTransport: Boolean(body.secureTransport), serverCwd: body.cwd ?? "" }));
    setCreateAgentForm((cur) => ({ ...cur, cwd: body.cwd ?? cur.cwd }));

    if (body.authenticated) {
      await bootstrap(signal);
      if (signal?.aborted) return;
      await connectTransport();
    }

    setBooting(false);
  }, [fetchJson, bootstrap, connectTransport]);

  const handleLogin = useCallback(async (password: string) => {
    setAuthError("");
    try {
      const body = await fetchJson("/api/login", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ password }) });
      csrfTokenRef.current = typeof body.csrfToken === "string" ? body.csrfToken : csrfTokenRef.current;
      setLoginPassword("");
      await initializeSession();
    } catch (error: any) {
      setAuthError(error.message);
    }
  }, [fetchJson, initializeSession]);

  const handleLogout = useCallback(async () => {
    wsRef.current?.close();
    wsRef.current = null;
    pollAbortRef.current?.abort();
    await fetchJson("/api/logout", { method: "POST" }).catch(() => {});
    csrfTokenRef.current = "";
    setApp(initialState());
    setBootError("");
    setBooting(false);
  }, [fetchJson]);

  // ── Send command ─────────────────────────────────────────────────

  const sendCommand = useCallback(async (payload: any) => {
    if (appRef.current.transportMode === "websocket" && wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify(payload));
      return null;
    }
    const response = await fetchJson("/api/command", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(payload) });
    if (response.response && response.response.type !== "ok") applyEvent(response.response, false);
    return response.response ?? null;
  }, [fetchJson, applyEvent]);

  // ── Init on mount ────────────────────────────────────────────────

  useEffect(() => {
    if (bootedRef.current) {
      return () => { wsRef.current?.close(); pollAbortRef.current?.abort(); };
    }
    bootedRef.current = true;
    const controller = new AbortController();
    void initializeSession({ signal: controller.signal }).catch((error) => {
      if (controller.signal.aborted || error?.name === "AbortError") return;
      setBooting(false);
      setBootError(error.message);
      setApp((cur) => ({ ...cur, activity: addActivity(cur.activity, `Bootstrap error: ${error.message}`) }));
    });
    return () => { controller.abort(); wsRef.current?.close(); pollAbortRef.current?.abort(); };
  }, [initializeSession]);

  // ── Derived values ───────────────────────────────────────────────

  const activeAgent = getActiveAgent(app);
  const activeRun = getActiveRun(app);
  const activeTask = getActiveTask(app);

  useEffect(() => {
    if (!activeAgent) { setAgentEditorForm(emptyAgentForm()); return; }
    setAgentEditorForm({ name: activeAgent.name ?? "", role: activeAgent.role ?? "worker", cwd: activeAgent.cwd ?? "", mode: activeAgent.mode ?? "", model: activeAgent.model ?? "", notes: activeAgent.notes ?? "", autoApprove: Boolean(activeAgent.autoApprove), skillIds: activeAgent.skillIds ?? [] });
  }, [activeAgent?.id]);

  useEffect(() => {
    if (!activeTask) { setTaskForm(emptyTaskForm()); return; }
    setTaskForm({ title: activeTask.title ?? "", description: activeTask.description ?? "", assignedAgentId: activeTask.assignedAgentId ?? "", modeId: activeTask.modeId ?? "", modelId: activeTask.modelId ?? "", skillIds: activeTask.skillIds ?? [] });
  }, [activeTask?.id]);

  const allModels = uniqueValues([
    ...COPILOT_MODEL_CHOICES,
    ...app.agents.flatMap((a) => [a.model, ...(a.availableModels ?? []).map((m) => extractChoiceValue(m))]),
  ]);
  const allModes = uniqueValues([
    ...ACP_MODE_CHOICES,
    ...app.agents.flatMap((a) => [a.mode, ...(a.availableModes ?? []).map((m) => extractChoiceValue(m))]),
  ]);
  const transcript = activeAgent ? app.transcripts[activeAgent.id] ?? [] : [];
  const readyTaskCount = app.runs.flatMap((r) => r.tasks).filter((t) => t.status === "ready").length;

  return {
    app, setApp,
    loginPassword, setLoginPassword,
    authError, bootError, booting, clientOrigin,
    inspectorTab, setInspectorTab,
    createAgentForm, setCreateAgentForm,
    agentEditorForm, setAgentEditorForm,
    createRunForm, setCreateRunForm,
    taskForm, setTaskForm,
    shareForm, setShareForm,
    sendCommand, handleLogin, handleLogout, initializeSession,
    activeAgent, activeRun, activeTask,
    allModels, allModes, transcript, readyTaskCount,
  };
}
