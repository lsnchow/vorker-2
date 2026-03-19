import { EventEmitter } from "node:events";
import { createSupervisorEvent } from "./events.js";
import { SupervisorStore } from "./store.js";

function toSession(agent = {}) {
  if (!agent?.id) {
    return null;
  }

  return {
    id: agent.id,
    name: agent.name ?? agent.id,
    role: agent.role ?? "worker",
    status: agent.status ?? "unknown",
    mode: agent.mode ?? null,
    model: agent.model ?? null,
    cwd: agent.cwd ?? "",
    notes: agent.notes ?? "",
    skillIds: Array.isArray(agent.skillIds) ? [...agent.skillIds] : [],
    autoApprove: agent.autoApprove ?? false,
    createdAt: agent.createdAt,
    updatedAt: agent.updatedAt,
  };
}

export class SupervisorService extends EventEmitter {
  constructor(options = {}) {
    super();
    this.manager = options.manager ?? null;
    this.orchestrator = options.orchestrator ?? null;
    this.tunnelManager = options.tunnelManager ?? null;
    this.skillCatalog = options.skillCatalog ?? null;
    this.eventLog = options.eventLog ?? null;
    this.store = options.store ?? new SupervisorStore();
    this.disposers = [];
  }

  async start() {
    this.#seedInitialState();

    if (this.manager) {
      const onAgentCreated = ({ agent }) => {
        const session = toSession(agent);
        if (session) {
          void this.record("session.registered", { session });
        }
      };

      const onAgentEvent = (event) => {
        void this.#handleAgentEvent(event);
      };

      this.manager.on("agent_created", onAgentCreated);
      this.manager.on("agent_event", onAgentEvent);
      this.disposers.push(() => this.manager.off("agent_created", onAgentCreated));
      this.disposers.push(() => this.manager.off("agent_event", onAgentEvent));
    }

    if (this.orchestrator) {
      const onOrchestratorEvent = (event) => {
        void this.#handleOrchestratorEvent(event);
      };

      this.orchestrator.on("event", onOrchestratorEvent);
      this.disposers.push(() => this.orchestrator.off("event", onOrchestratorEvent));
    }

    if (this.tunnelManager) {
      const onTunnelEvent = (event) => {
        if (event?.share) {
          void this.record("share.updated", { share: event.share });
        }
      };

      this.tunnelManager.on("event", onTunnelEvent);
      this.disposers.push(() => this.tunnelManager.off("event", onTunnelEvent));
    }
  }

  async close() {
    while (this.disposers.length > 0) {
      const dispose = this.disposers.pop();
      dispose?.();
    }
  }

  snapshot() {
    return this.store.snapshot();
  }

  async refreshSkills() {
    if (!this.skillCatalog?.listSkills) {
      return this.store.snapshot().skills;
    }

    const skills = this.skillCatalog.listSkills();
    await this.record("skills.updated", { skills });
    return skills;
  }

  async record(type, payload = {}, options = {}) {
    const event = createSupervisorEvent(type, payload, options);
    this.store.append(event);
    if (this.eventLog?.append) {
      await this.eventLog.append(event);
    }
    this.emit("event", event);
    return event;
  }

  #seedInitialState() {
    if (this.manager?.listAgents) {
      for (const agent of this.manager.listAgents()) {
        const session = toSession(agent);
        if (session) {
          this.store.append(createSupervisorEvent("session.registered", { session }));
        }
      }
    }

    if (this.orchestrator?.listRuns) {
      for (const run of this.orchestrator.listRuns()) {
        this.store.append(createSupervisorEvent("run.created", { run: { ...run, tasks: undefined } }));
        for (const task of run.tasks ?? []) {
          this.store.append(createSupervisorEvent("task.created", { task }));
        }
      }
    }

    if (this.skillCatalog?.listSkills) {
      this.store.append(createSupervisorEvent("skills.updated", { skills: this.skillCatalog.listSkills() }));
    }

    if (this.tunnelManager?.snapshot) {
      this.store.append(createSupervisorEvent("share.updated", { share: this.tunnelManager.snapshot() }));
    }
  }

  async #handleAgentEvent(event = {}) {
    const agent = event.agent ?? (event.agentId ? { id: event.agentId } : null);
    const session = toSession(agent ?? {});

    switch (event.type) {
      case "prompt_started":
        if (session) {
          await Promise.all([
            this.record("session.updated", { session }),
            this.record("session.prompt.started", {
              sessionId: session.id,
              message: {
                role: "user",
                text: String(event.text ?? "").trim(),
              },
            }),
          ]);
        }
        break;
      case "prompt_finished":
        if (session) {
          await Promise.all([
            this.record("session.updated", { session }),
            this.record("session.prompt.finished", {
              sessionId: session.id,
              message: {
                role: "assistant",
                text: String(event.responseText ?? "").trim(),
              },
            }),
          ]);
        }
        break;
      case "agent_state":
      case "initialized":
      case "mode_changed":
      case "model_changed":
      case "closed":
        if (session) {
          await this.record("session.updated", { session });
        }
        break;
      default:
        break;
    }
  }

  async #handleOrchestratorEvent(event = {}) {
    switch (event.type) {
      case "run_created":
      case "run_updated":
        if (event.run) {
          await this.record(event.type === "run_created" ? "run.created" : "run.updated", {
            run: { ...event.run, tasks: undefined },
          });
        }
        break;
      case "task_created":
      case "task_updated":
        if (event.task) {
          await this.record(event.type === "task_created" ? "task.created" : "task.updated", {
            task: event.task,
          });
        }
        break;
      default:
        break;
    }
  }
}
