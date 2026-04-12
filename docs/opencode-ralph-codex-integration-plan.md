# Vorker OpenCode / RALPH / Codex Integration Plan

> Working rule: do not stop at MVP if the next improvement is clear. After every implementation pass, ask whether the output can be more controllable, more readable, more reliable, or more demoable. If yes, keep improving and commit the checkpoint.

## Goal

Turn Vorker from a Copilot ACP wrapper into a serious local CLI agent harness:

- shell-first coding flow
- Codex-backed side agents
- controllable long-running work
- adversarial review with coaching and patching
- rich terminal rendering for code blocks and diffs
- project-scoped persistent state under `~/.vorker`
- themeable UI inspired by opencode’s semantic token system
- future RALPH-style completion/verification loop

## Reference Sources

- `opencode`: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode`
- `oh-my-codex`: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/oh-my-codex`
- OpenAI Codex repo: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/codex-upstream`
- `claw-code`: unavailable at clone time because GitHub returned 403 / repo disabled
- `mini-claw-code`: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/mini-claw-code`

## RALPH Status

RALPH is usable, but not yet ergonomic.

Root cause found:

- `omx setup --scope project` stores `.omx/setup-scope.json` with `"scope": "project"`.
- OhMyCodex launch code resolves project scope to `CODEX_HOME=<repo>/.codex`.
- This repo-local `.codex` has prompts/config, but no `auth.json`.
- The authenticated Codex home is `~/.codex`, which contains `auth.json`.
- Therefore plain `omx ralph ...` launches the Codex login TUI instead of starting work.

Working invocation:

```bash
CODEX_HOME="$HOME/.codex" TERM=xterm-256color omx ralph --no-deslop --no-alt-screen "<task>"
```

Verified behavior:

- `CODEX_HOME="$HOME/.codex" omx ralph --no-deslop --version` launches Codex and exits with `codex-cli 0.118.0`.
- A real read-only RALPH prompt got past sign-in, started MCP servers, read this plan, searched the local reference repos, and produced a resumable Codex session: `codex resume 019d7754-ebd4-7a23-8643-64f816eac464`.

Durable Vorker fix:

- Add `vorker ralph <task>` and `/ralph <task>` as first-class Vorker commands.
- The wrapper should preserve project state under `.omx` / `~/.vorker`, but launch Codex with a usable auth home.
- Short-term default: if `<repo>/.codex/auth.json` is missing and `~/.codex/auth.json` exists, set `CODEX_HOME=$HOME/.codex`.
- Longer-term safer default: create a Vorker-managed runtime Codex home under `~/.vorker/codex-home/<project-key>/`, copy non-secret project config/prompts there, and symlink or reference `~/.codex/auth.json` without committing secrets.
- Always pass `--no-alt-screen` by default when Vorker is launching RALPH from its own shell so the transcript remains observable.
- Record the RALPH session id and `codex resume ...` command into the current Vorker thread.

## Already Shipped On This Branch

- Rust shell path as default `vorker`
- project-scoped `~/.vorker/projects/<slug-hash>` state
- startup confirmation
- `/review` adversarial review
- staged and all-files review scopes
- review mode
- `/coach`, `/apply`, `/exit-review`
- rich review highlighting for severity, inline code, file refs, code quotes, and diffs
- `--json` Codex event bridge for review rows
- `/stop`, `/steer`, `/queue`, `/agent`, `/theme`
- `/agent-stop`
- slash commands still execute while a prompt is active, so `/stop` is not accidentally queued as plain text

## Next Feature Backlog

This backlog is now source-backed rather than MVP-scoped. Each lane should be implemented with tests, committed as an independent slice, and re-evaluated before moving to the next lane.

### Immediate Follow-Ups From Active Shell Audit

- [ ] Fix `/skills` navigation so Up / Down move through actions and skills reliably, and `Enter` toggles the selected skill without forcing mouse-like flow.
- [ ] Tighten the shell layout further toward Codex’s cleaner single-surface shell model; keep removing dashboard leftovers and mismatched spacing.
- [ ] Expand the harness so Vorker can spawn and track more concurrent agents cleanly instead of treating extra agents as a thin subprocess add-on.
- [ ] Rework `/review` so it stays in the main terminal session and opens as a dedicated in-shell chat/thread mode instead of relying on external terminal pop-outs.

### 0. RALPH Wrapper and Runtime Bridge

Source references:

- OhMyCodex RALPH launcher: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/oh-my-codex/src/cli/ralph.ts`
- OhMyCodex launch CODEX_HOME resolution: `/opt/homebrew/lib/node_modules/oh-my-codex/dist/cli/index.js`
- Vorker CLI entrypoint: `crates/vorker-cli/src/main.rs`
- Vorker slash registry: `crates/vorker-tui/src/slash.rs`
- Vorker app command loop: `crates/vorker-tui/src/app.rs`

Implementation tasks:

- [ ] Add a `vorker ralph` subcommand in `crates/vorker-cli/src/main.rs`.
- [ ] Add `crates/vorker-cli/src/ralph.rs` to build and run the `omx ralph` command.
- [ ] Add auth-home resolution: prefer repo `.codex/auth.json`; fall back to `~/.codex/auth.json`; never copy secret content into git-tracked paths.
- [ ] Add `--no-alt-screen` by default unless the user explicitly opts into full TUI mode.
- [ ] Add `--no-deslop`, `--model`, `--xhigh`, and passthrough args.
- [ ] Add `/ralph <task>` to `crates/vorker-tui/src/slash.rs`.
- [ ] Add `AppCommand::RunRalph { task, model, no_deslop }` in `crates/vorker-tui/src/app.rs`.
- [ ] Record a transcript row with the exact resumable command after launch.
- [ ] Add tests that verify the generated command sets `CODEX_HOME` correctly when repo auth is missing.
- [ ] Add a smoke test command using `omx ralph --no-deslop --version` so CI does not invoke an LLM.

### 1. Codex Agent Jobs

- Track `/agent` jobs instead of fire-and-forget spawning. Shipped in-memory tracking; persistence remains.
- Store job id, prompt, cwd, model, status, started/finished times, stdout events, final output, and report path. Shipped id/prompt/status/output path in-memory; stdout event capture remains.
- Add `/agents` to list active/recent jobs. Shipped for current shell session.
- Add `/agent-result <id>` to show final output. Shipped for current shell session.
- Add `/agent-stop <id>` to kill a specific job. Shipped for current shell session.
- Use Codex `exec --json` events.
- Persist side-agent jobs under the project workspace, not only in-memory.
- Give side-agent ids monotonic or UUID-based ids instead of second-resolution timestamps.
- Keep stdout/stderr/event logs in `~/.vorker/projects/<project-key>/agents/<agent-id>/`.
- Capture `codex exec --json` events into typed rows: started, tool, command, file edit, text delta, final output, error.
- Add `/agent-resume <id>` to rerun or resume a previous side-agent task from stored prompt/output.
- Add `/agent-tail <id>` or `/agent-log <id>` for a compact log view.
- Add a parent/child thread model so side agents show up as real child sessions, not detached process rows.
- Add navigation similar to opencode and Codex: previous/next agent, parent agent, active agent picker.

Source references:

- Codex multi-agent rendering/navigation: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/codex-upstream/codex-rs/tui/src/multi_agents.rs`
- opencode subagent footer: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/cli/cmd/tui/routes/session/subagent-footer.tsx`
- opencode task tool resumable subagent sessions: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/tool/task.ts`
- mini-claw child-agent tool: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/mini-claw-code/mini-claw-code/src/subagent.rs`

### 2. Queue / Steer UX

- If user presses Enter while work is active, show a small “queue or steer?” prompt instead of silently queueing forever.
- `/steer` should be visible and should annotate the transcript.
- `/queue` should show queue length and next item.
- `/stop` should cancel both Copilot ACP and Vorker-managed subprocess jobs. Shipped for ACP, review jobs, and active side-agent jobs.
- Enter while active should not silently queue by default. It should open a compact choice:
  - `Enter` again: queue after current turn.
  - `/steer <text>`: attach guidance to current work.
  - `/stop`: interrupt current work.
- Implement a bridge-level active prompt handle so `/steer` cannot race as an unrelated prompt unless the backend explicitly only supports a new prompt.
- Add an in-transcript queue row that shows pending prompt count and next prompt preview.
- Add queue reordering and clearing: `/queue list`, `/queue clear`, `/queue pop`.
- Add UI copy matching Codex footer behavior: when busy and draft exists, show a queue hint instead of generic help.

Source references:

- Codex footer queue hint and collapse rules: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/codex-upstream/codex-rs/tui/src/bottom_pane/footer.rs`
- Codex composer state machine: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/codex-upstream/codex-rs/tui/src/bottom_pane/chat_composer.rs`
- Codex status indicator interrupt row: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/codex-upstream/codex-rs/tui/src/status_indicator_widget.rs`

### 3. Theme System

- Replace stringly hard-coded `default` / `review` with semantic theme tokens.
- Tokens:
  - border
  - accent
  - muted
  - composer background
  - markdown heading
  - markdown inline code
  - code block background
  - diff add/remove/context
  - severity critical/high/medium/low
- Add `/theme list`.
- Add `/theme opencode`, `/theme default`, `/theme review`.
- Add live theme preview: moving through the theme list temporarily applies the highlighted theme, then reverts on cancel.
- Add persisted per-project and per-user theme selection.
- Add syntax/diff theme tokens, not just UI chrome tokens.
- Add selected foreground contrast logic so highlighted menu rows are readable across themes.
- Add `diff_style` configuration: unified vs split when terminal width allows.
- Auto-generate a "system" theme from the active terminal palette, then layer custom/plugin themes over it.
- Add theme import from future plugins.

Source references:

- opencode theme dialog with preview/revert: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/cli/cmd/tui/component/dialog-theme-list.tsx`
- opencode dynamic theme generation: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/cli/cmd/tui/context/theme.tsx`
- opencode terminal palette helpers: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/cli/cmd/tui/util/terminal.ts`
- opencode selected foreground theme helper: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/cli/cmd/tui/component/prompt/autocomplete.tsx`
- Codex theme picker and syntax highlighting: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/codex-upstream/codex-rs/tui/src/theme_picker.rs`
- Codex diff renderer: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/codex-upstream/codex-rs/tui/src/diff_render.rs`

### 4. Review Finding Cards

- Group each finding into a compact card:
  - severity badge
  - title
  - file/line
  - why it matters
  - quoted snippet
  - recommendation
  - coaching / patch direction
- Add per-finding “apply” affordance later.
- Add split/unified diff preview before apply.
- Add reject/approve affordances for each proposed patch.
- Add reviewer severity filter: critical/high/all.
- Add "coaching mode" display that separates learning notes from blocking findings.
- Add markdown export and transcript copy for a review session.
- Add true end-to-end tests against a small local bad-code repo using fake `codex exec --json` fixtures.

Source references:

- opencode permission prompt diff renderer: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/cli/cmd/tui/routes/session/permission.tsx`
- Codex diff render snapshots: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/codex-upstream/codex-rs/tui/src/snapshots/`
- Vorker current adversarial output: `crates/vorker-cli/src/adversarial.rs`

### 5. Command System and Autocomplete

Vorker currently has a static slash array. It should move toward a command registry.

Implementation tasks:

- [ ] Add a `SlashCommand` metadata model with name, aliases, description, category, keybind, hidden/enabled predicates, and whether it can run while work is active.
- [ ] Replace positional slash-array indexing with command ids and predicates.
- [ ] Add `/timeline`, `/fork`, `/undo`, `/redo`, `/compact`, `/copy`, `/export`, `/status`, `/mcp`, `/apps`, `/plugins`, `/lsp`, `/thinking`, `/timestamps`, and `/diff`.
- [ ] Add mixed autocomplete for `/` commands, server/provider commands, MCP commands, and future plugins.
- [ ] Add mixed autocomplete for `@` files, `@agent`, and MCP resources.
- [ ] Support `@file#L1-L5` / `@file#10-20` style line ranges.
- [ ] Add frecency scoring to file mentions so recently selected files rise in the list.
- [ ] Add prompt chips/virtual spans so attached files and agents are visibly bound tokens, not plain text that can silently drift.
- [ ] Persist prompt history as structured entries, not just raw text.
- [ ] Add prompt stash/save/restore so long drafts survive mode switches, thread changes, and crashes.
- [ ] Self-heal corrupted prompt history/stash files instead of crashing the shell.

Source references:

- opencode command registry: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/cli/cmd/tui/component/dialog-command.tsx`
- opencode autocomplete: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/cli/cmd/tui/component/prompt/autocomplete.tsx`
- opencode prompt history: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/cli/cmd/tui/component/prompt/history.tsx`
- opencode prompt stash: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/cli/cmd/tui/component/prompt/stash.tsx`
- opencode stash dialog: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/cli/cmd/tui/component/dialog-stash.tsx`
- Codex slash command enum and active-task gating: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/codex-upstream/codex-rs/tui/src/slash_command.rs`
- Codex file popup: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/codex-upstream/codex-rs/tui/src/bottom_pane/file_search_popup.rs`
- Codex app-server fuzzy file search: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/codex-upstream/codex-rs/app-server/src/fuzzy_file_search.rs`

### 6. Transcript, Scrollback, and Markdown Rendering

Current Vorker rendering still behaves like a repainting TUI. The target should feel closer to Codex/opencode: terminal fills naturally, with separate overlays for heavy navigation.

Implementation tasks:

- [ ] Add `--no-alt-screen` as the default for Vorker shell mode unless user config opts into alt-screen.
- [ ] Add a transcript pager overlay for long history and live tail.
- [ ] Add newline-gated markdown streaming so partial markdown does not jitter.
- [ ] Add batch catch-up when stream backlog is large.
- [ ] Add markdown file link rendering relative to cwd.
- [ ] Add syntax-highlighted code blocks and diff blocks using a real highlighter or a small theme-aware parser.
- [ ] Add transcript export/copy with options for tool details, thinking, and metadata.
- [ ] Add snapshot tests for long output, wrapping, markdown lists, blockquotes, code fences, and diff galleries.

Source references:

- Codex markdown stream collector: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/codex-upstream/codex-rs/tui/src/markdown_stream.rs`
- Codex streaming controller: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/codex-upstream/codex-rs/tui/src/streaming/controller.rs`
- Codex pager overlay: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/codex-upstream/codex-rs/tui/src/pager_overlay.rs`
- opencode transcript export/copy command: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/cli/cmd/tui/routes/session/index.tsx`

### 7. Plan Mode and Approval-Gated Execution

- Add a real `/plan` mode that is read-only by default.
- Restrict tools during plan mode to read/list/search/LSP/ask.
- Add `exit_plan` to submit a plan for user approval.
- After approval, synthesize a build-mode user message and allow edits.
- Tie `/ralph` to approved plans so RALPH executes a known plan rather than free-form prompting.
- Persist approved plans under `~/.vorker/projects/<project-key>/plans/`.

Source references:

- mini-claw plan agent: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/mini-claw-code/mini-claw-code/src/planning.rs`
- opencode plan-exit tool: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/tool/plan.ts`
- OhMyCodex RALPH approved handoff support: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/oh-my-codex/src/cli/ralph.ts`

### 8. Permissions, Questions, and Safety

- Add a structured permission dock instead of a plain chooser.
- Show exact tool metadata and diff previews for edit/apply requests.
- Support allow-once, always-allow pattern, reject with reason, and cancel.
- Add a question prompt dock so agents can ask clarifying questions without dumping plain text.
- Add a permissions policy file under the project store: command denylist, allowlist, read roots, write roots, network policy, and "dangerous command" explanations.
- Integrate Codex sandbox / approval modes into visible `/permissions` and `/status`.

Source references:

- opencode permission prompt: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/cli/cmd/tui/routes/session/permission.tsx`
- opencode question tool surface: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/tool/question.ts`
- Codex approvals/sandbox slash commands: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/codex-upstream/codex-rs/tui/src/slash_command.rs`
- Codex Guardian approvals mode: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/codex-upstream/codex-rs/tui/src/app.rs`

### 9. LSP, MCP, Apps, Plugins, and External Context

- Show LSP/MCP connected counts in the footer.
- Add `/mcp` and `/lsp` status dialogs.
- Add LSP tool support: hover, definition, references, document/workspace symbols.
- Add MCP resource mentions into `@` autocomplete.
- Add plugin-style command registration so future Vorker extensions can add commands without editing the core slash list.
- Add an app-server/control-plane bridge only after the shell is stable.
- Add provider registry with explicit auth flows and priority ordering.
- Add plugin compatibility checks before loading plugin commands/themes.
- Add TUI slot/command registration once the shell has stable extension points.

Source references:

- opencode footer LSP/MCP status: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/cli/cmd/tui/routes/session/footer.tsx`
- opencode LSP tool: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/tool/lsp.ts`
- opencode TUI plugin API/runtime: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/cli/cmd/tui/plugin/`
- opencode plugin loader: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/plugin/loader.ts`
- opencode plugin shared contracts: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/plugin/shared.ts`
- opencode plugin installer: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/plugin/install.ts`
- opencode provider dialog: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/cli/cmd/tui/component/dialog-provider.tsx`
- Codex app-server thread state and control plane: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/codex-upstream/codex-rs/app-server/src/thread_state.rs`

### 10. Undo, Fork, Resume, Compact, and Export

- Add `/timeline` with jump-to-message.
- Add `/fork` from a selected message.
- Add `/undo` and `/redo` backed by snapshots/diffs.
- Add `/compact` using the selected provider/model.
- Add `/copy` and `/export` for the full transcript with options.
- Add `/resume` and `/list` integration so old Vorker threads are discoverable across cwd.
- Add "continue with command" hints on exit.
- Add workspace-scoped terminal tabs as a separate lane, not mixed into the main transcript:
  - persist terminal tabs per workspace
  - reconnect PTY/WebSocket with backoff
  - restore buffer, cursor, and scroll state
  - support clone, rename, reorder, and close
  - keep coding-agent shell and embedded terminals conceptually separate

Source references:

- opencode timeline/fork/compact/undo/redo/export command registration: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/cli/cmd/tui/routes/session/index.tsx`
- opencode revert service: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/session/revert.ts`
- opencode workspace terminal context: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/app/src/context/terminal.tsx`
- opencode terminal panel: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/app/src/pages/session/terminal-panel.tsx`
- opencode terminal component: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/app/src/components/terminal.tsx`
- Codex resume picker: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/codex-upstream/codex-rs/tui/src/resume_picker.rs`

### 11. Observability and Harness Evaluation

- Add Vorker-native trace files for every turn, side agent, review, and RALPH run.
- Add a HUD/status overlay inspired by OMX showing active mode, queue length, agent count, iteration, tests, and last verification.
- Add "eval report" generation for interview/demo: what the junior agent did, what the adversarial agent found, what was patched, and what the human learned.
- Add a harness score model: build/test result, review severity count, iteration count, time-to-fix, and residual risk.
- Add Obsidian-compatible markdown export under `/Users/lucas/Desktop/Obsidian/Home/Vorker/` for long-term learning history.
- Add `.omx`-style active-mode status for `review`, `ralph`, `team`, `plan`, `side-agent`, and `preflight`.
- Add a `vorker trace` CLI with timeline and summary views over Vorker event logs.
- Add HUD watch mode or a compact terminal status overlay for long-running RALPH/team work.
- Add explicit mode start/end events so reports can reconstruct execution flow.

Source references:

- OhMyCodex HUD: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/oh-my-codex/src/hud/`
- OhMyCodex HUD render/state/reconcile: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/oh-my-codex/src/hud/render.ts`, `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/oh-my-codex/src/hud/state.ts`, `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/oh-my-codex/src/hud/reconcile.ts`
- OhMyCodex HUD authority tick: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/oh-my-codex/src/hud/authority.ts`
- OhMyCodex trace MCP: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/oh-my-codex/src/mcp/trace-server.ts`
- OhMyCodex MCP parity CLI: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/oh-my-codex/src/cli/mcp-parity.ts`
- OhMyCodex memory/notepad MCP: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/oh-my-codex/src/mcp/memory-server.ts`

### 11.5 OMX Team Runtime

Vorker side agents are currently subprocesses. OMX team mode is a stronger model for multi-agent execution.

Implementation tasks:

- [ ] Add `vorker team <task>` as a wrapper around OMX team mode after `vorker ralph` works.
- [ ] Persist a team state root under the Vorker project workspace.
- [ ] Represent workers with role, model, reasoning, assigned task, worktree path, mailbox, heartbeat, and status.
- [ ] Add `vorker team status`, `vorker team wait`, and `vorker team cleanup` parity commands.
- [ ] Add a TUI status panel showing worker state and last mailbox event.
- [ ] Add worktree integration policy:
  - use isolated worktrees for workers
  - checkpoint dirty worker changes
  - summarize worker diffs
  - rebase/merge only when idle/done
  - surface conflicts instead of hiding them
- [ ] Use role-router/model-contract logic instead of ad hoc side-agent model strings.

Source references:

- OhMyCodex team CLI: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/oh-my-codex/src/cli/team.ts`
- OhMyCodex team runtime: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/oh-my-codex/src/team/runtime.ts`
- OhMyCodex team state: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/oh-my-codex/src/team/state.ts`
- OhMyCodex team API interop: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/oh-my-codex/src/team/api-interop.ts`
- OhMyCodex worker bootstrap: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/oh-my-codex/src/team/worker-bootstrap.ts`
- OhMyCodex role router: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/oh-my-codex/src/team/role-router.ts`
- OhMyCodex model contract: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/oh-my-codex/src/team/model-contract.ts`

### 11.6 Normalized Tool and Session Event Timeline

opencode's transcript is not just text; it models every tool and session transition as structured parts/events.

Implementation tasks:

- [ ] Define a Vorker `SessionEvent` schema covering text, reasoning, tool start, tool delta, tool completion, permission request, question, retry, compaction, diff, snapshot, error, and status.
- [ ] Store events as append-only JSONL per thread.
- [ ] Render from events into transcript rows rather than mutating arbitrary strings.
- [ ] Add a `/status` dialog that summarizes provider, model, MCP, LSP, active tools, pending permissions, retries, and queue state.
- [ ] Use the same event stream for CLI reports and Obsidian exports.

Source references:

- opencode message part schema: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/session/message-v2.ts`
- opencode session status: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/session/status.ts`
- opencode sync events: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/sync/index.ts`
- opencode status dialog: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/opencode/packages/opencode/src/cli/cmd/tui/component/dialog-status.tsx`

### 12. Provenance and Safety Constraints

- Do not depend on leak-derived Claude Code source.
- The previously requested `claw-code` repo was unavailable from the original URL; public search results also surface malware/leak warnings around similarly named repos.
- Use `mini-claw-code` as the safe Rust reference for agent-loop architecture and tests.
- Treat opencode/OMX/Codex source as design references, not code to copy wholesale.
- Keep secrets such as `auth.json` outside git and outside repo-local tracked paths.

## Reference Findings

### opencode ideas worth porting

- semantic theme tokens with markdown and syntax channels
- command palette metadata model
- mixed-source autocomplete
- virtual prompt chips for file/agent references
- compact diff rendering
- markdown transcript export
- dense footer/status chrome

### OhMyCodex ideas worth porting

- `.omx`-style project runtime state
- RALPH loop with persistence and verification
- RALPLAN → team/ralph staged workflow
- team worktrees with state-root coordination
- HUD/status mode indicators
- hook/status/event trail for long-running automation

## Verification Rules

- Every feature must have at least focused tests.
- Run `cargo test -p vorker-tui`.
- Run `cargo test -p vorker-cli`.
- Run `node --test tests/rust-launcher.test.js`.
- For interactive changes, smoke test in `/Users/lucas/Desktop/NationGraph prep/test`.

## Commit Rules

- Commit each coherent slice.
- Keep branch: `codex/opencode-ralph-harness`.
- If the user wants this on `main`, merge only after a clean verification pass.
