"use client";

import { Bot, ListTodo, Plus, RefreshCw, Play, Sparkles, X, Send } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";
import { Label } from "@/components/ui/label";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from "@/components/ui/select";
import { cn } from "@/lib/utils";
import {
  emptyAgentForm, emptyRunForm, emptyTaskForm,
  type Agent, type Run, type Task,
  type AgentForm, type RunForm, type TaskForm,
} from "@/hooks/use-vorker";

export function ReviewPanel({ vorker }: { vorker: any }) {
  const { app, inspectorTab, setInspectorTab } = vorker;

  const tabs = [
    { id: "tasks" as const, label: "Tasks", icon: ListTodo },
    { id: "agent" as const, label: "Agent", icon: Bot },
    { id: "create" as const, label: "Create", icon: Plus },
  ];

  return (
    <div className="flex h-full w-80 flex-col border-l border-border bg-card">
      {/* Tab Bar */}
      <div className="flex items-center border-b border-border">
        {tabs.map((t) => (
          <button
            key={t.id}
            onClick={() => setInspectorTab(t.id)}
            className={cn(
              "flex flex-1 items-center justify-center gap-1.5 py-2.5 text-xs font-medium transition-colors",
              inspectorTab === t.id
                ? "border-b-2 border-primary text-primary"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            <t.icon className="h-3.5 w-3.5" />
            {t.label}
          </button>
        ))}
      </div>

      <ScrollArea className="flex-1">
        <div className="p-3">
          {inspectorTab === "tasks" && <TasksTab vorker={vorker} />}
          {inspectorTab === "agent" && <AgentTab vorker={vorker} />}
          {inspectorTab === "create" && <CreateTab vorker={vorker} />}
        </div>
      </ScrollArea>
    </div>
  );
}

// ── Tasks Tab ───────────────────────────────────────────────────────

function TasksTab({ vorker }: { vorker: any }) {
  const { app, setApp, sendCommand, activeRun, activeTask, taskForm, setTaskForm } = vorker;

  if (!activeRun) {
    return (
      <EmptyState>Create or select a run to edit tasks and dispatch work.</EmptyState>
    );
  }

  return (
    <div className="space-y-3">
      {/* Run Info */}
      <div className="rounded-lg border border-border bg-background p-3">
        <div className="flex items-start justify-between gap-2">
          <div>
            <div className="text-sm font-semibold text-foreground">{activeRun.name}</div>
            <div className="mt-0.5 text-xs text-muted-foreground">{activeRun.goal}</div>
          </div>
          <Badge variant={activeRun.status === "running" ? "default" : "outline"} className="text-[10px]">
            {activeRun.status}
          </Badge>
        </div>
        <div className="mt-2.5 flex flex-wrap gap-1.5">
          <Button size="sm" variant="secondary" className="h-7 text-xs" onClick={() => void sendCommand({ type: "plan_run", runId: activeRun.id })}>
            <Sparkles className="mr-1 h-3 w-3" /> Plan
          </Button>
          <Button size="sm" className="h-7 text-xs" onClick={() => void sendCommand({ type: "auto_dispatch_run", runId: activeRun.id })}>
            <Play className="mr-1 h-3 w-3" /> Auto Dispatch
          </Button>
          <Button
            size="sm"
            variant="outline"
            className="h-7 text-xs"
            onClick={() => { setApp((c: any) => ({ ...c, activeTaskId: null })); setTaskForm(emptyTaskForm()); }}
          >
            <Plus className="mr-1 h-3 w-3" /> New Task
          </Button>
        </div>
      </div>

      {/* Task Queue */}
      <div className="rounded-lg border border-border bg-background p-3">
        <div className="mb-2 text-xs font-medium text-muted-foreground">Task Queue ({activeRun.tasks.length})</div>
        {activeRun.tasks.length === 0 ? (
          <div className="py-3 text-center text-xs text-muted-foreground">No tasks yet.</div>
        ) : (
          <div className="space-y-1">
            {activeRun.tasks.map((task: Task) => (
              <button
                key={task.id}
                type="button"
                onClick={() => setApp((c: any) => ({ ...c, activeTaskId: task.id }))}
                className={cn(
                  "w-full rounded-md border px-2.5 py-2 text-left transition-colors",
                  app.activeTaskId === task.id ? "border-primary/40 bg-primary/5" : "border-border hover:bg-accent",
                )}
              >
                <div className="flex items-start justify-between gap-2">
                  <span className="text-xs font-medium text-foreground">{task.title}</span>
                  <Badge variant={task.status === "ready" || task.status === "running" ? "default" : "outline"} className="text-[9px] shrink-0">
                    {task.status}
                  </Badge>
                </div>
                {task.description && <div className="mt-0.5 line-clamp-1 text-[10px] text-muted-foreground">{task.description}</div>}
                {(task.executionAgentId || task.branchName) && (
                  <div className="mt-1 space-y-0.5 text-[10px] text-muted-foreground">
                    {task.executionAgentId ? <div>exec: {task.executionAgentId}</div> : null}
                    {task.branchName ? <div>branch: {task.branchName}</div> : null}
                    {task.commitSha ? <div>commit: {task.commitSha.slice(0, 12)} ({task.changeCount ?? 0} files)</div> : null}
                  </div>
                )}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Task Form */}
      <form
        className="rounded-lg border border-border bg-background p-3 space-y-3"
        onSubmit={(e) => {
          e.preventDefault();
          if (activeTask) {
            void sendCommand({ type: "update_task", taskId: activeTask.id, ...taskForm });
          } else {
            void sendCommand({ type: "create_task", runId: activeRun.id, status: "ready", ...taskForm });
          }
        }}
      >
        <div className="text-xs font-medium text-foreground">{activeTask ? "Edit Task" : "Create Task"}</div>

        <FormField label="Title">
          <Input className="h-8 text-xs" value={taskForm.title} onChange={(e) => setTaskForm((c: TaskForm) => ({ ...c, title: e.target.value }))} />
        </FormField>

        <FormField label="Assigned Worker">
          <Select value={taskForm.assignedAgentId} onValueChange={(v) => setTaskForm((c: TaskForm) => ({ ...c, assignedAgentId: v }))}>
            <SelectTrigger className="h-8 text-xs"><SelectValue placeholder="Choose worker" /></SelectTrigger>
            <SelectContent>
              {app.agents.map((a: Agent) => <SelectItem key={a.id} value={a.id}>{a.name}</SelectItem>)}
            </SelectContent>
          </Select>
        </FormField>

        <div className="grid grid-cols-2 gap-2">
          <FormField label="Mode">
            <ModelModeSelect
              value={taskForm.modeId}
              options={vorker.allModes}
              placeholder="Mode"
              onChange={(v) => setTaskForm((c: TaskForm) => ({ ...c, modeId: v }))}
            />
          </FormField>
          <FormField label="Model">
            <ModelModeSelect
              value={taskForm.modelId}
              options={vorker.allModels}
              placeholder="Model"
              onChange={(v) => setTaskForm((c: TaskForm) => ({ ...c, modelId: v }))}
            />
          </FormField>
        </div>

        <FormField label="Description">
          <Textarea className="min-h-[60px] text-xs" value={taskForm.description} onChange={(e) => setTaskForm((c: TaskForm) => ({ ...c, description: e.target.value }))} />
        </FormField>

        <div className="flex gap-2">
          <Button type="submit" size="sm" className="flex-1 text-xs">{activeTask ? "Save" : "Create"}</Button>
          {activeTask && (
            <Button
              type="button"
              size="sm"
              variant="secondary"
              className="text-xs"
              onClick={() => {
                void sendCommand({
                  type: "dispatch_task",
                  taskId: activeTask.id,
                  agentId: taskForm.assignedAgentId || undefined,
                  modeId: taskForm.modeId || undefined,
                  modelId: taskForm.modelId || undefined,
                });
              }}
            >
              <Send className="mr-1 h-3 w-3" /> Dispatch
            </Button>
          )}
        </div>

        {activeTask && (activeTask.workspacePath || activeTask.branchName || activeTask.executionAgentId) ? (
          <div className="rounded-md border border-border bg-muted/20 p-2 text-[10px] text-muted-foreground">
            {activeTask.templateAgentId ? <div>template worker: {activeTask.templateAgentId}</div> : null}
            {activeTask.executionAgentId ? <div>execution agent: {activeTask.executionAgentId}</div> : null}
            {activeTask.branchName ? <div>branch: {activeTask.branchName}</div> : null}
            {activeTask.baseBranch ? <div>base: {activeTask.baseBranch}</div> : null}
            {activeTask.commitSha ? <div>commit: {activeTask.commitSha}</div> : null}
            {activeTask.changedFiles?.length ? <div>files: {activeTask.changedFiles.join(", ")}</div> : null}
            {activeTask.workspacePath ? <div className="break-all">workspace: {activeTask.workspacePath}</div> : null}
          </div>
        ) : null}
      </form>
    </div>
  );
}

// ── Agent Tab ───────────────────────────────────────────────────────

function AgentTab({ vorker }: { vorker: any }) {
  const { app, sendCommand, activeAgent, agentEditorForm, setAgentEditorForm } = vorker;

  if (!activeAgent) {
    return <EmptyState>Select an agent to edit its profile.</EmptyState>;
  }

  return (
    <form
      className="space-y-3"
      onSubmit={(e) => {
        e.preventDefault();
        void sendCommand({ type: "update_agent", agentId: activeAgent.id, ...agentEditorForm });
      }}
    >
      {/* Agent header */}
      <div className="rounded-lg border border-border bg-background p-3">
        <div className="flex items-start justify-between gap-2">
          <div>
            <div className="text-sm font-semibold text-foreground">{activeAgent.name}</div>
            <div className="mt-0.5 text-xs text-muted-foreground">{activeAgent.role || "worker"} &bull; {activeAgent.status}</div>
          </div>
          <Badge variant={activeAgent.status === "ready" || activeAgent.status === "running" ? "default" : "outline"} className="text-[10px]">
            {activeAgent.status}
          </Badge>
        </div>
      </div>

      {/* Edit form */}
      <div className="rounded-lg border border-border bg-background p-3 space-y-3">
        <FormField label="Name">
          <Input className="h-8 text-xs" value={agentEditorForm.name} onChange={(e) => setAgentEditorForm((c: AgentForm) => ({ ...c, name: e.target.value }))} />
        </FormField>

        <FormField label="Role">
          <Input className="h-8 text-xs" value={agentEditorForm.role} onChange={(e) => setAgentEditorForm((c: AgentForm) => ({ ...c, role: e.target.value }))} />
        </FormField>

        <div className="grid grid-cols-2 gap-2">
          <FormField label="Mode">
            <ModelModeSelect
              value={agentEditorForm.mode}
              options={vorker.allModes}
              placeholder="Mode"
              onChange={(v) => setAgentEditorForm((c: AgentForm) => ({ ...c, mode: v }))}
            />
          </FormField>
          <FormField label="Model">
            <ModelModeSelect
              value={agentEditorForm.model}
              options={vorker.allModels}
              placeholder="Model"
              onChange={(v) => setAgentEditorForm((c: AgentForm) => ({ ...c, model: v }))}
            />
          </FormField>
        </div>

        <FormField label="System Prompt">
          <Textarea className="min-h-[60px] text-xs" value={agentEditorForm.notes} onChange={(e) => setAgentEditorForm((c: AgentForm) => ({ ...c, notes: e.target.value }))} />
        </FormField>

        <div className="flex items-center gap-2">
          <Switch
            id="autoApprove"
            checked={agentEditorForm.autoApprove}
            onCheckedChange={(v) => setAgentEditorForm((c: AgentForm) => ({ ...c, autoApprove: v }))}
          />
          <Label htmlFor="autoApprove" className="text-xs">Auto-approve tool requests</Label>
        </div>

        <div className="flex gap-2">
          <Button type="submit" size="sm" className="flex-1 text-xs">Save Agent</Button>
          <Button
            type="button"
            size="sm"
            variant="destructive"
            className="text-xs"
            onClick={() => void sendCommand({ type: "close_agent", agentId: activeAgent.id })}
          >
            <X className="mr-1 h-3 w-3" /> Close
          </Button>
        </div>
      </div>
    </form>
  );
}

// ── Create Tab ──────────────────────────────────────────────────────

function CreateTab({ vorker }: { vorker: any }) {
  const {
    app, sendCommand, setInspectorTab,
    createRunForm, setCreateRunForm,
    createAgentForm, setCreateAgentForm,
  } = vorker;

  return (
    <div className="space-y-3">
      {/* Create Run */}
      <form
        className="rounded-lg border border-border bg-background p-3 space-y-3"
        onSubmit={(e) => {
          e.preventDefault();
          void sendCommand({ type: "create_run", ...createRunForm, workspace: app.serverCwd }).then(() => {
            setCreateRunForm(emptyRunForm());
            setInspectorTab("tasks");
          });
        }}
      >
        <div className="text-xs font-medium text-foreground">Create Run</div>

        <FormField label="Name">
          <Input className="h-8 text-xs" value={createRunForm.name} onChange={(e) => setCreateRunForm((c: RunForm) => ({ ...c, name: e.target.value }))} />
        </FormField>

        <FormField label="Goal">
          <Textarea className="min-h-[60px] text-xs" value={createRunForm.goal} onChange={(e) => setCreateRunForm((c: RunForm) => ({ ...c, goal: e.target.value }))} />
        </FormField>

        <FormField label="Arbitrator Agent">
          <Select value={createRunForm.arbitratorAgentId} onValueChange={(v) => setCreateRunForm((c: RunForm) => ({ ...c, arbitratorAgentId: v }))}>
            <SelectTrigger className="h-8 text-xs"><SelectValue placeholder="Choose agent" /></SelectTrigger>
            <SelectContent>
              {app.agents.map((a: Agent) => <SelectItem key={a.id} value={a.id}>{a.name}</SelectItem>)}
            </SelectContent>
          </Select>
        </FormField>

        <div className="space-y-1.5">
          <label className="text-[10px] uppercase tracking-widest text-muted-foreground">Worker Agents</label>
          {app.agents.map((agent: Agent) => (
            <label key={agent.id} className="flex items-center gap-2 rounded-md border border-border px-2.5 py-2 text-xs">
              <Checkbox
                checked={createRunForm.workerAgentIds.includes(agent.id)}
                onCheckedChange={(checked) => {
                  setCreateRunForm((c: RunForm) => ({
                    ...c,
                    workerAgentIds: checked ? [...c.workerAgentIds, agent.id] : c.workerAgentIds.filter((id) => id !== agent.id),
                  }));
                }}
              />
              <span className="text-foreground">{agent.name}</span>
              <span className="text-muted-foreground">{agent.role || "worker"}</span>
            </label>
          ))}
        </div>

        <Button type="submit" size="sm" className="w-full text-xs">Create Run</Button>
      </form>

      <Separator />

      {/* Create Agent */}
      <form
        className="rounded-lg border border-border bg-background p-3 space-y-3"
        onSubmit={(e) => {
          e.preventDefault();
          void sendCommand({ type: "create_agent", ...createAgentForm }).then(() => {
            setCreateAgentForm((c: AgentForm) => ({ ...emptyAgentForm(), cwd: c.cwd }));
            setInspectorTab("agent");
          });
        }}
      >
        <div className="text-xs font-medium text-foreground">Create Agent</div>

        <div className="grid grid-cols-2 gap-2">
          <FormField label="Name">
            <Input className="h-8 text-xs" value={createAgentForm.name} onChange={(e) => setCreateAgentForm((c: AgentForm) => ({ ...c, name: e.target.value }))} />
          </FormField>
          <FormField label="Role">
            <Input className="h-8 text-xs" value={createAgentForm.role} onChange={(e) => setCreateAgentForm((c: AgentForm) => ({ ...c, role: e.target.value }))} />
          </FormField>
        </div>

        <FormField label="Workspace">
          <Input className="h-8 text-xs" value={createAgentForm.cwd} onChange={(e) => setCreateAgentForm((c: AgentForm) => ({ ...c, cwd: e.target.value }))} />
        </FormField>

        <div className="grid grid-cols-2 gap-2">
          <FormField label="Mode">
            <ModelModeSelect
              value={createAgentForm.mode}
              options={vorker.allModes}
              placeholder="Mode"
              onChange={(v) => setCreateAgentForm((c: AgentForm) => ({ ...c, mode: v }))}
            />
          </FormField>
          <FormField label="Model">
            <ModelModeSelect
              value={createAgentForm.model}
              options={vorker.allModels}
              placeholder="Model"
              onChange={(v) => setCreateAgentForm((c: AgentForm) => ({ ...c, model: v }))}
            />
          </FormField>
        </div>

        <FormField label="System Prompt">
          <Textarea className="min-h-[60px] text-xs" value={createAgentForm.notes} onChange={(e) => setCreateAgentForm((c: AgentForm) => ({ ...c, notes: e.target.value }))} />
        </FormField>

        <div className="flex items-center gap-2">
          <Switch
            id="createAutoApprove"
            checked={createAgentForm.autoApprove}
            onCheckedChange={(v) => setCreateAgentForm((c: AgentForm) => ({ ...c, autoApprove: v }))}
          />
          <Label htmlFor="createAutoApprove" className="text-xs">Auto-approve tool requests</Label>
        </div>

        <Button type="submit" size="sm" className="w-full text-xs">
          <Plus className="mr-1 h-3 w-3" /> Create Agent
        </Button>
      </form>
    </div>
  );
}

// ── Shared components ───────────────────────────────────────────────

function FormField({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="space-y-1">
      <label className="text-[10px] uppercase tracking-widest text-muted-foreground">{label}</label>
      {children}
    </div>
  );
}

function ModelModeSelect({ value, options, placeholder, onChange }: {
  value: string; options: string[]; placeholder: string; onChange: (v: string) => void;
}) {
  return (
    <Select value={value || undefined} onValueChange={onChange}>
      <SelectTrigger className="h-8 text-xs">
        <SelectValue placeholder={placeholder} />
      </SelectTrigger>
      <SelectContent>
        {options.map((opt) => (
          <SelectItem key={opt} value={opt}>{formatChoiceLabel(opt)}</SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

function EmptyState({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-center rounded-lg border border-dashed border-border bg-background p-6">
      <p className="text-center text-xs text-muted-foreground">{children}</p>
    </div>
  );
}

function formatChoiceLabel(value: string) {
  if (value.startsWith("https://agentclientprotocol.com/protocol/session-modes#")) {
    const fragment = value.split("#")[1] ?? value;
    return fragment.charAt(0).toUpperCase() + fragment.slice(1);
  }
  return value;
}
