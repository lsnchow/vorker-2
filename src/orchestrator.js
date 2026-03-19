import { EventEmitter } from "node:events";
import { randomUUID } from "node:crypto";
import process from "node:process";
import { TaskWorkspaceManager } from "./git/task-workspace.js";

const RUN_STATUSES = ["draft", "planning", "ready", "running", "completed", "failed"];
const TASK_STATUSES = ["draft", "ready", "running", "completed", "failed"];

function nowIso() {
  return new Date().toISOString();
}

function sortByUpdatedAtDesc(items) {
  return [...items].sort((left, right) => right.updatedAt.localeCompare(left.updatedAt));
}

function normalizeString(value) {
  if (typeof value !== "string") {
    return "";
  }
  return value.trim();
}

function normalizeStringArray(values) {
  const result = [];
  const seen = new Set();

  for (const value of values ?? []) {
    const normalized = normalizeString(value);
    if (!normalized || seen.has(normalized)) {
      continue;
    }
    seen.add(normalized);
    result.push(normalized);
  }

  return result;
}

function mergeUniqueStrings(...groups) {
  const result = [];
  const seen = new Set();

  for (const group of groups) {
    for (const value of group ?? []) {
      const normalized = normalizeString(value);
      if (!normalized || seen.has(normalized)) {
        continue;
      }
      seen.add(normalized);
      result.push(normalized);
    }
  }

  return result;
}

function maybeJson(text) {
  const trimmed = normalizeString(text);
  if (!trimmed) {
    return null;
  }

  const fenced = trimmed.match(/```(?:json)?\s*([\s\S]*?)```/i);
  const candidate = fenced?.[1]?.trim() ?? trimmed;
  return candidate;
}

function parsePlanResponse(text) {
  const candidate = maybeJson(text);
  if (!candidate) {
    return [];
  }

  const parsed = JSON.parse(candidate);
  const tasks = Array.isArray(parsed) ? parsed : Array.isArray(parsed.tasks) ? parsed.tasks : [];
  return tasks
    .map((task, index) => {
      if (!task || typeof task !== "object") {
        return null;
      }
      const title = normalizeString(task.title || task.name || `Task ${index + 1}`);
      const description = normalizeString(task.description || task.prompt || task.instructions);
      if (!title && !description) {
        return null;
      }
      return {
        title: title || `Task ${index + 1}`,
        description: description || title,
        skillIds: normalizeStringArray(task.skillIds),
        recommendedAgentId: normalizeString(task.recommendedAgentId) || null,
        modeId: normalizeString(task.modeId) || null,
        modelId: normalizeString(task.modelId) || null,
      };
    })
    .filter(Boolean);
}

export class Orchestrator extends EventEmitter {
  constructor(options = {}) {
    super();
    this.manager = options.manager;
    this.runs = new Map();
    this.tasks = new Map();
    const repoRoot = options.repoRoot ?? this.manager?.defaults?.cwd ?? process.cwd();
    this.workspaceManager = options.workspaceManager ?? new TaskWorkspaceManager({ repoRoot });
  }

  publish(type, payload = {}) {
    const event = {
      type,
      ...payload,
    };
    this.emit("event", event);
    return event;
  }

  snapshotTask(taskId) {
    const task = this.tasks.get(taskId);
    if (!task) {
      return null;
    }

    return {
      ...task,
    };
  }

  snapshotRun(runId) {
    const run = this.runs.get(runId);
    if (!run) {
      return null;
    }

    return {
      ...run,
      tasks: sortByUpdatedAtDesc(run.taskIds.map((taskId) => this.snapshotTask(taskId)).filter(Boolean)),
    };
  }

  listRuns() {
    return sortByUpdatedAtDesc(Array.from(this.runs.keys()).map((runId) => this.snapshotRun(runId)).filter(Boolean));
  }

  getRun(runId) {
    return this.runs.get(runId) ?? null;
  }

  requireRun(runId) {
    const run = this.getRun(runId);
    if (!run) {
      throw new Error(`Unknown run: ${runId}`);
    }
    return run;
  }

  getTask(taskId) {
    return this.tasks.get(taskId) ?? null;
  }

  requireTask(taskId) {
    const task = this.getTask(taskId);
    if (!task) {
      throw new Error(`Unknown task: ${taskId}`);
    }
    return task;
  }

  setRun(runId, updates) {
    const current = this.requireRun(runId);
    const next = {
      ...current,
      ...updates,
      updatedAt: nowIso(),
    };
    this.runs.set(runId, next);
    this.publish("run_updated", { run: this.snapshotRun(runId) });
    return next;
  }

  setTask(taskId, updates) {
    const current = this.requireTask(taskId);
    const next = {
      ...current,
      ...updates,
      updatedAt: nowIso(),
    };
    this.tasks.set(taskId, next);
    this.publish("task_updated", { task: this.snapshotTask(taskId), run: this.snapshotRun(next.runId) });
    return next;
  }

  createRun(input = {}) {
    const runId = randomUUID();
    const run = createRunRecord({ ...input, id: runId }, []);

    this.runs.set(runId, run);
    this.publish("run_created", { run: this.snapshotRun(runId) });
    return this.snapshotRun(runId);
  }

  createTask(input = {}) {
    const run = this.requireRun(normalizeString(input.runId));
    const taskId = randomUUID();
    const task = createTaskRecord({ ...input, id: taskId, runId: run.id }, run.id);

    this.tasks.set(taskId, task);
    this.runs.set(run.id, {
      ...run,
      taskIds: [...run.taskIds, taskId],
      updatedAt: nowIso(),
    });
    this.publish("task_created", { task: this.snapshotTask(taskId), run: this.snapshotRun(run.id) });
    return this.snapshotTask(taskId);
  }

  updateRun(runId, updates = {}) {
    const next = this.setRun(runId, {
      name: normalizeString(updates.name) || this.requireRun(runId).name,
      goal: normalizeString(updates.goal) || this.requireRun(runId).goal,
      notes: typeof updates.notes === "string" ? normalizeString(updates.notes) : this.requireRun(runId).notes,
      arbitratorAgentId:
        "arbitratorAgentId" in updates
          ? normalizeString(updates.arbitratorAgentId) || null
          : this.requireRun(runId).arbitratorAgentId,
      workerAgentIds:
        Array.isArray(updates.workerAgentIds) ? normalizeStringArray(updates.workerAgentIds) : this.requireRun(runId).workerAgentIds,
      workspace: normalizeString(updates.workspace) || this.requireRun(runId).workspace,
      status: RUN_STATUSES.includes(updates.status) ? updates.status : this.requireRun(runId).status,
    });
    return this.snapshotRun(next.id);
  }

  updateTask(taskId, updates = {}) {
    const current = this.requireTask(taskId);
    const next = this.setTask(taskId, {
      title: normalizeString(updates.title) || current.title,
      description: normalizeString(updates.description) || current.description,
      status: TASK_STATUSES.includes(updates.status) ? updates.status : current.status,
      assignedAgentId:
        "assignedAgentId" in updates ? normalizeString(updates.assignedAgentId) || null : current.assignedAgentId,
      skillIds: Array.isArray(updates.skillIds) ? normalizeStringArray(updates.skillIds) : current.skillIds,
      modeId: "modeId" in updates ? normalizeString(updates.modeId) || null : current.modeId,
      modelId: "modelId" in updates ? normalizeString(updates.modelId) || null : current.modelId,
      templateAgentId:
        "templateAgentId" in updates ? normalizeString(updates.templateAgentId) || null : current.templateAgentId,
      executionAgentId:
        "executionAgentId" in updates ? normalizeString(updates.executionAgentId) || null : current.executionAgentId,
      workspacePath: "workspacePath" in updates ? normalizeString(updates.workspacePath) || null : current.workspacePath,
      branchName: "branchName" in updates ? normalizeString(updates.branchName) || null : current.branchName,
      baseBranch: "baseBranch" in updates ? normalizeString(updates.baseBranch) || null : current.baseBranch,
      commitSha: "commitSha" in updates ? normalizeString(updates.commitSha) || null : current.commitSha,
      changeCount: "changeCount" in updates && Number.isFinite(updates.changeCount) ? Number(updates.changeCount) : current.changeCount,
      changedFiles: Array.isArray(updates.changedFiles) ? normalizeStringArray(updates.changedFiles) : current.changedFiles,
      error: "error" in updates ? normalizeString(updates.error) || null : current.error,
      outputText: "outputText" in updates ? String(updates.outputText ?? "") : current.outputText,
    });
    return this.snapshotTask(next.id);
  }

  async planRun(runId) {
    const run = this.requireRun(runId);
    if (!run.arbitratorAgentId) {
      throw new Error("Select an arbitrator agent before planning.");
    }

    this.setRun(runId, { status: "planning" });

    const skills = this.manager.skillCatalog?.listSkills() ?? [];
    const agents = this.manager.listAgents();
    const planningPrompt = [
      "You are the arbitrator for a multi-agent coding control plane.",
      "Only decompose when the work is genuinely parallelizable. Sequential reasoning chains should stay as one task or a very small number of ordered tasks.",
      "Prefer a hierarchical plan: one arbitrator, a few concrete worker tasks, and a final validation-oriented task only when needed.",
      "Minimize coordination overhead and avoid redundant workers solving the same path.",
      "Break the goal into 2-8 concrete worker tasks when decomposition is justified.",
      "Return JSON only in this shape:",
      '{"tasks":[{"title":"", "description":"", "recommendedAgentId":"optional", "skillIds":["optional absolute skill ids"], "modeId":"optional", "modelId":"optional"}]}',
      "",
      `Run name: ${run.name}`,
      `Goal: ${run.goal}`,
      run.notes ? `Notes: ${run.notes}` : null,
      `Available worker ids: ${run.workerAgentIds.join(", ") || "(none selected)"}`,
      `Available agents: ${agents.map((agent) => `${agent.id}:${agent.name}`).join(", ") || "(none)"}`,
      skills.length > 0
        ? `Available skills:\n${skills.map((skill) => `- ${skill.id} :: ${skill.name} :: ${skill.description}`).join("\n")}`
        : "Available skills: none discovered.",
    ]
      .filter(Boolean)
      .join("\n");

    const result = await this.manager.promptAgent(run.arbitratorAgentId, planningPrompt, {
      displayText: `Plan run: ${run.name}`,
      contextSections: [
        "Plan like a strong root planner: only emit tasks you can confidently specify right now, and avoid fake parallelism.",
        "Return a valid JSON object with a tasks array. Do not wrap it in prose unless you must use a fenced json block.",
      ],
    });

    const planText = String(result.responseText ?? "").trim();
    let tasks = [];

    try {
      tasks = parsePlanResponse(planText);
    } catch (error) {
      tasks = [];
      this.publish("run_plan_error", {
        runId,
        message: error.message,
        rawText: planText,
      });
    }

    if (tasks.length === 0) {
      tasks = [
        {
          title: run.goal.slice(0, 80) || "Fallback task",
          description: run.goal || "No goal provided.",
          skillIds: [],
          recommendedAgentId: run.workerAgentIds[0] ?? null,
          modeId: null,
          modelId: null,
        },
      ];
    }

    for (const task of tasks) {
      this.createTask({
        runId,
        title: task.title,
        description: task.description,
        status: "ready",
        assignedAgentId: task.recommendedAgentId || null,
        skillIds: task.skillIds,
        modeId: task.modeId,
        modelId: task.modelId,
      });
    }

    this.setRun(runId, {
      status: "ready",
      lastPlanText: planText,
    });

    return this.snapshotRun(runId);
  }

  chooseWorkerForTask(task, run, offset = 0) {
    if (task.templateAgentId) {
      return task.templateAgentId;
    }

    if (task.assignedAgentId) {
      return task.assignedAgentId;
    }

    if (!run.workerAgentIds.length) {
      return null;
    }

    return run.workerAgentIds[offset % run.workerAgentIds.length];
  }

  findAgentProfile(agentId) {
    if (!agentId) {
      return null;
    }

    const direct = this.manager?.getAgent?.(agentId);
    if (direct?.snapshot) {
      return direct.snapshot();
    }

    return this.manager?.listAgents?.().find((agent) => agent.id === agentId) ?? null;
  }

  async dispatchTask(taskId, options = {}) {
    const task = this.requireTask(taskId);
    const run = this.requireRun(task.runId);
    const templateAgentId = normalizeString(options.agentId) || this.chooseWorkerForTask(task, run, 0);
    if (!templateAgentId) {
      throw new Error("No worker agent selected for this task.");
    }

    const modeId = normalizeString(options.modeId) || task.modeId;
    const modelId = normalizeString(options.modelId) || task.modelId;
    const workspace = await this.workspaceManager.ensureTaskWorkspace({
      runId: run.id,
      taskId: task.id,
      title: task.title,
    });
    const templateAgent = this.findAgentProfile(templateAgentId);
    const executionAgent = await this.manager.createAgent({
      name: `${templateAgent?.name ?? "Task Worker"} :: ${task.title}`,
      cwd: workspace.workspacePath,
      role: templateAgent?.role ?? "worker",
      notes: templateAgent?.notes ?? "",
      skillIds: mergeUniqueStrings(templateAgent?.skillIds, task.skillIds),
      autoApprove: typeof templateAgent?.autoApprove === "boolean" ? templateAgent.autoApprove : false,
      mode: modeId || templateAgent?.mode || null,
      model: modelId || templateAgent?.model || null,
    });
    const executionAgentId = executionAgent?.id ?? executionAgent?.snapshot?.().id;

    this.setTask(taskId, {
      status: "running",
      assignedAgentId: templateAgentId,
      templateAgentId,
      executionAgentId,
      workspacePath: workspace.workspacePath,
      branchName: workspace.branchName,
      baseBranch: workspace.baseBranch,
      error: null,
      lastDispatchAt: nowIso(),
    });
    this.setRun(run.id, { status: "running" });

    try {
      const response = await this.manager.promptAgent(executionAgentId, task.description, {
        displayText: `Task: ${task.title}`,
        skillIds: task.skillIds,
        contextSections: [
          `Run: ${run.name}`,
          `Goal: ${run.goal}`,
          `Task title: ${task.title}`,
          `Workspace: ${workspace.workspacePath}`,
          `Branch: ${workspace.branchName}`,
          `Base branch: ${workspace.baseBranch}`,
          "Work inside the assigned repository and report what you changed, any blockers, and the next recommended step.",
        ],
      });
      const commitSummary = await this.workspaceManager.commitTaskWorkspace({
        workspacePath: workspace.workspacePath,
        taskId: task.id,
        title: task.title,
      });

      this.setTask(taskId, {
        status: "completed",
        commitSha: commitSummary.commitSha,
        changeCount: commitSummary.changedFiles.length,
        changedFiles: commitSummary.changedFiles,
        outputText: String(response.responseText ?? "").trim(),
        error: null,
      });
    } catch (error) {
      this.setTask(taskId, {
        status: "failed",
        error: error instanceof Error ? error.message : String(error),
      });
      this.setRun(run.id, { status: "failed" });
      throw error;
    }

    this.recomputeRunStatus(run.id);

    return this.snapshotTask(taskId);
  }

  async autoDispatchReadyTasks(runId) {
    const run = this.requireRun(runId);
    const readyTasks = run.taskIds
      .map((taskId) => this.requireTask(taskId))
      .filter((task) => task.status === "ready" || task.status === "draft");

    const results = await Promise.allSettled(
      readyTasks.map(async (task, index) => {
        const agentId = this.chooseWorkerForTask(task, run, index);
        return await this.dispatchTask(task.id, {
          agentId,
          modeId: task.modeId,
          modelId: task.modelId,
        });
      }),
    );
    return results;
  }

  snapshot() {
    return {
      runs: this.listRuns(),
    };
  }

  hydrate(runs = []) {
    this.runs.clear();
    this.tasks.clear();

    for (const run of runs ?? []) {
      const taskIds = [];
      for (const task of run.tasks ?? []) {
        const createdTask = createTaskRecord(task, run.id);
        this.tasks.set(createdTask.id, createdTask);
        taskIds.push(createdTask.id);
      }

      const createdRun = createRunRecord(run, taskIds);
      this.runs.set(createdRun.id, createdRun);
    }

    return this.snapshot();
  }

  recomputeRunStatus(runId) {
    const run = this.requireRun(runId);
    const tasks = run.taskIds.map((candidateTaskId) => this.requireTask(candidateTaskId));

    if (tasks.some((candidate) => candidate.status === "failed")) {
      this.setRun(run.id, { status: "failed" });
      return "failed";
    }

    if (tasks.length > 0 && tasks.every((candidate) => candidate.status === "completed")) {
      this.setRun(run.id, { status: "completed" });
      return "completed";
    }

    if (tasks.some((candidate) => candidate.status === "running")) {
      this.setRun(run.id, { status: "running" });
      return "running";
    }

    if (tasks.some((candidate) => candidate.status === "ready")) {
      this.setRun(run.id, { status: "ready" });
      return "ready";
    }

    this.setRun(run.id, { status: "draft" });
    return "draft";
  }
}

function createRunRecord(run, taskIds = []) {
  const createdAt = normalizeString(run.createdAt) || nowIso();
  return {
    id: normalizeString(run.id) || randomUUID(),
    name: normalizeString(run.name) || "New run",
    goal: normalizeString(run.goal),
    workspace: normalizeString(run.workspace),
    arbitratorAgentId: normalizeString(run.arbitratorAgentId) || null,
    workerAgentIds: normalizeStringArray(run.workerAgentIds),
    status: RUN_STATUSES.includes(run.status) ? run.status : "draft",
    notes: normalizeString(run.notes),
    taskIds,
    createdAt,
    updatedAt: normalizeString(run.updatedAt) || createdAt,
    lastPlanText: typeof run.lastPlanText === "string" ? run.lastPlanText : "",
  };
}

function createTaskRecord(task, runId) {
  const createdAt = normalizeString(task.createdAt) || nowIso();
  return {
    id: normalizeString(task.id) || randomUUID(),
    runId: normalizeString(task.runId) || runId,
    title: normalizeString(task.title) || "Untitled task",
    description: normalizeString(task.description) || normalizeString(task.title) || "No description",
    status: TASK_STATUSES.includes(task.status) ? task.status : "draft",
    assignedAgentId: normalizeString(task.assignedAgentId) || null,
    skillIds: normalizeStringArray(task.skillIds),
    modeId: normalizeString(task.modeId) || null,
    modelId: normalizeString(task.modelId) || null,
    templateAgentId: normalizeString(task.templateAgentId) || null,
    executionAgentId: normalizeString(task.executionAgentId) || null,
    workspacePath: normalizeString(task.workspacePath) || null,
    branchName: normalizeString(task.branchName) || null,
    baseBranch: normalizeString(task.baseBranch) || null,
    commitSha: normalizeString(task.commitSha) || null,
    changeCount: Number.isFinite(task.changeCount) ? Number(task.changeCount) : 0,
    changedFiles: normalizeStringArray(task.changedFiles),
    lastDispatchAt: normalizeString(task.lastDispatchAt) || null,
    outputText: typeof task.outputText === "string" ? task.outputText : "",
    error: normalizeString(task.error) || null,
    createdAt,
    updatedAt: normalizeString(task.updatedAt) || createdAt,
    parentTaskId: normalizeString(task.parentTaskId) || null,
  };
}
