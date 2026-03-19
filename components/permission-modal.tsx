"use client";

import { ShieldAlert } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle,
} from "@/components/ui/dialog";
import type { PermissionOption } from "@/hooks/use-vorker";

export function PermissionModal({ vorker }: { vorker: any }) {
  const { app, setApp, sendCommand } = vorker;
  const pending = app.pendingPermission;

  if (!pending) return null;

  function respond(option: PermissionOption) {
    void sendCommand({
      type: "permission_response",
      requestId: pending.requestId,
      outcome: "selected",
      optionId: option.optionId,
    });
    setApp((cur: any) => ({ ...cur, pendingPermission: null }));
  }

  return (
    <Dialog open={true} onOpenChange={() => {}}>
      <DialogContent className="sm:max-w-md" onPointerDownOutside={(e) => e.preventDefault()}>
        <DialogHeader>
          <div className="flex items-center gap-2">
            <ShieldAlert className="h-5 w-5 text-primary" />
            <DialogTitle className="text-sm">{pending.toolCall?.title ?? "Permission Request"}</DialogTitle>
          </div>
          <DialogDescription className="text-xs">
            An agent is requesting approval for a tool call. Choose how to handle it.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-2 pt-2">
          {pending.options.map((option: PermissionOption) => (
            <Button
              key={option.optionId}
              variant="outline"
              className="w-full justify-start text-left text-xs"
              onClick={() => respond(option)}
            >
              <span className="font-medium">{option.name}</span>
              <span className="ml-2 text-muted-foreground">({option.kind})</span>
            </Button>
          ))}
        </div>
      </DialogContent>
    </Dialog>
  );
}
