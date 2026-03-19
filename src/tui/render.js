import { TITLE_ART, colorize, emphasize, fit, hardWrap, highlight, pad, truncate, visibleLength } from "./theme.js";

function statusColor(status) {
  switch (status) {
    case "ready":
    case "completed":
    case "merged":
      return "brightGreen";
    case "running":
    case "planning":
    case "starting":
      return "yellow";
    case "failed":
    case "error":
    case "conflict":
      return "red";
    default:
      return "gray";
  }
}

function laneMeter(status, color) {
  switch (status) {
    case "completed":
    case "merged":
      return colorize("■■■■", "brightGreen", color);
    case "running":
    case "planning":
    case "starting":
      return `${colorize("■■■", "yellow", color)}${colorize("□", "brightBlack", color)}`;
    case "ready":
      return `${colorize("■", "green", color)}${colorize("□□□", "brightBlack", color)}`;
    case "failed":
    case "error":
    case "conflict":
      return colorize("■■■■", "red", color);
    default:
      return colorize("□□□□", "brightBlack", color);
  }
}

function summarizeEvent(event) {
  switch (event?.type) {
    case "task.updated":
      return `task ${event.payload?.task?.title ?? event.payload?.task?.id ?? "unknown"} -> ${event.payload?.task?.status ?? "updated"}`;
    case "task.created":
      return `task ${event.payload?.task?.title ?? event.payload?.task?.id ?? "unknown"} created`;
    case "run.updated":
      return `run ${event.payload?.run?.name ?? event.payload?.run?.id ?? "unknown"} -> ${event.payload?.run?.status ?? "updated"}`;
    case "run.created":
      return `run ${event.payload?.run?.name ?? event.payload?.run?.id ?? "unknown"} created`;
    case "session.registered":
      return `session ${event.payload?.session?.name ?? event.payload?.session?.id ?? "unknown"} ready`;
    case "session.prompt.started":
      return `prompt -> ${event.payload?.sessionId ?? "session"}`;
    case "session.prompt.finished":
      return `reply <- ${event.payload?.sessionId ?? "session"}`;
    case "skills.updated":
      return `skills refreshed (${event.payload?.skills?.length ?? 0})`;
    case "share.updated":
      return `tunnel ${event.payload?.share?.state ?? "idle"}`;
    default:
      return event?.type ?? "event";
  }
}

function appendField(lines, label, value, width, color, options = {}) {
  const indent = options.indent ?? "  ";
  const labelText = `${indent}${colorize(label, "gray", color)}`;
  const valueText = String(value ?? "");
  const innerWidth = Math.max(12, width - 2);
  const inline = `${labelText} ${valueText}`;

  if (!options.stacked && visibleLength(inline) <= innerWidth) {
    lines.push(inline);
    return;
  }

  lines.push(labelText);

  const continuationIndent = indent + (options.valueIndent ?? "  ");
  const wrapWidth = Math.max(8, innerWidth - continuationIndent.length);
  for (const chunk of hardWrap(valueText, wrapWidth)) {
    lines.push(`${continuationIndent}${chunk}`);
  }
}

function styleSelectable(line, selected, color) {
  if (selected) {
    return color ? highlight(` ${truncate(line, 999)} `, color, { background: "bgGreen", foreground: "black" }) : `> ${line}`;
  }
  return line;
}

function buildPanel(title, lines, width, options = {}) {
  const color = Boolean(options.color);
  const innerWidth = Math.max(12, width - 2);
  const focused = Boolean(options.focused);
  const borderColor = focused ? "brightGreen" : options.borderColor ?? "green";
  const plainTitle = ` ${title} `;
  const fillerWidth = Math.max(0, innerWidth - plainTitle.length);
  const leftFill = "─".repeat(Math.floor(fillerWidth / 2));
  const rightFill = "─".repeat(Math.ceil(fillerWidth / 2));
  const titleText = focused
    ? highlight(plainTitle, color, { background: "bgGreen", foreground: "black" })
    : colorize(emphasize(plainTitle, color), borderColor, color);
  const top = `${colorize(`┌${leftFill}`, borderColor, color)}${titleText}${colorize(`${rightFill}┐`, borderColor, color)}`;
  const body = (lines.length > 0 ? lines : [""]).map(
    (line) => `${colorize("│", borderColor, color)}${fit(line, innerWidth)}${colorize("│", borderColor, color)}`,
  );
  const bottom = colorize(`└${"─".repeat(innerWidth)}┘`, borderColor, color);
  return [top, ...body, bottom];
}

function combineColumns(leftLines, rightLines, gap = 1) {
  const leftWidth = Math.max(...leftLines.map((line) => visibleLength(line)));
  const height = Math.max(leftLines.length, rightLines.length);
  const result = [];

  for (let index = 0; index < height; index += 1) {
    const left = leftLines[index] ?? " ".repeat(leftWidth);
    const right = rightLines[index] ?? "";
    result.push(`${pad(left, leftWidth)}${" ".repeat(gap)}${right}`);
  }

  return result;
}

function renderBanner(width, color) {
  const strapline = [
    emphasize(colorize("VORKER CONTROL PLANE", "brightGreen", color), color),
    colorize("VORKER-2 supervisor mesh", "green", color),
    colorize("arrow nav / task lanes / cloud tunnel", "gray", color),
  ].join("   ");

  return [
    ...TITLE_ART.map((line) => colorize(line, "brightGreen", color)),
    strapline,
    colorize("Agent lanes stay hot, merges stay visible, prompts stay one keystroke away.", "gray", color),
    colorize("─".repeat(Math.max(40, Math.min(width, 120))), "green", color),
  ];
}

function renderSessionList(snapshot, options, width, color) {
  const sessions = snapshot.sessions ?? [];
  const lines = sessions.length
    ? sessions.map((session) => {
        const selected = session.id === options.activeSessionId;
        const status = colorize((session.status ?? "unknown").toUpperCase(), statusColor(session.status), color);
        const model = session.model ? colorize(session.model, "gray", color) : colorize("no-model", "gray", color);
        const line = `${selected ? "▶" : "•"} ${session.name} ${status} ${colorize(`[${session.role ?? "worker"}]`, "gray", color)} ${model}`;
        return styleSelectable(line, selected, color);
      })
    : [colorize("No agents yet. Use /agent <name> to start one.", "gray", color)];

  return buildPanel("ACTIVE SESSIONS", lines, width, {
    color,
    focused: options.focusedPane === "sessions",
  });
}

function renderRunBoard(snapshot, options, width, color) {
  const runs = snapshot.runs ?? [];
  const activeRun = runs.find((entry) => entry.id === options.activeRunId) ?? runs[0] ?? null;

  if (!activeRun) {
    return buildPanel("RUN BOARD", [colorize("No runs yet. Use /run <name> | <goal>.", "gray", color)], width, {
      color,
      focused: options.focusedPane === "runs" || options.focusedPane === "tasks",
    });
  }

  const taskCounts = {
    ready: activeRun.tasks.filter((task) => task.status === "ready").length,
    running: activeRun.tasks.filter((task) => task.status === "running").length,
    completed: activeRun.tasks.filter((task) => task.status === "completed").length,
    failed: activeRun.tasks.filter((task) => task.status === "failed").length,
  };

  const lines = [
    `${colorize("run", "gray", color)} ${activeRun.name} ${colorize((activeRun.status ?? "draft").toUpperCase(), statusColor(activeRun.status), color)}`,
    `${colorize("goal", "gray", color)} ${truncate(activeRun.goal ?? "", width - 10)}`,
    `${colorize("lanes", "gray", color)} hot=${taskCounts.running} ready=${taskCounts.ready} done=${taskCounts.completed} fail=${taskCounts.failed}`,
    colorize("─".repeat(Math.max(10, width - 6)), "brightBlack", color),
  ];

  const selectedTask = (activeRun.tasks ?? []).find((task) => task.id === options.selectedTaskId) ?? activeRun.tasks?.[0] ?? null;

  if ((activeRun.tasks ?? []).length === 0) {
    lines.push(colorize("No tasks in this run.", "gray", color));
  } else {
    for (const task of activeRun.tasks.slice(0, 7)) {
      const selected = task.id === selectedTask?.id;
      const status = colorize((task.status ?? "draft").toUpperCase(), statusColor(task.status), color);
      const agentId = task.executionAgentId ?? task.assignedAgentId ?? "queue";
      const line = `${laneMeter(task.status, color)} ${task.title} ${status} ${colorize(agentId, "gray", color)}`;
      lines.push(styleSelectable(line, selected, color));
    }
  }

  if (selectedTask) {
    lines.push(colorize("─".repeat(Math.max(10, width - 6)), "brightBlack", color));
    lines.push(colorize("selected lane", "gray", color));
    appendField(lines, "task", selectedTask.title, width, color, { stacked: true });
    appendField(lines, "agent", selectedTask.executionAgentId ?? selectedTask.assignedAgentId ?? "queue", width, color);
    if (selectedTask.branchName) {
      appendField(lines, "branch", selectedTask.branchName, width, color, { stacked: true });
    }
    if (selectedTask.commitSha) {
      appendField(lines, "commit", `${selectedTask.commitSha} (${selectedTask.changeCount ?? 0} files)`, width, color);
    }
    if (selectedTask.mergeStatus) {
      appendField(
        lines,
        "merge",
        `${selectedTask.mergeStatus}${selectedTask.mergeCommitSha ? ` ${selectedTask.mergeCommitSha}` : ""}`,
        width,
        color,
      );
    }
  }

  return buildPanel("RUN BOARD", lines, width, {
    color,
    focused: options.focusedPane === "runs" || options.focusedPane === "tasks",
  });
}

function renderActiveSession(snapshot, options, width, color) {
  const session = (snapshot.sessions ?? []).find((entry) => entry.id === options.activeSessionId) ?? snapshot.sessions?.[0] ?? null;

  if (!session) {
    return buildPanel("ACTIVE SESSION", [colorize("No active session selected.", "gray", color)], width, {
      color,
      focused: options.focusedPane === "active",
    });
  }

  const lines = [
    `${colorize("name", "gray", color)} ${session.name}`,
    `${colorize("role", "gray", color)} ${session.role ?? "worker"}    ${colorize("status", "gray", color)} ${colorize((session.status ?? "unknown").toUpperCase(), statusColor(session.status), color)}`,
    `${colorize("model", "gray", color)} ${session.model ?? "unset"}`,
  ];
  appendField(lines, "cwd", session.cwd ?? "", width, color, { stacked: true });
  lines.push(colorize("─".repeat(Math.max(10, width - 6)), "brightBlack", color));

  const transcript = session.transcript ?? [];
  if (transcript.length === 0) {
    lines.push(colorize("No transcript yet.", "gray", color));
  } else {
    for (const entry of transcript.slice(-6)) {
      const roleColor = entry.role === "assistant" ? "brightGreen" : entry.role === "user" ? "green" : "gray";
      lines.push(`${colorize(entry.role.padEnd(9), roleColor, color)} ${truncate(entry.text, width - 12)}`);
    }
  }

  return buildPanel("ACTIVE SESSION", lines, width, { color });
}

function renderEventFeed(snapshot, options, width, color) {
  const lines = (snapshot.events ?? [])
    .slice(-8)
    .reverse()
    .map((event) => {
      const tone = event?.type?.includes("failed") ? "red" : event?.type?.includes("updated") ? "green" : "gray";
      return `${colorize("•", tone, color)} ${summarizeEvent(event)}`;
    });

  return buildPanel(
    "EVENT FEED",
    lines.length > 0 ? lines : [colorize("No supervisor events yet.", "gray", color)],
    width,
    {
      color,
      focused: options.focusedPane === "events",
    },
  );
}

function renderFooter(snapshot, options, width, color) {
  const shareState = snapshot.share?.state ?? "idle";
  const lines = [
    `${colorize("status", "gray", color)} ${options.statusLine ?? "Ready."}`,
    `${colorize("focus", "gray", color)} ${options.focusedPane ?? "sessions"}    ${colorize("tunnel", "gray", color)} ${colorize(shareState.toUpperCase(), statusColor(shareState), color)}`,
  ];
  appendField(lines, "url", snapshot.share?.publicUrl ?? "not shared", width, color, { stacked: true });
  appendField(
    lines,
    "input >",
    options.commandBuffer?.length ? options.commandBuffer : "type a prompt or /command and press Enter",
    width,
    color,
    { stacked: true },
  );
  lines.push(colorize("arrows move  tab cycles panes  enter sends  esc clears  ctrl+c quits", "gray", color));
  return buildPanel("COMMAND DECK", lines, width, { color, focused: true });
}

export function renderDashboard(snapshot, options = {}) {
  const color = Boolean(options.color);
  const width = Math.max(80, Math.min(Number(options.width ?? 120), 160));
  const leftWidth = width >= 130 ? 46 : Math.floor(width * 0.42);
  const rightWidth = width - leftWidth - 1;

  const panelOptions = {
    activeSessionId: options.activeSessionId ?? null,
    activeRunId: options.activeRunId ?? null,
    selectedTaskId: options.selectedTaskId ?? null,
    focusedPane: options.focusedPane ?? "sessions",
  };

  const sessionPanel = renderSessionList(snapshot, panelOptions, leftWidth, color);
  const runPanel = renderRunBoard(snapshot, panelOptions, leftWidth, color);
  const activePanel = renderActiveSession(snapshot, panelOptions, rightWidth, color);
  const eventPanel = renderEventFeed(snapshot, panelOptions, rightWidth, color);

  const rows = [
    ...renderBanner(width, color),
    ...combineColumns(sessionPanel, activePanel),
    ...combineColumns(runPanel, eventPanel),
    ...renderFooter(snapshot, options, width, color),
  ];

  return rows.join("\n");
}
