# Vorker

Vorker is a local-first CLI coding harness. The default user path is now a Rust terminal shell inspired by Codex and Claude Code: one transcript, one composer, slash commands, file mentions, review mode, Codex-backed side agents, and project-scoped state under `~/.vorker`.

The older web/mobile control plane and JavaScript supervisor still exist, but the main demo path is:

```bash
vorker
```

## Install / Link

From this repo:

```bash
npm link
```

That exposes `vorker` on your PATH through the Node wrapper, which forwards shell/review/RALPH commands to the Rust CLI.

If Rust is not already available in your shell:

```bash
source "$HOME/.cargo/env"
```

## Core Commands

```bash
vorker
vorker tui
vorker tui --once
vorker ralph --no-deslop --xhigh "continue implementing the plan"
vorker adversarial --coach --scope all-files "review this repo"
vorker demo hyperloop
vorker serve
vorker share
```

By default, `vorker` runs inline and preserves terminal scrollback. Use `--alt-screen` if you explicitly want alternate-screen mode.

```bash
vorker --alt-screen
```

## Shell Commands

Inside `vorker`, type `/` to open the command list. Current high-value commands:

```text
/model                 choose the active model
/new                   start a fresh thread
/review                open adversarial review in a review shell
/coach                 rerun review with coaching guidance
/apply                 rerun review and ask Codex to apply a safe patch
/ralph                 launch an OMX RALPH persistence session
/agent                 spawn a Codex-backed side agent
/agents                list stored side-agent jobs
/agent-result <id>     show side-agent result and compact event summary
/agent-stop <id>       stop or mark a side-agent job stopped
/queue <prompt>        queue follow-up work
/steer <guidance>      send steering guidance
/stop                  interrupt active work and side agents
/theme list            list available themes
/theme <name>          switch theme
/status                show model/cwd/workspace/thread/agent status
/export                export current transcript as markdown
/permissions           toggle manual vs auto approvals
/rename <name>         rename current thread
/list                  list saved threads
/list <thread-id>      switch to a saved thread
/history               show recent prompts
/cd <path>             switch project directory
```

Aliases:

- `/clean` is an alias for `/stop`
- `/approvals` is an alias for `/permissions`
- `/?` is an alias for `/help`

When the composer is not in slash mode, `Up` / `Down` recall recent prompts from project history.

## File Mentions

Use `@` in the composer to search workspace files:

```text
› Improve docs in @README.md
```

Selected mentions are resolved into attached file context before the prompt is sent. Binary files are rejected inline instead of silently expanded.

## RALPH

RALPH is an OhMyCodex persistence loop for long-running completion with verification.

Use Vorker’s wrapper instead of raw `omx ralph`:

```bash
vorker ralph --no-deslop --xhigh "finish implementing docs/opencode-ralph-codex-integration-plan.md"
```

Why: project-scope OMX stores config in `./.codex`, but auth usually lives in `~/.codex/auth.json`. Vorker resolves this safely:

- use project `.codex/auth.json` if present
- otherwise use `~/.codex/auth.json`
- never copy auth secrets into the repo
- default to `--no-alt-screen` so the transcript remains observable

Dry-run the exact launch:

```bash
vorker ralph --dry-run --no-deslop --xhigh --model gpt-5.4 "smoke test"
```

## Adversarial Review

Use `/review` in the shell or run:

```bash
vorker adversarial --scope all-files --coach "review the API shape"
vorker adversarial --scope staged --coach --apply "patch the worst issue"
```

Useful flags:

- `--scope auto`
- `--scope working-tree`
- `--scope staged`
- `--scope all-files`
- `--scope branch --base <ref>`
- `--coach`
- `--apply`
- `--popout`

Reports are written under the project workspace in `~/.vorker/projects/<project-key>/reports/`.

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

Inspect results:

```text
/agents
/agent-result <agent-id>
/agent-stop <agent-id>
```

## Transcript Export

Inside the shell:

```text
/export
```

Exports go to:

```text
~/.vorker/projects/<project-key>/exports/
```

The exporter currently renders the visible thread rows. Future work should render from the normalized event log once that schema lands.

## Prompt History

Vorker records submitted prompts per project:

```text
~/.vorker/projects/<project-key>/prompt-history.jsonl
```

Use:

```text
/history
```

or press `Up` / `Down` in an empty normal composer to recall recent prompts.

## Project State

Vorker asks for confirmation the first time it runs in a directory. Project state is scoped by canonical cwd:

```text
~/.vorker/projects/<project-key>/meta.json
~/.vorker/projects/<project-key>/threads.json
~/.vorker/projects/<project-key>/side-agents.json
~/.vorker/projects/<project-key>/reports/
~/.vorker/projects/<project-key>/exports/
```

Override the state root:

```bash
VORKER_HOME=/tmp/vorker-home vorker
```

## Web / Share Mode

The local web control plane still exists:

```bash
VORKER_PASSWORD=your-password vorker serve
VORKER_PASSWORD=your-password vorker share
```

`vorker share` uses Cloudflare Quick Tunnel and defaults to long-polling for better tunnel compatibility.

## Verification

Run the focused checks used during development:

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

OhMyCodex health:

```bash
omx doctor
```

## Active Roadmap

The detailed source-backed roadmap lives at:

```text
docs/opencode-ralph-codex-integration-plan.md
```

Next major lanes:

- command registry and richer autocomplete
- durable prompt history and stash
- transcript pager and event-backed export
- plan mode with approval-gated execution
- permission/question docks
- LSP/MCP status and tools
- undo/fork/resume/compact
- OMX-style HUD/trace
- team runtime and worker worktrees

## Safety Notes

- Do not commit `auth.json` or other provider secrets.
- RALPH uses the authenticated user Codex home when project auth is absent.
- Vorker side agents are still local processes; use `/stop` and `/agent-stop <id>` if they run too long.
- The current implementation is intentionally local-first. Expose web/share mode only with a real password and trusted tunnel/TLS setup.
