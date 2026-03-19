"use client";

import { useVorkerState, transportLabel } from "@/hooks/use-vorker";
import { WorkspaceSidebar } from "@/components/workspace-sidebar";
import { CenterPanel, MobileConsolePanel } from "@/components/center-panel";
import { ReviewPanel } from "@/components/review-panel";
import { PermissionModal } from "@/components/permission-modal";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Lock } from "lucide-react";

export default function DashboardPage() {
  const vorker = useVorkerState();

  if (!vorker.app.authenticated) {
    return (
      <div className="flex h-screen w-full items-center justify-center bg-background p-4">
        <Card className="w-full max-w-md">
          <CardHeader className="text-center">
            <div className="mx-auto mb-3 flex h-12 w-12 items-center justify-center rounded-full bg-primary/10">
              <Lock className="h-6 w-6 text-primary" />
            </div>
            <CardTitle>Unlock Workspace</CardTitle>
            <CardDescription>
              Authenticate with the pairing password printed by the server, then the dashboard will attach to the live orchestration stream.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <form
              className="space-y-4"
              onSubmit={(e) => {
                e.preventDefault();
                void vorker.handleLogin(vorker.loginPassword);
              }}
            >
              <div className="space-y-2">
                <label className="text-sm text-muted-foreground">Password</label>
                <Input
                  type="password"
                  autoComplete="current-password"
                  placeholder="Pairing password"
                  value={vorker.loginPassword}
                  onChange={(e) => vorker.setLoginPassword(e.target.value)}
                />
              </div>
              {vorker.bootError && (
                <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive-foreground">
                  {vorker.bootError}
                </div>
              )}
              {vorker.authError && (
                <div className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive-foreground">
                  {vorker.authError}
                </div>
              )}
              <Button type="submit" className="w-full">
                Connect
              </Button>
            </form>
          </CardContent>
        </Card>
      </div>
    );
  }

  return (
    <>
      <div className="hidden h-screen w-full overflow-hidden bg-background lg:flex">
        <WorkspaceSidebar vorker={vorker} />

        <main className="flex-1 overflow-hidden border-x border-border">
          <CenterPanel vorker={vorker} />
        </main>

        <ReviewPanel vorker={vorker} />
      </div>

      <div className="flex h-dvh w-full overflow-hidden bg-background lg:hidden">
        <main className="flex-1 overflow-hidden">
          <MobileConsolePanel vorker={vorker} />
        </main>
      </div>

      <PermissionModal vorker={vorker} />
    </>
  );
}
