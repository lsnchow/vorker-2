const ANSI_PATTERN = /\x1b\[[0-9;]*m/g;

const COLORS = {
  reset: "\x1b[0m",
  bold: "\x1b[1m",
  dim: "\x1b[2m",
  cyan: "\x1b[36m",
  green: "\x1b[32m",
  yellow: "\x1b[33m",
  red: "\x1b[31m",
  blue: "\x1b[34m",
  magenta: "\x1b[35m",
  gray: "\x1b[90m",
};

const BANNER = [
  "__     ______  ____  _  ________ _____ ",
  "\\ \\   / / __ \\/ __ \\/ |/ / __/ //_/_  /",
  " \\ \\_/ / /_/ / /_/ /    / _// ,<   / / ",
  "  \\___/\\____/\\____/_/|_/___/_/|_| /_/  ",
];

function stripAnsi(text) {
  return String(text ?? "").replace(ANSI_PATTERN, "");
}

function visibleLength(text) {
  return stripAnsi(text).length;
}

function colorize(text, color, enabled) {
  if (!enabled || !color) {
    return text;
  }
  return `${COLORS[color] ?? ""}${text}${COLORS.reset}`;
}

function emphasize(text, enabled) {
  return enabled ? `${COLORS.bold}${text}${COLORS.reset}` : text;
}

function truncate(text, maxLength) {
  const value = String(text ?? "");
  if (visibleLength(value) <= maxLength) {
    return value;
  }
  return `${stripAnsi(value).slice(0, Math.max(0, maxLength - 1))}…`;
}

function pad(text, width) {
  const value = String(text ?? "");
  const missing = Math.max(0, width - visibleLength(value));
  return `${value}${" ".repeat(missing)}`;
}

function fit(text, width) {
  return pad(truncate(text, width), width);
}

function hardWrap(text, maxLength) {
  const value = stripAnsi(text);
  if (value.length <= maxLength) {
    return [value];
  }

  const lines = [];
  let remaining = value;

  while (remaining.length > maxLength) {
    const slice = remaining.slice(0, maxLength);
    const breakAt = slice.lastIndexOf(" ");
    if (breakAt >= Math.floor(maxLength * 0.6)) {
      lines.push(remaining.slice(0, breakAt));
      remaining = remaining.slice(breakAt + 1);
      continue;
    }

    lines.push(slice);
    remaining = remaining.slice(maxLength);
  }

  if (remaining.length > 0) {
    lines.push(remaining);
  }

  return lines;
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

function statusColor(status) {
  switch (status) {
    case "ready":
    case "completed":
    case "merged":
      return "green";
    case "running":
    case "planning":
    case "starting":
      return "yellow";
    case "failed":
    case "error":
    case "conflict":
      return "red";
    case "draft":
      return "blue";
    default:
      return "gray";
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

function buildPanel(title, lines, width, options = {}) {
  const color = Boolean(options.color);
  const innerWidth = Math.max(12, width - 2);
  const renderedTitle = emphasize(title, color);
  const plainTitle = ` ${title} `;
  const fillerWidth = Math.max(0, innerWidth - plainTitle.length);
  const leftFill = "─".repeat(Math.floor(fillerWidth / 2));
  const rightFill = "─".repeat(Math.ceil(fillerWidth / 2));
  const top = `┌${leftFill}${emphasize(plainTitle, color)}${rightFill}┐`;
  const body = (lines.length > 0 ? lines : [""]).map((line) => `│${fit(line, innerWidth)}│`);
  const bottom = `└${"─".repeat(innerWidth)}┘`;
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
  const status = [
    colorize("VORKER-2", "magenta", color),
    colorize("CONTROL PLANE", "cyan", color),
    colorize("local-first / isolated / agentic", "gray", color),
  ].join("   ");
  return [...BANNER.map((line) => colorize(line, "cyan", color)), status, "─".repeat(Math.max(20, Math.min(width, 110)))];
}

function renderSessionList(snapshot, activeSessionId, width, color) {
  const sessions = snapshot.sessions ?? [];
  const lines = sessions.length
    ? sessions.map((session) => {
        const marker = session.id === activeSessionId ? colorize("▸", "cyan", color) : "•";
        const status = colorize(session.status ?? "unknown", statusColor(session.status), color);
        const model = session.model ? colorize(session.model, "gray", color) : colorize("no-model", "gray", color);
        return `${marker} ${session.name} [${session.role ?? "worker"}] ${status} ${model}`;
      })
    : [colorize("No agents yet. Use /agent <name> to start one.", "gray", color)];

  return buildPanel("ACTIVE SESSIONS", lines, width, { color });
}

function renderRunBoard(snapshot, activeRunId, width, color) {
  const runs = snapshot.runs ?? [];
  const activeRun = runs.find((entry) => entry.id === activeRunId) ?? runs[0] ?? null;

  if (!activeRun) {
    return buildPanel("RUN BOARD", [colorize("No runs yet. Use /run <name> | <goal>.", "gray", color)], width, { color });
  }

  const taskCounts = {
    ready: activeRun.tasks.filter((task) => task.status === "ready").length,
    running: activeRun.tasks.filter((task) => task.status === "running").length,
    completed: activeRun.tasks.filter((task) => task.status === "completed").length,
    failed: activeRun.tasks.filter((task) => task.status === "failed").length,
  };

  const lines = [
    `${colorize("run", "gray", color)} ${activeRun.name} ${colorize(activeRun.status ?? "draft", statusColor(activeRun.status), color)}`,
    `${colorize("goal", "gray", color)} ${activeRun.goal ?? ""}`,
    `${colorize("queue", "gray", color)} rdy=${taskCounts.ready} run=${taskCounts.running} done=${taskCounts.completed} fail=${taskCounts.failed}`,
    "─".repeat(Math.max(10, width - 6)),
  ];

  if ((activeRun.tasks ?? []).length === 0) {
    lines.push(colorize("No tasks in this run.", "gray", color));
  } else {
    for (const task of activeRun.tasks.slice(0, 6)) {
      lines.push(`${task.status === "running" ? colorize("▸", "yellow", color) : "•"} ${task.title} [${task.status ?? "draft"}]`);
      if (task.executionAgentId) {
        appendField(lines, "exec", task.executionAgentId, width, color);
      }
      if (task.branchName) {
        appendField(lines, "branch", task.branchName, width, color, { stacked: true });
      }
      if (task.commitSha) {
        appendField(lines, "commit", `${task.commitSha} (${task.changeCount ?? 0} files)`, width, color);
      }
      if (task.mergeStatus) {
        appendField(
          lines,
          "merge",
          `${task.mergeStatus}${task.mergeCommitSha ? ` ${task.mergeCommitSha}` : ""}`,
          width,
          color,
        );
      }
    }
  }

  return buildPanel("RUN BOARD", lines, width, { color });
}

function renderActiveSession(snapshot, activeSessionId, width, color) {
  const session = (snapshot.sessions ?? []).find((entry) => entry.id === activeSessionId) ?? snapshot.sessions?.[0] ?? null;

  if (!session) {
    return buildPanel("ACTIVE SESSION", [colorize("No active session selected.", "gray", color)], width, { color });
  }

  const lines = [
    `${colorize("name", "gray", color)} ${session.name}`,
    `${colorize("role", "gray", color)} ${session.role ?? "worker"}    ${colorize("status", "gray", color)} ${colorize(session.status ?? "unknown", statusColor(session.status), color)}`,
    `${colorize("model", "gray", color)} ${session.model ?? "unset"}`,
  ];
  appendField(lines, "cwd", session.cwd ?? "", width, color, { stacked: true });
  lines.push("─".repeat(Math.max(10, width - 6)));

  const transcript = session.transcript ?? [];
  if (transcript.length === 0) {
    lines.push(colorize("No transcript yet.", "gray", color));
  } else {
    for (const entry of transcript.slice(-6)) {
      const roleColor = entry.role === "assistant" ? "green" : entry.role === "user" ? "cyan" : "gray";
      lines.push(`${colorize(entry.role.padEnd(9), roleColor, color)} ${truncate(entry.text, width - 12)}`);
    }
  }

  return buildPanel("ACTIVE SESSION", lines, width, { color });
}

function renderEventFeed(snapshot, width, color) {
  const eventLines = (snapshot.events ?? [])
    .slice(-8)
    .reverse()
    .map((event) => `• ${summarizeEvent(event)}`);

  const lines = eventLines.length > 0 ? eventLines : [colorize("No supervisor events yet.", "gray", color)];
  return buildPanel("EVENT FEED", lines, width, { color });
}

function renderFooter(snapshot, options, width, color) {
  const shareState = snapshot.share?.state ?? "idle";
  const lines = [
    `${colorize("status", "gray", color)} ${options.statusLine ?? "Ready."}`,
    `${colorize("tunnel", "gray", color)} ${colorize(shareState, statusColor(shareState), color)}`,
  ];
  appendField(lines, "url", snapshot.share?.publicUrl ?? "not shared", width, color, { stacked: true });
  appendField(
    lines,
    "commands",
    "/agent  /use  /run  /plan  /dispatch  /merge  /merge-task  /share start|stop  /quit",
    width,
    color,
    { stacked: true },
  );
  return buildPanel("COMMAND DECK", lines, width, { color });
}

export function renderDashboard(snapshot, options = {}) {
  const color = Boolean(options.color);
  const width = Math.max(80, Math.min(Number(options.width ?? 120), 160));
  const leftWidth = width >= 120 ? 42 : Math.floor(width * 0.4);
  const rightWidth = width - leftWidth - 1;

  const sessionPanel = renderSessionList(snapshot, options.activeSessionId ?? null, leftWidth, color);
  const runPanel = renderRunBoard(snapshot, options.activeRunId ?? null, leftWidth, color);
  const activePanel = renderActiveSession(snapshot, options.activeSessionId ?? null, rightWidth, color);
  const eventPanel = renderEventFeed(snapshot, rightWidth, color);

  const rows = [
    ...renderBanner(width, color),
    ...combineColumns(sessionPanel, activePanel),
    ...combineColumns(runPanel, eventPanel),
    ...renderFooter(snapshot, options, width, color),
  ];

  return rows.join("\n");
}
