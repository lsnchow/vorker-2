export const TITLE_ART = [
  "██╗   ██╗ ██████╗ ██████╗ ██╗  ██╗███████╗██████╗",
  "██║   ██║██╔═══██╗██╔══██╗██║ ██╔╝██╔════╝██╔══██╗",
  "██║   ██║██║   ██║██████╔╝█████╔╝ █████╗  ██████╔╝",
  "╚██╗ ██╔╝██║   ██║██╔══██╗██╔═██╗ ██╔══╝  ██╔══██╗",
  " ╚████╔╝ ╚██████╔╝██║  ██║██║  ██╗███████╗██║  ██║",
  "  ╚═══╝   ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝╚══════╝╚═╝  ╚═╝",
];

export const ANSI_PATTERN = /\x1b\[[0-9;]*m/g;

export const COLORS = {
  reset: "\x1b[0m",
  bold: "\x1b[1m",
  dim: "\x1b[2m",
  black: "\x1b[30m",
  green: "\x1b[32m",
  yellow: "\x1b[33m",
  red: "\x1b[31m",
  cyan: "\x1b[36m",
  magenta: "\x1b[35m",
  white: "\x1b[97m",
  gray: "\x1b[90m",
  brightGreen: "\x1b[92m",
  brightMagenta: "\x1b[95m",
  brightBlack: "\x1b[90m",
  bgGreen: "\x1b[42m",
  bgBrightGreen: "\x1b[102m",
  bgMagenta: "\x1b[45m",
  bgBrightMagenta: "\x1b[105m",
};

export function stripAnsi(text) {
  return String(text ?? "").replace(ANSI_PATTERN, "");
}

export function visibleLength(text) {
  return stripAnsi(text).length;
}

export function paint(text, styles, enabled) {
  if (!enabled || !styles || styles.length === 0) {
    return text;
  }

  const prefix = styles.map((style) => COLORS[style] ?? "").join("");
  return `${prefix}${text}${COLORS.reset}`;
}

export function colorize(text, color, enabled) {
  return paint(text, [color], enabled);
}

export function emphasize(text, enabled) {
  return paint(text, ["bold"], enabled);
}

export function highlight(text, enabled, options = {}) {
  if (!enabled) {
    return text;
  }

  return paint(text, ["bold", options.foreground ?? "black", options.background ?? "bgBrightGreen"], enabled);
}

export function truncate(text, maxLength) {
  const value = String(text ?? "");
  if (visibleLength(value) <= maxLength) {
    return value;
  }
  return `${stripAnsi(value).slice(0, Math.max(0, maxLength - 1))}…`;
}

export function pad(text, width) {
  const value = String(text ?? "");
  const missing = Math.max(0, width - visibleLength(value));
  return `${value}${" ".repeat(missing)}`;
}

export function fit(text, width) {
  return pad(truncate(text, width), width);
}

export function hardWrap(text, maxLength) {
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
