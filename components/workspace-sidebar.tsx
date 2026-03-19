"use client";

import { useState } from "react";
import {
  Plus, Cloud, Palette, LogOut, Play, Square, ChevronDown, ChevronRight,
  Server, Globe, KeyRound, Link2, Bot, Workflow,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import {
  Popover, PopoverContent, PopoverTrigger,
} from "@/components/ui/popover";
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Collapsible, CollapsibleContent, CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { cn } from "@/lib/utils";
import { useTheme, type ThemeColor } from "@/components/theme-context";
import { transportLabel, type Agent, type Run } from "@/hooks/use-vorker";

const themeColors: { value: ThemeColor; label: string; color: string }[] = [
  { value: "green", label: "Green", color: "bg-emerald-500" },
  { value: "red", label: "Red", color: "bg-red-500" },
  { value: "blue", label: "Blue", color: "bg-blue-500" },
  { value: "yellow", label: "Yellow", color: "bg-amber-500" },
];

export function WorkspaceSidebar({ vorker }: { vorker: any }) {
  const { themeColor, setThemeColor } = useTheme();
  const { app, setApp, sendCommand, handleLogout, shareForm, setShareForm, setInspectorTab } = vorker;
  const [agentsOpen, setAgentsOpen] = useState(true);
  const [runsOpen, setRunsOpen] = useState(true);
  const [tunnelOpen, setTunnelOpen] = useState(false);

  return (
    <div className="flex h-full w-64 flex-col border-r border-border bg-card">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-border px-3 py-2.5">
        <span className="text-sm font-semibold tracking-widest text-muted-foreground uppercase">vorker</span>
        <Badge variant="outline" className="text-[10px]">
          {transportLabel(app)}
        </Badge>
      </div>

      {/* Workspace Info */}
      <div className="space-y-1 border-b border-border px-3 py-3">
        <InfoRow icon={Server} label="Root" value={app.serverCwd || "Unknown"} />
        <InfoRow icon={Globe} label="Origin" value={vorker.clientOrigin || "Loading..."} />
        <InfoRow icon={KeyRound} label="Password" value={app.pairingPassword || "Hidden"} />
        <InfoRow icon={Link2} label="Tunnel" value={app.share?.publicUrl ?? "Not started"} />
      </div>

      <ScrollArea className="flex-1">
        {/* Agents Section */}
        <Collapsible open={agentsOpen} onOpenChange={setAgentsOpen}>
          <CollapsibleTrigger className="flex w-full items-center justify-between px-3 py-2 hover:bg-accent">
            <div className="flex items-center gap-2 text-sm font-medium text-foreground">
              <Bot className="h-4 w-4 text-primary" />
              Agents
              <span className="text-muted-foreground">({app.agents.length})</span>
            </div>
            {agentsOpen ? <ChevronDown className="h-3.5 w-3.5 text-muted-foreground" /> : <ChevronRight className="h-3.5 w-3.5 text-muted-foreground" />}
          </CollapsibleTrigger>
          <CollapsibleContent>
            <div className="space-y-0.5 px-1 pb-2">
              {app.agents.length === 0 ? (
                <div className="px-3 py-3 text-xs text-muted-foreground">No agents running.</div>
              ) : (
                app.agents.map((agent: Agent) => (
                  <button
                    key={agent.id}
                    type="button"
                    onClick={() => {
                      setInspectorTab("agent");
                      setApp((cur: any) => ({ ...cur, activeAgentId: agent.id }));
                    }}
                    className={cn(
                      "flex w-full items-start gap-2 rounded-md px-2 py-1.5 text-left hover:bg-accent",
                      app.activeAgentId === agent.id && "bg-accent",
                    )}
                  >
                    <div className="mt-1.5">
                      <div className={cn("h-2 w-2 rounded-full", statusColor(agent.status))} />
                    </div>
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center justify-between gap-2">
                        <span className="truncate text-sm font-medium text-foreground">{agent.name}</span>
                        <Badge variant="outline" className="text-[10px] shrink-0">{agent.status}</Badge>
                      </div>
                      <div className="text-xs text-muted-foreground">
                        {agent.role || "worker"} {agent.model ? `\u2022 ${agent.model}` : ""}
                      </div>
                    </div>
                  </button>
                ))
              )}
              <Button
                variant="ghost"
                size="sm"
                className="w-full justify-start gap-2 text-muted-foreground"
                onClick={() => setInspectorTab("create")}
              >
                <Plus className="h-3.5 w-3.5" /> Add Agent
              </Button>
            </div>
          </CollapsibleContent>
        </Collapsible>

        <Separator />

        {/* Runs Section */}
        <Collapsible open={runsOpen} onOpenChange={setRunsOpen}>
          <CollapsibleTrigger className="flex w-full items-center justify-between px-3 py-2 hover:bg-accent">
            <div className="flex items-center gap-2 text-sm font-medium text-foreground">
              <Workflow className="h-4 w-4 text-primary" />
              Runs
              <span className="text-muted-foreground">({app.runs.length})</span>
            </div>
            {runsOpen ? <ChevronDown className="h-3.5 w-3.5 text-muted-foreground" /> : <ChevronRight className="h-3.5 w-3.5 text-muted-foreground" />}
          </CollapsibleTrigger>
          <CollapsibleContent>
            <div className="space-y-0.5 px-1 pb-2">
              {app.runs.length === 0 ? (
                <div className="px-3 py-3 text-xs text-muted-foreground">No runs yet.</div>
              ) : (
                app.runs.map((run: Run) => (
                  <button
                    key={run.id}
                    type="button"
                    onClick={() => {
                      setInspectorTab("tasks");
                      setApp((cur: any) => ({ ...cur, activeRunId: run.id, activeTaskId: run.tasks[0]?.id ?? null }));
                    }}
                    className={cn(
                      "flex w-full items-start gap-2 rounded-md px-2 py-1.5 text-left hover:bg-accent",
                      app.activeRunId === run.id && "bg-accent",
                    )}
                  >
                    <div className="mt-1.5">
                      <div className={cn("h-2 w-2 rounded-full", statusColor(run.status))} />
                    </div>
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center justify-between gap-2">
                        <span className="truncate text-sm font-medium text-foreground">{run.name}</span>
                        <Badge variant="outline" className="text-[10px] shrink-0">{run.status}</Badge>
                      </div>
                      <div className="line-clamp-1 text-xs text-muted-foreground">{run.goal}</div>
                    </div>
                  </button>
                ))
              )}
              <Button
                variant="ghost"
                size="sm"
                className="w-full justify-start gap-2 text-muted-foreground"
                onClick={() => setInspectorTab("create")}
              >
                <Plus className="h-3.5 w-3.5" /> New Run
              </Button>
            </div>
          </CollapsibleContent>
        </Collapsible>
      </ScrollArea>

      {/* Bottom Actions */}
      <div className="border-t border-border">
        {/* Cloudflare Tunnel */}
        <Popover open={tunnelOpen} onOpenChange={setTunnelOpen}>
          <PopoverTrigger asChild>
            <button className="flex w-full items-center gap-2 border-b border-border px-3 py-2.5 text-sm text-foreground hover:bg-accent">
              <Cloud className={cn("h-4 w-4", app.share?.state === "running" ? "text-emerald-400" : "text-primary")} />
              <span className="flex-1 text-left">cloudflare tunnel</span>
              {app.share?.state === "running" && <div className="h-2 w-2 rounded-full bg-emerald-500" />}
            </button>
          </PopoverTrigger>
          <PopoverContent align="start" side="top" className="w-64 space-y-3">
            <div className="text-sm font-medium">Tunnel Settings</div>
            <div className="space-y-2">
              <label className="text-xs text-muted-foreground">cloudflared binary</label>
              <Input
                className="h-8 text-xs"
                value={shareForm.cloudflaredBin}
                onChange={(e) => setShareForm((c: any) => ({ ...c, cloudflaredBin: e.target.value }))}
              />
            </div>
            <div className="grid grid-cols-2 gap-2">
              <div className="space-y-1">
                <label className="text-xs text-muted-foreground">Protocol</label>
                <Input
                  className="h-8 text-xs"
                  value={shareForm.edgeProtocol}
                  onChange={(e) => setShareForm((c: any) => ({ ...c, edgeProtocol: e.target.value }))}
                />
              </div>
              <div className="space-y-1">
                <label className="text-xs text-muted-foreground">IP Mode</label>
                <Input
                  className="h-8 text-xs"
                  value={shareForm.edgeIpVersion}
                  onChange={(e) => setShareForm((c: any) => ({ ...c, edgeIpVersion: e.target.value }))}
                />
              </div>
            </div>
            <div className="grid grid-cols-2 gap-2">
              <Button
                size="sm"
                onClick={() => {
                  void sendCommand({ type: "share_start", ...shareForm });
                  setTunnelOpen(false);
                }}
              >
                <Play className="mr-1 h-3 w-3" /> Start
              </Button>
              <Button
                size="sm"
                variant="secondary"
                onClick={() => {
                  void sendCommand({ type: "share_stop" });
                  setTunnelOpen(false);
                }}
              >
                <Square className="mr-1 h-3 w-3" /> Stop
              </Button>
            </div>
            {app.share?.publicUrl && (
              <div className="break-all rounded-md bg-secondary px-2 py-1.5 text-xs text-muted-foreground">
                {app.share.publicUrl}
              </div>
            )}
          </PopoverContent>
        </Popover>

        {/* Theme */}
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button className="flex w-full items-center gap-2 border-b border-border px-3 py-2.5 text-sm text-foreground hover:bg-accent">
              <Palette className="h-4 w-4 text-primary" />
              theme
              <div className={cn("ml-auto h-3 w-3 rounded-full", themeColors.find((t) => t.value === themeColor)?.color)} />
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="start" className="w-40">
            {themeColors.map((theme) => (
              <DropdownMenuItem key={theme.value} onClick={() => setThemeColor(theme.value)} className="flex items-center gap-2">
                <div className={cn("h-3 w-3 rounded-full", theme.color)} />
                {theme.label}
                {themeColor === theme.value && <span className="ml-auto text-primary">*</span>}
              </DropdownMenuItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>

        {/* Logout */}
        <button
          className="flex w-full items-center gap-2 px-3 py-2.5 text-sm text-muted-foreground hover:bg-accent hover:text-foreground"
          onClick={handleLogout}
        >
          <LogOut className="h-4 w-4" />
          Logout
        </button>
      </div>
    </div>
  );
}

function InfoRow({ icon: Icon, label, value }: { icon: any; label: string; value: string }) {
  return (
    <div className="flex items-start gap-2">
      <Icon className="mt-0.5 h-3.5 w-3.5 shrink-0 text-muted-foreground" />
      <div className="min-w-0 flex-1">
        <div className="text-[10px] uppercase tracking-widest text-muted-foreground">{label}</div>
        <div className="truncate text-xs text-foreground">{value}</div>
      </div>
    </div>
  );
}

function statusColor(status?: string): string {
  switch (status) {
    case "running":
    case "ready": return "bg-emerald-500";
    case "failed": return "bg-red-500";
    case "completed": return "bg-teal-400";
    case "planning": return "bg-blue-500";
    default: return "bg-zinc-500";
  }
}
