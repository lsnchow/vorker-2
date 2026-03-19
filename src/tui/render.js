function truncate(text, maxLength) {
  const value = String(text ?? "");
  if (value.length <= maxLength) {
    return value;
  }
  return `${value.slice(0, Math.max(0, maxLength - 1))}…`;
}

function renderSessions(snapshot, activeSessionId) {
  const sessions = snapshot.sessions ?? [];
  if (sessions.length === 0) {
    return "Sessions\n  No agents yet. Use /agent <name> to start one.";
  }

  const lines = ["Sessions"];
  for (const session of sessions) {
    const marker = session.id === activeSessionId ? ">" : " ";
    lines.push(
      `${marker} ${truncate(session.name, 20)} [${session.role ?? "worker"}] ${session.status ?? "unknown"} ${session.model ?? ""}`.trim(),
    );
  }
  return lines.join("\n");
}

function renderRuns(snapshot, activeRunId) {
  const runs = snapshot.runs ?? [];
  if (runs.length === 0) {
    return "Runs\n  No runs yet. Use /run <name> | <goal> to create one.";
  }

  const lines = ["Runs"];
  for (const run of runs) {
    const marker = run.id === activeRunId ? ">" : " ";
    lines.push(`${marker} ${truncate(run.name, 28)} (${run.status ?? "draft"})`);
    for (const task of (run.tasks ?? []).slice(0, 6)) {
      lines.push(`    - ${truncate(task.title, 36)} [${task.status ?? "draft"}]`);
      if (task.executionAgentId) {
        lines.push(`      exec ${truncate(task.executionAgentId, 24)}`);
      }
      if (task.branchName) {
        lines.push(`      branch ${truncate(task.branchName, 56)}`);
      }
      if (task.workspacePath) {
        lines.push(`      ws ${truncate(task.workspacePath, 56)}`);
      }
      if (task.commitSha) {
        lines.push(`      commit ${truncate(task.commitSha, 16)} (${task.changeCount ?? 0} files)`);
      }
      if (task.mergeStatus) {
        lines.push(`      merge ${task.mergeStatus}${task.mergeCommitSha ? ` ${truncate(task.mergeCommitSha, 16)}` : ""}`);
      }
    }
  }
  return lines.join("\n");
}

function renderTranscript(snapshot, activeSessionId) {
  const session = (snapshot.sessions ?? []).find((entry) => entry.id === activeSessionId) ?? snapshot.sessions?.[0];
  const transcript = session?.transcript ?? [];
  const lines = ["Transcript"];

  if (transcript.length === 0) {
    lines.push("  No messages yet.");
    return lines.join("\n");
  }

  for (const entry of transcript.slice(-6)) {
    lines.push(`  ${entry.role}: ${truncate(entry.text, 72)}`);
  }
  return lines.join("\n");
}

export function renderDashboard(snapshot, options = {}) {
  const activeSessionId = options.activeSessionId ?? null;
  const activeRunId = options.activeRunId ?? null;
  const sessions = snapshot.sessions ?? [];
  const runs = snapshot.runs ?? [];
  const shareState = snapshot.share?.state ?? "idle";
  const shareUrl = snapshot.share?.publicUrl ? ` ${snapshot.share.publicUrl}` : "";

  const sections = [
    `VORKER-2  sessions=${sessions.length} runs=${runs.length}  Tunnel: ${shareState}${shareUrl}`,
    "═".repeat(Math.max(24, Math.min(Number(options.width ?? 100), 120))),
    renderSessions(snapshot, activeSessionId),
    "",
    renderRuns(snapshot, activeRunId),
    "",
    renderTranscript(snapshot, activeSessionId),
    "",
    "Commands: /agent <name>, /use <session-id>, /run <name> | <goal>, /run-use <run-id>, /plan, /dispatch, /merge, /merge-task <task-id>, /share start|stop, /quit",
  ];

  return sections.join("\n");
}
