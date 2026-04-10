# Codex Agent Prompt: Rebuild Vorker-2 TUI

You are working inside the `vorker-2` repository.

Read this PRD first and use it as the source of truth:

- `docs/codex-style-tui-rebuild-prd.md`

Then inspect the current implementation, especially:

- `crates/vorker-tui/src/app.rs`
- `crates/vorker-tui/src/render.rs`
- `src/tui.js`
- `src/tui/render.js`
- `src/tui/commands.js`
- `src/copilot.js`

Your task:

1. Rebuild the terminal UX so it behaves like Codex or Claude Code, not like a dashboard.
2. Make the transcript the main surface.
3. Make the bottom composer the default focus.
4. Add slash commands that open below the composer and filter live as the user types.
5. Move orchestration surfaces into secondary navigation or overlays.
6. Do not fake a native Codex backend if the abstraction does not exist yet. If needed, create the provider abstraction first.

Constraints:

- preserve existing working features where possible
- prefer the Rust TUI as the long-term primary path
- do not ship a visual clone of OpenAI branding
- keep behavior aligned with the PRD
- verify any changes with relevant tests or checks before declaring success

Expected output:

- the code changes
- a short summary of what changed
- any architectural compromises still left
- commands run for verification
