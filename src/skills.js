import { promises as fs } from "node:fs";
import path from "node:path";
import process from "node:process";

function parseFrontmatter(source) {
  const match = source.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?/);
  if (!match) {
    return {};
  }

  const data = {};
  for (const line of match[1].split(/\r?\n/)) {
    const parsed = line.match(/^([A-Za-z0-9_-]+):\s*(.+)$/);
    if (!parsed) {
      continue;
    }

    const [, key, value] = parsed;
    data[key] = value.replace(/^["']|["']$/g, "").trim();
  }

  return data;
}

async function pathExists(targetPath) {
  try {
    await fs.access(targetPath);
    return true;
  } catch {
    return false;
  }
}

function uniquePaths(values) {
  const resolved = [];
  const seen = new Set();

  for (const value of values) {
    if (!value) {
      continue;
    }
    const normalized = path.resolve(value);
    if (seen.has(normalized)) {
      continue;
    }
    seen.add(normalized);
    resolved.push(normalized);
  }

  return resolved;
}

async function listDirectories(root) {
  try {
    const entries = await fs.readdir(root, { withFileTypes: true });
    return entries.filter((entry) => entry.isDirectory()).map((entry) => path.join(root, entry.name));
  } catch {
    return [];
  }
}

export class SkillCatalog {
  constructor(options = {}) {
    this.cwd = path.resolve(options.cwd ?? process.cwd());
    this.workspaceRoots = uniquePaths([this.cwd, ...(options.workspaceRoots ?? [])]);
    this.extraSkillRoots = uniquePaths(options.extraSkillRoots ?? []);
    this.skills = new Map();
    this.lastRefreshedAt = null;
  }

  setWorkspaceRoots(roots) {
    this.workspaceRoots = uniquePaths([this.cwd, ...(roots ?? [])]);
  }

  addWorkspaceRoot(rootPath) {
    this.workspaceRoots = uniquePaths([...this.workspaceRoots, rootPath]);
  }

  async refresh(extraRoots = []) {
    const discovered = new Map();
    const rootsToScan = uniquePaths([
      ...this.workspaceRoots.flatMap((workspace) => [
        path.join(workspace, ".agents", "skills"),
        path.join(workspace, ".github", "skills"),
      ]),
      ...(process.env.CODEX_HOME ? [path.join(process.env.CODEX_HOME, "skills")] : []),
      ...this.extraSkillRoots,
      ...extraRoots,
    ]);

    for (const root of rootsToScan) {
      if (!(await pathExists(root))) {
        continue;
      }

      const directories = await listDirectories(root);
      for (const skillDir of directories) {
        const skillFile = path.join(skillDir, "SKILL.md");
        if (!(await pathExists(skillFile))) {
          continue;
        }

        const content = await fs.readFile(skillFile, "utf8");
        const frontmatter = parseFrontmatter(content);
        const name = frontmatter.name || path.basename(skillDir);
        const description = frontmatter.description || "Skill";
        const workspaceRoot = this.workspaceRoots.find((workspace) => skillFile.startsWith(workspace)) ?? null;
        const relativePath = workspaceRoot ? path.relative(workspaceRoot, skillFile) : path.relative(root, skillFile);

        discovered.set(skillFile, {
          id: skillFile,
          name,
          description,
          path: skillFile,
          relativePath,
          sourceRoot: root,
          workspaceRoot,
          updatedAt: (await fs.stat(skillFile)).mtime.toISOString(),
        });
      }
    }

    this.skills = discovered;
    this.lastRefreshedAt = new Date().toISOString();
    return this.listSkills();
  }

  listSkills() {
    return Array.from(this.skills.values()).sort((left, right) =>
      left.name.localeCompare(right.name, undefined, { sensitivity: "base" }),
    );
  }

  getSkill(skillId) {
    return this.skills.get(skillId) ?? null;
  }

  async getSkillSnippets(skillIds, options = {}) {
    const maxPerSkillBytes = options.maxPerSkillBytes ?? 6000;
    const maxTotalBytes = options.maxTotalBytes ?? 18000;
    const snippets = [];
    let totalBytes = 0;

    for (const skillId of skillIds ?? []) {
      const skill = this.getSkill(skillId);
      if (!skill) {
        continue;
      }

      const source = await fs.readFile(skill.path, "utf8");
      let content = source;
      let truncated = false;

      const skillBytes = Buffer.byteLength(content, "utf8");
      if (skillBytes > maxPerSkillBytes) {
        let bytes = 0;
        const chunks = [];
        for (const char of content) {
          const charBytes = Buffer.byteLength(char, "utf8");
          if (bytes + charBytes > maxPerSkillBytes) {
            truncated = true;
            break;
          }
          chunks.push(char);
          bytes += charBytes;
        }
        content = chunks.join("");
      }

      const contentBytes = Buffer.byteLength(content, "utf8");
      if (totalBytes + contentBytes > maxTotalBytes) {
        break;
      }

      totalBytes += contentBytes;
      snippets.push({
        id: skill.id,
        name: skill.name,
        path: skill.relativePath || skill.path,
        content,
        truncated,
      });
    }

    return snippets;
  }

  snapshot() {
    return {
      lastRefreshedAt: this.lastRefreshedAt,
      skills: this.listSkills(),
    };
  }
}
