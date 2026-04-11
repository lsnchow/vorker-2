# Vorker

Vorker is a local-first CLI coding harness for running AI-assisted development with stronger review, control, and project memory.

It started as a Copilot ACP wrapper, but the current demo path is a Rust terminal shell inspired by Codex and Claude Code: a single chat transcript, slash commands, file mentions, adversarial review, project-scoped skills, and Codex-backed side agents.

```bash
vorker
```

## Why It Exists

LLM coding tools can move quickly, but junior developers still need guardrails: clearer workflows, better review loops, and a way to learn from the mistakes an agent makes. Vorker experiments with that harness layer.

Core ideas:

- **Agent shell:** a simple terminal interface for coding prompts, `/` commands, and file context.
- **Adversarial review:** a second-pass reviewer that can critique, coach, and propose patches.
- **Skills:** project-scoped `SKILL.md` instructions that steer the underlying agent.
- **Side agents:** Codex-backed background workers for parallel investigation.
- **Local-first state:** threads, prompt history, reports, exports, and agent logs live under `~/.vorker`.
- **Remote demo mode:** optional Tailscale / tunnel support for sharing a local session.

## Status

This is an active prototype, not a polished production agent platform. The core shell path works, but provider behavior still depends on the local Copilot/Codex setup and the repo is changing quickly.

The repository currently used for development is:

```text
https://github.com/lsnchow/vorker-2
```

## Quick Start

From this repo:

```bash
npm link
vorker
```

If Rust is not already available in your shell:

```bash
source "$HOME/.cargo/env"
```

By default, `vorker` runs inline and preserves terminal scrollback. Use alternate-screen mode only when you explicitly want it:

```bash
vorker --alt-screen
```

## CLI Commands

```bash
vorker
vorker tui
vorker tui --once
vorker adversarial --coach --scope all-files "review this repo"
vorker ralph --no-deslop --xhigh "continue implementing the plan"
vorker demo hyperloop
vorker serve
vorker tailnet
vorker share
```

## Shell Commands

Inside the shell, type `/` to open the command list.

```text
/model                 choose the active model
/new                   start a fresh thread
/review                open adversarial review
/coach                 rerun review with teaching guidance
/apply                 rerun review and request a safe patch
/skills                list or toggle project skills
/agent                 spawn a Codex-backed side agent
/agents                list side-agent jobs
/agent-result <id>     show side-agent output
/agent-stop <id>       stop a side-agent job
/queue <prompt>        queue follow-up work
/queue list            show queued follow-up prompts
/queue pop             remove the next queued follow-up prompt
/queue clear           clear queued follow-up prompts
/steer <guidance>      send steering guidance
/stop                  interrupt active work and side agents
/theme <name>          switch theme
/status                show shell/session status
/export                export the current transcript
/copy                  copy the current transcript to the clipboard
/compact               compact the current transcript into a short summary
/permissions           toggle manual vs auto approvals
/rename <name>         rename the current thread
/list                  list saved threads
/list <thread-id>      switch to a saved thread
/timeline              show a compact timeline of the current thread
/history               show recent prompts
/cd <path>             switch project directory
```

Aliases:

- `/clean` -> `/stop`
- `/approvals` -> `/permissions`
- `/?` -> `/help`

When the composer is not in slash mode, `Up` / `Down` recall recent prompts from project history.

If work is already running and you press `Enter` on a draft prompt, Vorker now opens a compact choice to either queue the text for later or send it as steering guidance to the active turn. Use `/stop` to interrupt active work.

## File Mentions

Use `@` in the composer to attach workspace files:

```text
› Improve the README in @README.md
› Review @src/main.rs#L10-L40
```

Selected mentions are resolved into file context before the prompt is sent. Binary files are rejected inline instead of silently expanded.

Line ranges are supported with `#Lstart-Lend`, `#start-end`, `#Lline`, or `#line`.

## Skills

Vorker discovers Codex-style `SKILL.md` files from:

```text
<project>/.codex/skills/
<project>/.agents/skills/
<project>/.github/skills/
~/.codex/skills/
~/.codex/superpowers/skills/
~/.agents/skills/
```

Use `/skills` to open the Codex-style skills menu, or run:

```text
/skills list
/skills enable code-review
/skills disable code-review
```

Enabled skills are stored per project and prepended to the Copilot ACP prompt with Vorker's personality harness, so the underlying agent is steered to behave as Vorker rather than introducing itself as GitHub Copilot.

## Adversarial Review

From the shell:

```text
/review --coach --no-popout review the API shape
/apply
```

From the CLI:

```bash
vorker adversarial --scope all-files --coach "review the API shape"
vorker adversarial --scope staged --coach --apply "patch the highest-risk issue"
```

Useful scopes:

- `--scope auto`
- `--scope working-tree`
- `--scope staged`
- `--scope all-files`
- `--scope branch --base <ref>`

Reports are written under:

```text
~/.vorker/projects/<project-key>/reports/
```

## Side Agents

Spawn a Codex side agent:

```text
/agent inspect the auth boundary
```

Vorker stores side-agent metadata and logs under:

```text
~/.vorker/projects/<project-key>/side-agents.json
~/.vorker/projects/<project-key>/side-agents/<agent-id>/
```

Each side agent gets:

- `last-message.md`
- `stderr.log`
- `events.jsonl`

## Project State

Vorker asks for confirmation the first time it runs in a directory. Project state is scoped by canonical cwd:

```text
~/.vorker/projects/<project-key>/meta.json
~/.vorker/projects/<project-key>/threads.json
~/.vorker/projects/<project-key>/prompt-history.jsonl
~/.vorker/projects/<project-key>/skills.json
~/.vorker/projects/<project-key>/side-agents.json
~/.vorker/projects/<project-key>/reports/
~/.vorker/projects/<project-key>/exports/
```

Override the state root:

```bash
VORKER_HOME=/tmp/vorker-home vorker
```

## RALPH

Vorker can launch OhMyCodex RALPH sessions for long-running work:

```bash
vorker ralph --no-deslop --xhigh "finish implementing docs/opencode-ralph-codex-integration-plan.md"
```

Dry-run the exact launch:

```bash
vorker ralph --dry-run --no-deslop --xhigh --model gpt-5.4 "smoke test"
```

Vorker uses project `.codex/auth.json` when present and otherwise falls back to `~/.codex/auth.json`. It does not copy auth secrets into the repo.

## Web / Remote Access

The local web control plane still exists:

```bash
VORKER_PASSWORD=your-password vorker serve
VORKER_PASSWORD=your-password vorker tailnet
VORKER_PASSWORD=your-password vorker share
```

`vorker tailnet` uses Tailscale Serve for private tailnet access. Public Funnel is opt-in:

```bash
VORKER_PASSWORD=your-password vorker tailnet --funnel
```

Preview what Vorker will run:

```bash
vorker tailnet --dry-run --port 4173
```

## Development

Focused verification:

```bash
source "$HOME/.cargo/env"
cargo test -p vorker-tui
cargo test -p vorker-cli
node --test tests/rust-launcher.test.js
```

Full workspace:

```bash
source "$HOME/.cargo/env"
cargo fmt --all
cargo test --workspace
```

The detailed roadmap lives at:

```text
docs/opencode-ralph-codex-integration-plan.md
```

## Safety Notes

- Do not commit `auth.json` or other provider secrets.
- Vorker side agents are local processes; use `/stop` or `/agent-stop <id>` if they run too long.
- The current implementation is intentionally local-first. Use web/share mode only with a real password and a trusted tunnel/TLS setup.
