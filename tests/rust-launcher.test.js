import test from "node:test";
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import path from "node:path";

const execFileAsync = promisify(execFile);

test("Rust TUI launcher script boots the one-shot dashboard", async () => {
  const scriptPath = path.join(process.cwd(), "scripts", "run-rust-tui.sh");
  const { stdout } = await execFileAsync("sh", [scriptPath, "--once"], {
    cwd: process.cwd(),
  });

  assert.match(stdout, /VORKER CONTROL PLANE/);
  assert.match(stdout, /LAUNCH RAIL/);
});
