import { TITLE_ART, colorize, emphasize, fit } from "./theme.js";

const SPINNER = ["|", "/", "-", "\\"];

function statusColor(status) {
  switch (status) {
    case "ready":
      return "brightGreen";
    case "loading":
      return "yellow";
    case "error":
      return "red";
    default:
      return "gray";
  }
}

function meter(status, tick, color) {
  if (status === "ready") {
    return colorize("■■■■", "brightGreen", color);
  }
  if (status === "loading") {
    const fill = (tick % 4) + 1;
    return `${colorize("■".repeat(fill), "green", color)}${colorize("□".repeat(4 - fill), "brightBlack", color)}`;
  }
  if (status === "error") {
    return colorize("■■■■", "red", color);
  }
  return colorize("□□□□", "brightBlack", color);
}

export function renderBootFrame(options = {}) {
  const width = Math.max(72, Math.min(Number(options.width ?? 100), 120));
  const tick = Number(options.tick ?? 0);
  const color = Boolean(options.color);
  const activeStepId = options.activeStepId ?? null;
  const steps = options.steps ?? [];

  const lines = [
    ...TITLE_ART.map((line) => colorize(line, "brightGreen", color)),
    emphasize(colorize("VORKER CONTROL PLANE // VORKER-2 supervisor mesh", "green", color), color),
    colorize("Arrow-led operator shell. Booting agent lanes and replaying the supervisor bus.", "gray", color),
    "─".repeat(Math.max(40, width)),
  ];

  for (const step of steps) {
    const liveStatus = step.id === activeStepId ? "loading" : step.status ?? "pending";
    const spinner = liveStatus === "loading" ? ` ${SPINNER[tick % SPINNER.length]}` : "";
    const statusLabel = colorize(`${liveStatus.toUpperCase()}${spinner}`, statusColor(liveStatus), color);
    const line = `${meter(liveStatus, tick, color)} ${step.label.padEnd(14)} ${step.detail} ${statusLabel}`;
    lines.push(fit(line, width));
  }

  return lines.join("\n");
}
