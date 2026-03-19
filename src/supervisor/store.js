function cloneValue(value) {
  return value == null ? value : JSON.parse(JSON.stringify(value));
}

function sortByTimestampDesc(items, field = "updatedAt") {
  return [...items].sort((left, right) => {
    const leftValue = String(left?.[field] ?? left?.createdAt ?? "");
    const rightValue = String(right?.[field] ?? right?.createdAt ?? "");
    return rightValue.localeCompare(leftValue);
  });
}

function createSessionRecord(session = {}) {
  return {
    id: session.id ?? "",
    name: session.name ?? session.id ?? "session",
    role: session.role ?? "worker",
    status: session.status ?? "unknown",
    mode: session.mode ?? null,
    model: session.model ?? null,
    cwd: session.cwd ?? "",
    transcript: Array.isArray(session.transcript) ? session.transcript.map((entry) => ({ ...entry })) : [],
    createdAt: session.createdAt ?? new Date().toISOString(),
    updatedAt: session.updatedAt ?? new Date().toISOString(),
  };
}

function createRunRecord(run = {}) {
  const createdAt = run.createdAt ?? new Date().toISOString();
  return {
    id: run.id ?? "",
    name: run.name ?? "Untitled run",
    goal: run.goal ?? "",
    status: run.status ?? "draft",
    notes: run.notes ?? "",
    workerAgentIds: Array.isArray(run.workerAgentIds) ? [...run.workerAgentIds] : [],
    arbitratorAgentId: run.arbitratorAgentId ?? null,
    taskIds: Array.isArray(run.taskIds) ? [...run.taskIds] : [],
    createdAt,
    updatedAt: run.updatedAt ?? createdAt,
  };
}

function createTaskRecord(task = {}) {
  const createdAt = task.createdAt ?? new Date().toISOString();
  return {
    id: task.id ?? "",
    runId: task.runId ?? "",
    parentTaskId: task.parentTaskId ?? null,
    title: task.title ?? "Untitled task",
    description: task.description ?? "",
    status: task.status ?? "draft",
    assignedAgentId: task.assignedAgentId ?? null,
    templateAgentId: task.templateAgentId ?? null,
    executionAgentId: task.executionAgentId ?? null,
    workspacePath: task.workspacePath ?? null,
    branchName: task.branchName ?? null,
    baseBranch: task.baseBranch ?? null,
    commitSha: task.commitSha ?? null,
    changeCount: Number.isFinite(task.changeCount) ? Number(task.changeCount) : 0,
    changedFiles: Array.isArray(task.changedFiles) ? [...task.changedFiles] : [],
    mergeStatus: task.mergeStatus ?? null,
    mergeCommitSha: task.mergeCommitSha ?? null,
    mergeError: task.mergeError ?? null,
    mergedAt: task.mergedAt ?? null,
    outputText: task.outputText ?? "",
    error: task.error ?? null,
    createdAt,
    updatedAt: task.updatedAt ?? createdAt,
  };
}

export class SupervisorStore {
  constructor() {
    this.events = [];
    this.runs = new Map();
    this.tasks = new Map();
    this.sessions = new Map();
    this.skills = [];
    this.share = null;
  }

  append(event) {
    this.events.push(cloneValue(event));
    this.apply(event);
    return event;
  }

  apply(event) {
    switch (event.type) {
      case "run.created":
      case "run.updated":
        this.#applyRun(event.payload?.run);
        break;
      case "task.created":
      case "task.updated":
        this.#applyTask(event.payload?.task);
        break;
      case "session.registered":
      case "session.updated":
        this.#applySession(event.payload?.session);
        break;
      case "session.prompt.started":
      case "session.prompt.finished":
        this.#appendTranscript(event.payload?.sessionId, event.payload?.message);
        break;
      case "skills.updated":
        this.skills = Array.isArray(event.payload?.skills) ? cloneValue(event.payload.skills) : [];
        break;
      case "share.updated":
        this.share = cloneValue(event.payload?.share ?? null);
        break;
      default:
        break;
    }
  }

  snapshot() {
    const tasks = sortByTimestampDesc(Array.from(this.tasks.values()));
    const runs = sortByTimestampDesc(Array.from(this.runs.values())).map((run) => ({
      ...cloneValue(run),
      tasks: tasks.filter((task) => task.runId === run.id),
    }));
    const sessions = sortByTimestampDesc(Array.from(this.sessions.values()));

    return {
      runs,
      tasks,
      sessions,
      skills: cloneValue(this.skills),
      share: cloneValue(this.share),
      events: cloneValue(this.events),
    };
  }

  #applyRun(run) {
    if (!run?.id) {
      return;
    }

    const current = this.runs.get(run.id) ?? createRunRecord(run);
    this.runs.set(run.id, {
      ...current,
      ...createRunRecord(run),
      taskIds: current.taskIds ?? [],
    });
  }

  #applyTask(task) {
    if (!task?.id || !task?.runId) {
      return;
    }

    const current = this.tasks.get(task.id) ?? createTaskRecord(task);
    const next = {
      ...current,
      ...createTaskRecord(task),
    };
    this.tasks.set(task.id, next);

    const run = this.runs.get(task.runId) ?? createRunRecord({ id: task.runId });
    const taskIds = new Set(run.taskIds ?? []);
    taskIds.add(task.id);
    this.runs.set(task.runId, {
      ...run,
      taskIds: [...taskIds],
      updatedAt: next.updatedAt ?? run.updatedAt,
    });
  }

  #applySession(session) {
    if (!session?.id) {
      return;
    }

    const current = this.sessions.get(session.id) ?? createSessionRecord(session);
    this.sessions.set(session.id, {
      ...current,
      ...createSessionRecord(session),
      transcript: current.transcript ?? [],
    });
  }

  #appendTranscript(sessionId, message) {
    if (!sessionId || !message?.text) {
      return;
    }

    const current = this.sessions.get(sessionId) ?? createSessionRecord({ id: sessionId });
    this.sessions.set(sessionId, {
      ...current,
      transcript: [...(current.transcript ?? []), { role: message.role ?? "assistant", text: String(message.text) }],
      updatedAt: new Date().toISOString(),
    });
  }
}
