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

## Next Feature Backlog

### 1. Codex Agent Jobs

- Track `/agent` jobs instead of fire-and-forget spawning.
- Store job id, prompt, cwd, model, status, started/finished times, stdout events, final output, and report path.
- Add `/agents` to list active/recent jobs.
- Add `/agent-result <id>` to show final output.
- Add `/agent-stop <id>` to kill a specific job.
- Use Codex `exec --json` events.

### 2. Queue / Steer UX

- If user presses Enter while work is active, show a small “queue or steer?” prompt instead of silently queueing forever.
- `/steer` should be visible and should annotate the transcript.
- `/queue` should show queue length and next item.
- `/stop` should cancel both Copilot ACP and Vorker-managed subprocess jobs.

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

### 5. RALPH-Style Completion Loop

- Study OhMyCodex RALPH state and verifier loop.
- Add Vorker-local `vorker ralph <task>` or `/ralph <task>`.
- Persist loop state under the project workspace.
- Add verifier step before completion claims.
- Integrate with `/agent` jobs and `/review`.

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
