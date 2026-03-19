import path from "node:path";
import { mkdir, appendFile, readFile } from "node:fs/promises";

export class EventLog {
  constructor(options = {}) {
    this.rootDir = path.resolve(options.rootDir ?? process.cwd());
    this.filePath = path.resolve(options.filePath ?? path.join(this.rootDir, "events.ndjson"));
  }

  async append(event) {
    await mkdir(path.dirname(this.filePath), { recursive: true });
    await appendFile(this.filePath, `${JSON.stringify(event)}\n`, "utf8");
    return event;
  }

  async readAll() {
    try {
      const raw = await readFile(this.filePath, "utf8");
      return raw
        .split("\n")
        .map((line) => line.trim())
        .filter(Boolean)
        .map((line) => JSON.parse(line));
    } catch (error) {
      if (error && typeof error === "object" && "code" in error && error.code === "ENOENT") {
        return [];
      }
      throw error;
    }
  }
}
