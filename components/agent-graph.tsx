"use client";

import { useMemo, useCallback } from "react";
import {
  Background, Controls, MarkerType, MiniMap, ReactFlow,
  type Node, type Edge, type NodeMouseHandler,
} from "@xyflow/react";
import dagre from "dagre";
import { cn } from "@/lib/utils";
import type { Agent, Run, ShareState } from "@/hooks/use-vorker";

// ── Status colors ──────────────────────────────────────────────────

function statusColors(status?: string) {
  switch (status) {
    case "running":
    case "ready": return { bg: "bg-emerald-500/15", border: "border-emerald-500/40", text: "text-emerald-400", dot: "bg-emerald-500", edge: "#10b981" };
    case "failed": return { bg: "bg-red-500/15", border: "border-red-500/40", text: "text-red-400", dot: "bg-red-500", edge: "#ef4444" };
    case "completed": return { bg: "bg-teal-500/15", border: "border-teal-500/40", text: "text-teal-400", dot: "bg-teal-400", edge: "#14b8a6" };
    case "planning": return { bg: "bg-blue-500/15", border: "border-blue-500/40", text: "text-blue-400", dot: "bg-blue-500", edge: "#3b82f6" };
    default: return { bg: "bg-muted", border: "border-border", text: "text-muted-foreground", dot: "bg-muted-foreground", edge: "#52525b" };
  }
}

// ── Custom node label ──────────────────────────────────────────────

function NodeLabel({ kind, title, subtitle, status, selected }: {
  kind: string; title: string; subtitle?: string; status?: string; selected?: boolean;
}) {
  const c = statusColors(status);
  return (
    <div className={cn(
      "rounded-lg border px-3 py-2.5 transition-all min-w-[160px] max-w-[200px]",
      c.border, c.bg,
      selected && "ring-2 ring-primary ring-offset-1 ring-offset-background",
    )}>
      <div className="flex items-center gap-1.5 mb-1">
        <div className={cn("h-1.5 w-1.5 rounded-full shrink-0", c.dot)} />
        <span className="text-[9px] font-medium uppercase tracking-widest text-muted-foreground truncate">{kind}</span>
      </div>
      <div className="text-xs font-semibold text-foreground truncate">{title}</div>
      {subtitle && <div className={cn("text-[10px] truncate mt-0.5", c.text)}>{subtitle}</div>}
    </div>
  );
}

// ── Dagre layout ───────────────────────────────────────────────────

const NODE_WIDTH = 200;
const NODE_HEIGHT = 75;

function layoutGraph(nodes: Node[], edges: Edge[]): Node[] {
  const g = new dagre.graphlib.Graph();
  g.setDefaultEdgeLabel(() => ({}));
  g.setGraph({ rankdir: "LR", nodesep: 40, ranksep: 80, marginx: 20, marginy: 20 });

  for (const node of nodes) {
    g.setNode(node.id, { width: NODE_WIDTH, height: NODE_HEIGHT });
  }
  for (const edge of edges) {
    g.setEdge(edge.source, edge.target);
  }

  dagre.layout(g);

  return nodes.map((node) => {
    const pos = g.node(node.id);
    return { ...node, position: { x: pos.x - NODE_WIDTH / 2, y: pos.y - NODE_HEIGHT / 2 } };
  });
}

// ── Edge builder ───────────────────────────────────────────────────

function makeEdge(id: string, source: string, target: string, label: string | undefined, color: string, animated = false): Edge {
  return {
    id, source, target, label,
    type: "smoothstep",
    animated,
    markerEnd: { type: MarkerType.ArrowClosed, color, width: 16, height: 16 },
    style: { stroke: color, strokeWidth: 1.4 },
    labelStyle: { fill: "#a1a1aa", fontSize: 10 },
    labelShowBg: false,
  };
}

// ── Build graph data ───────────────────────────────────────────────

interface GraphProps {
  agents: Agent[];
  runs: Run[];
  share: ShareState | null;
  activeAgentId: string | null;
  activeRunId: string | null;
  activeTaskId: string | null;
  onSelectAgent?: (id: string) => void;
  onSelectRun?: (id: string) => void;
  onSelectTask?: (id: string) => void;
}

function buildGraph(props: GraphProps) {
  const { agents, runs, share, activeAgentId, activeRunId, activeTaskId } = props;
  const rawNodes: Node[] = [];
  const edges: Edge[] = [];

  // Workspace node (always present)
  rawNodes.push({
    id: "workspace",
    position: { x: 0, y: 0 },
    data: {
      label: <NodeLabel kind="Control Plane" title="Workspace" subtitle="orchestration hub" status="ready" />,
    },
    style: { background: "transparent", border: "none", padding: 0, width: NODE_WIDTH },
  });

  // Share node
  if (share) {
    const active = share.state === "running" || share.state === "starting";
    rawNodes.push({
      id: "share",
      position: { x: 0, y: 0 },
      data: {
        label: <NodeLabel kind="Access" title="Quick Tunnel" subtitle={share.state} status={active ? "running" : undefined} />,
      },
      style: { background: "transparent", border: "none", padding: 0, width: NODE_WIDTH },
    });
    edges.push(makeEdge("ws-share", "workspace", "share", "tunnel", active ? "#10b981" : "#52525b", share.state === "starting"));
  }

  // Agent nodes
  for (const agent of agents) {
    const selected = activeAgentId === agent.id;
    rawNodes.push({
      id: `agent-${agent.id}`,
      position: { x: 0, y: 0 },
      data: {
        label: <NodeLabel
          kind={agent.role || "agent"}
          title={agent.name}
          subtitle={`${agent.status ?? "idle"}${agent.model ? ` \u2022 ${agent.model}` : ""}`}
          status={agent.status}
          selected={selected}
        />,
        agentId: agent.id,
      },
      style: { background: "transparent", border: "none", padding: 0, width: NODE_WIDTH },
    });
    edges.push(makeEdge(`ws-a-${agent.id}`, "workspace", `agent-${agent.id}`, "session", selected ? "#10b981" : "#52525b"));
  }

  // Run nodes
  for (const run of runs) {
    const selected = activeRunId === run.id;
    const rc = statusColors(run.status);
    rawNodes.push({
      id: `run-${run.id}`,
      position: { x: 0, y: 0 },
      data: {
        label: <NodeLabel
          kind="Run"
          title={run.name}
          subtitle={`${run.status ?? "draft"} \u2022 ${run.tasks.length} tasks`}
          status={run.status}
          selected={selected}
        />,
        runId: run.id,
      },
      style: { background: "transparent", border: "none", padding: 0, width: NODE_WIDTH },
    });

    // Arbitrator edge
    if (run.arbitratorAgentId) {
      edges.push(makeEdge(`arb-${run.id}`, `agent-${run.arbitratorAgentId}`, `run-${run.id}`, "arbitrates", "#3b82f6", selected));
    }

    // Worker edges
    for (const wid of run.workerAgentIds) {
      edges.push(makeEdge(`wkr-${run.id}-${wid}`, `agent-${wid}`, `run-${run.id}`, "worker", selected ? "#10b981" : "#52525b"));
    }

    // Task nodes
    for (const task of run.tasks) {
      const taskSelected = activeTaskId === task.id;
      const tc = statusColors(task.status);
      rawNodes.push({
        id: `task-${task.id}`,
        position: { x: 0, y: 0 },
        data: {
          label: <NodeLabel
            kind="Task"
            title={task.title}
            subtitle={task.status}
            status={task.status}
            selected={taskSelected}
          />,
          taskId: task.id,
        },
        style: { background: "transparent", border: "none", padding: 0, width: NODE_WIDTH },
      });

      edges.push(makeEdge(`rt-${task.id}`, `run-${run.id}`, `task-${task.id}`, undefined, taskSelected ? "#10b981" : "#52525b"));

      if (task.assignedAgentId) {
        edges.push(makeEdge(`at-${task.id}`, `agent-${task.assignedAgentId}`, `task-${task.id}`, "assigned", tc.edge, task.status === "running"));
      }
    }
  }

  const nodes = layoutGraph(rawNodes, edges);
  return { nodes, edges };
}

// ── Legend ──────────────────────────────────────────────────────────

function Legend() {
  const items = [
    { color: "bg-emerald-500", label: "Running / Ready" },
    { color: "bg-blue-500", label: "Planning" },
    { color: "bg-teal-400", label: "Completed" },
    { color: "bg-red-500", label: "Failed" },
    { color: "bg-muted-foreground", label: "Idle / Draft" },
  ];

  return (
    <div className="absolute bottom-3 left-3 z-10 rounded-lg border border-border bg-card/90 px-3 py-2 backdrop-blur-sm">
      <div className="mb-1 text-[9px] font-medium uppercase tracking-widest text-muted-foreground">Legend</div>
      <div className="flex flex-wrap gap-x-3 gap-y-1">
        {items.map((item) => (
          <div key={item.label} className="flex items-center gap-1.5">
            <div className={cn("h-2 w-2 rounded-full", item.color)} />
            <span className="text-[10px] text-muted-foreground">{item.label}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

// ── Component ──────────────────────────────────────────────────────

export function AgentGraph(props: GraphProps) {
  const { agents, runs, share, activeAgentId, activeRunId, activeTaskId, onSelectAgent, onSelectRun, onSelectTask } = props;
  const taskCount = runs.reduce((sum, r) => sum + r.tasks.length, 0);

  const { nodes, edges } = useMemo(
    () => buildGraph(props),
    [agents, runs, share, activeAgentId, activeRunId, activeTaskId],
  );

  const onNodeClick: NodeMouseHandler = useCallback((_event, node) => {
    if (node.data.agentId && onSelectAgent) onSelectAgent(node.data.agentId as string);
    if (node.data.runId && onSelectRun) onSelectRun(node.data.runId as string);
    if (node.data.taskId && onSelectTask) onSelectTask(node.data.taskId as string);
  }, [onSelectAgent, onSelectRun, onSelectTask]);

  const graphKey = `${agents.length}:${runs.length}:${taskCount}:${share?.state ?? "idle"}:${activeRunId ?? "none"}:${activeTaskId ?? "none"}`;

  return (
    <div className="relative h-full w-full overflow-hidden rounded-lg border border-border bg-background">
      <ReactFlow
        key={graphKey}
        fitView
        fitViewOptions={{ padding: 0.3, minZoom: 0.5, maxZoom: 1.2 }}
        nodes={nodes}
        edges={edges}
        nodesDraggable={false}
        nodesConnectable={false}
        onNodeClick={onNodeClick}
        proOptions={{ hideAttribution: true }}
      >
        <MiniMap pannable zoomable className="!border !border-border !bg-card" />
        <Controls className="!bg-card" showInteractive={false} />
        <Background color="oklch(0.3 0 0)" gap={24} />
      </ReactFlow>
      <Legend />
    </div>
  );
}
