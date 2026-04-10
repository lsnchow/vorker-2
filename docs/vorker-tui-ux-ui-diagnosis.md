# Vorker TUI UX/UI Diagnosis

Date: `2026-03-21`  
Repo: `vorker-2`  
Reference baseline: OpenAI Codex TUI architecture notes from `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/openai-codex-tui-notes.md`

## Scope

This diagnosis is based on:

- reading the current Rust TUI implementation in `crates/vorker-tui`
- reading the Rust CLI entrypoint in `crates/vorker-cli`
- reading the approved Rust rewrite spec and phase-1 plan
- running `npm run tui:once`
- running the interactive TUI through `cargo run -p vorker-cli -- --no-alt-screen tui`

This document covers both:

- what is wrong now
- what the redesign should do next

## Executive Summary

The current Rust TUI is not really a TUI in the architectural sense. It is a synchronous ASCII dashboard renderer that clears and redraws the full terminal on every keypress.

That single decision explains most of the visible problems:

- the UX feels fake instead of operational
- the UI has almost no real focus hierarchy
- the prompt area feels like editing a report, not driving a console
- alignment breaks under real terminal widths
- the app looks over-branded and under-instrumented

The repo's own rewrite spec expected a first-class operator interface built with `ratatui` and `crossterm`, with the TUI acting as a client of the runtime state. The actual implementation is much thinner than that and mostly mutates local snapshot state directly.

In short: the current shell is a useful spike, but it is not a credible production operator console yet.

## The Main Structural Problem

The most important finding is that the current implementation is a string renderer, not a widget-based terminal application.

Evidence:

- the design spec explicitly recommends `ratatui` + `crossterm` in `docs/superpowers/specs/2026-03-20-rust-runtime-rewrite-design.md:169-183`
- the implementation plan also calls for `ratatui`, `crossterm`, and `tokio` in `docs/superpowers/plans/2026-03-20-rust-runtime-phase-1.md:5-10`
- `crates/vorker-tui/Cargo.toml` currently depends on `crossterm`, `serde_json`, and `vorker-core`, but not `ratatui` or `tokio`
- `crates/vorker-tui/src/app.rs:293-321` runs a blocking loop that:
  - reads terminal size
  - clears the whole screen
  - writes a giant rendered string
  - waits for one key event
- `crates/vorker-tui/src/render.rs:69-101` renders the whole dashboard as concatenated strings

This matters because a real TUI usually has:

- a terminal abstraction
- layout primitives
- focused widgets
- incremental redraws
- async event handling
- input components with cursor semantics
- scrollable transcript views
- overlays and modals that behave like overlays and modals

The current implementation has almost none of that. It has a good-enough string snapshot of a dashboard, but not an interaction model that can grow into a serious ACP operator UI.

## What I Saw In Live Runs

Two live issues stand out immediately.

### 1. The banner wastes space and wraps badly

At a real terminal width, the top copy wraps like this:

```text
VORKER CONTROL PLANE   VORKER-2 supervisor mesh   agents left / launch rail
top / swarm pink
```

This is not a small polish issue. It tells the operator that the screen is layout-fragile before they have even used it.

The cause is the manual wrap path in `crates/vorker-tui/src/render.rs:104-129` and `crates/vorker-tui/src/render.rs:584-590`.

### 2. The panel internals visibly misalign

After creating an agent, the detail panel renders this:

```text
|  cwd                                                                     |
|    .                                                                     |
|----------------------------------------------------------------------    |
|No transcript yet.                                                        |
```

That separator line with trailing spaces is exactly the kind of thing users describe as "horrible and misaligned." It looks broken because it is broken.

The immediate cause is the combination of:

- manual separator strings in `crates/vorker-tui/src/render.rs:410`
- `fit()` padding in `crates/vorker-tui/src/theme.rs:67-69`
- panel framing in `crates/vorker-tui/src/render.rs:532-559`

### 3. Typing a prompt redraws the entire dashboard on every character

In the live session, entering `hello from tui` produced a full-screen repaint for every character. The terminal output literally becomes a sequence of complete screen clears plus full re-renders.

That behavior comes directly from `crates/vorker-tui/src/app.rs:302-314`.

This is the biggest UX smell in the app. It makes the console feel like a crude terminal slideshow instead of a responsive operator surface.

## UX Diagnosis

### 1. The system does not feel operational

The TUI behaves like a mock control plane rather than a real runtime shell.

Why:

- `create_agent()` just appends an in-memory `session.registered` event in `crates/vorker-tui/src/app.rs:160-193`
- `launch_swarm()` fabricates a run plus two tasks in `crates/vorker-tui/src/app.rs:195-250`
- `send_prompt()` appends fake prompt events and synthesizes `Acknowledged: ...` in `crates/vorker-tui/src/app.rs:252-285`

That is fine for a spike. It is bad for operator trust. The moment the user sends a prompt and gets a fake assistant echo, the entire product promise gets weaker.

For an ACP-first tool, the TUI has to make the user feel they are operating real local Copilot sessions. Right now it feels like they are editing sample data.

### 2. The app does not answer the two core operator questions

Every serious terminal UI needs to answer:

- Where am I?
- What can I do next?

The current screen answers both poorly.

Problems:

- focus is tracked in navigation state, but barely visible on screen
- the footer says `focus actions`, `focus sessions`, etc., but the actual pane does not strongly reflect that state
- the active input mode switches between prompt and swarm-goal, but the distinction lives mostly in footer copy
- the model picker is technically a mode, but visually it is just an extra line under the launch rail

The state machine in `crates/vorker-tui/src/navigation.rs:34-233` is not the problem. The problem is that the render layer does not make that state obvious.

### 3. The interaction model is more confusing than it should be

The spec says the interface should feel immediately understandable without memorizing slash commands in `docs/superpowers/specs/2026-03-20-rust-runtime-rewrite-design.md:57-69`.

The current build does not meet that bar.

Reasons:

- left/right sometimes changes action selection and sometimes changes pane focus
- down from actions moves to sessions, but the footer is the actual place where prompts are typed
- the prompt is not a visibly focused input control
- the event feed is always present, but the input area is visually buried
- the model picker does not feel modal even though the system treats it as modal

This is not a navigation bug. It is a discoverability bug.

### 4. The copy prioritizes theme over clarity

Terms like these weaken the UX:

- `launch rail`
- `command deck`
- `pink lane`
- `swarm pink`

They are flavorful, but not useful enough. In an operator UI, labels should reduce cognitive load, not add brand voice.

Good terminal labels are blunt:

- Actions
- Agents
- Runs
- Tasks
- Activity
- Input
- Status

The current naming makes the UI feel more like a concept demo than a working console.

### 5. The information hierarchy is upside down

The screen spends too much space on:

- ASCII branding
- instructional copy
- decorative panel chrome

And not enough on:

- active session status
- ACP connection state
- permission requests
- currently running task/worktree
- prompt/input state
- last meaningful agent output

For a Copilot ACP supervisor, the screen should be built around operational feedback, not branding.

## UI Diagnosis

### 1. The whole interface is visually flat

The theme helpers are effectively stubs:

- `colorize()` returns raw text in `crates/vorker-tui/src/theme.rs:9-11`
- `emphasize()` returns raw text in `crates/vorker-tui/src/theme.rs:13-15`
- `highlight()` returns raw text in `crates/vorker-tui/src/theme.rs:17-19`

That means:

- selected chips are not actually highlighted
- focused panel headers are not visually distinct
- swarm state has no real pink accent
- the entire interface collapses into one gray density level

The navigation model exists. The visual affordances do not.

### 2. The app uses border-heavy ASCII composition instead of layout

The visible UI is assembled by:

- `build_panel()` in `crates/vorker-tui/src/render.rs:532-559`
- `combine_columns()` in `crates/vorker-tui/src/render.rs:561-582`
- `append_field()` in `crates/vorker-tui/src/render.rs:592-608`
- `fit()` and `hard_wrap()` in `crates/vorker-tui/src/theme.rs:67-98`

This design has predictable side effects:

- awkward wrapping
- ragged spacing
- brittle separators
- equal-looking panels regardless of importance
- hard-to-read transcript rows
- no true concept of widget boundaries, padding, or scrolling

The UI looks hand-drawn because it is hand-drawn, line by line.

### 3. The banner is visually dominant and operationally low value

`render_banner()` in `crates/vorker-tui/src/render.rs:104-129` prints:

- a five-line ASCII logo
- a long strapline
- an instructional sentence
- a full-width separator

That is a large amount of top-of-screen real estate for information that matters mostly once, not continuously.

In terminal products, the first screenful is extremely expensive. Spending that much vertical space on branding is a bad trade.

### 4. Every panel has the same weight

The current layout makes all regions feel equally important:

- Active agents
- Agent detail
- Run board
- Event feed
- Command deck

But they are not equally important.

The operator's main loop should usually be:

1. select session or run
2. inspect live output or task detail
3. issue an action or prompt
4. respond to approvals/errors/events

The UI should bias attention toward that loop. It currently does not.

### 5. The prompt area does not read as an input control

The bottom panel is rendered as a normal boxed panel with status lines above the prompt text in `crates/vorker-tui/src/render.rs:460-514`.

That causes several perception problems:

- the input line looks like another report row
- the cursor is not meaningful
- placeholder text and actual input text share the same visual treatment
- input mode changes are not obvious enough

For a terminal product, the input area is the control surface. It should feel active, anchored, and unmistakable.

### 6. The current test suite protects rendering safety, not usability

The render tests in `crates/vorker-tui/tests/render_dashboard.rs:101-208` mostly assert:

- output fits width
- layout stacks on medium widths
- borders stay ASCII-safe
- output stays ASCII-only

Those are valid constraints, but they do not test:

- whether focus is visible
- whether selection is legible
- whether prompts feel like prompts
- whether the banner steals too much space
- whether transcript rows remain readable
- whether activity vs detail vs input are visually prioritized correctly

That is why the test suite passes while the UI still feels bad in real use.

## Drift From The Repo's Own Plan

There is a clear gap between the approved architecture and the implemented shell.

The rewrite design says:

- the Rust TUI is the primary operator surface
- it must preserve the action rail, model picker, swarm flow, and immediate understandability
- `vorker-server` and `vorker-tui` should be clients of runtime state, not independent state owners

Source: `docs/superpowers/specs/2026-03-20-rust-runtime-rewrite-design.md:38-69` and `docs/superpowers/specs/2026-03-20-rust-runtime-rewrite-design.md:194-214`

The current implementation instead:

- mutates local snapshot state directly inside the TUI
- fabricates transcript responses
- has no `ratatui`
- has no async runtime
- has no redraw scheduler
- has no real ACP/runtime bridge from the TUI path

There is also a product messaging gap:

- `README.md:48-50` says the Rust rewrite now owns the supervisor core, terminal renderer, and native CLI entrypoint
- `crates/vorker-cli/src/main.rs:89-100` still prints placeholder text for `repl`, `chat`, `serve`, and `share`

That mismatch is not the main UI problem, but it does contribute to a feeling that the runtime story is ahead of the implementation reality.

## What Codex Gets Right That Vorker Should Copy

The useful lesson from the Codex TUI is not its visual style. It is the layering.

Codex is built more like this:

1. bootstrap/process layer
2. terminal/event broker layer
3. async app orchestration layer
4. widget/render layer

That architecture enables:

- real input widgets
- redraw coalescing
- overlays and modal flows
- streaming transcript updates
- terminal lifecycle control
- separation between runtime state and UI presentation

Vorker does not need to clone the Codex UI. It should clone the discipline:

- event-driven
- widget-based
- runtime-backed
- input-first

## What Is Worth Keeping

Not everything is wrong.

The parts worth keeping are:

- the overall dashboard shape from the rewrite spec
- arrow-key-first interaction as a product choice
- `vorker-core` as the source of truth for snapshots
- the navigation state model in `crates/vorker-tui/src/navigation.rs`
- the idea of a persistent model selector and swarm-goal mode

The problem is not the intent. The problem is the shell that presents that intent.

## Redesign Direction

The redesign should happen in three stages.

### Stage 1: Stop The Visual Bleeding

This is the shortest path to making the TUI less embarrassing before a deeper rewrite.

#### Changes

- remove the large ASCII masthead and replace it with a one-line title/status bar
- rename panels to operator language:
  - `LAUNCH RAIL` -> `ACTIONS`
  - `ACTIVE AGENTS` -> `AGENTS`
  - `RUN BOARD` -> `RUNS`
  - `EVENT FEED` -> `ACTIVITY`
  - `COMMAND DECK` -> `INPUT`
- implement actual ANSI styling in `theme.rs` so focused panes and selected rows are obvious
- stop rendering decorative internal separator lines inside content panels
- reduce descriptive helper copy under each panel
- make the prompt row visually distinct from the rest of the bottom panel
- make the model picker visually unmistakable when open

#### Expected outcome

This will not fix the architecture, but it will immediately improve:

- scanability
- alignment perception
- focus visibility
- perceived seriousness

### Stage 2: Replace The String Renderer With A Real TUI Shell

This is the actual fix.

#### Required move

Migrate `crates/vorker-tui` to the architecture the repo already planned:

- `ratatui` for layout and widgets
- `crossterm` for terminal mode/input
- `tokio` for async orchestration

#### Core structure

Split the current monolith into explicit widgets/components:

- top status bar
- actions bar
- agents list
- runs/tasks pane
- detail/transcript pane
- activity pane
- prompt input bar
- overlays for model picker and approvals

#### Runtime model

The TUI should subscribe to runtime state rather than authoring fake state itself.

That means:

- TUI dispatches intents
- runtime services emit supervisor events
- `vorker-core` produces snapshots
- UI redraws from snapshots

The TUI should stop manufacturing fake assistant responses in the UI layer.

### Stage 3: Make It A Real ACP Operator Console

The TUI should expose the operational state that matters for Copilot ACP supervision.

#### Agent/session panel should show

- session status: connecting, ready, waiting, error
- active model and mode
- cwd or worktree
- pending approval state
- most recent tool execution
- last output timestamp

#### Run/task panel should show

- arbitrator/worker assignments
- ready/running/completed counts
- active branch/worktree
- merge readiness
- failure/conflict states

#### Input/status area should show

- active target session
- current mode
- streaming state
- approval interruptions
- retry/reconnect state

#### Activity feed should be narrower and more important

Right now the feed summarizes generic events like `reply <- agent-2`. That is low value.

It should prioritize:

- approval requested
- session restarted
- task assigned
- task failed
- merge blocked
- tunnel connected/disconnected

## Recommended Layout

If the app keeps the existing dashboard intent, the screen should probably look closer to this:

```text
[vorker] [model] [target session] [tunnel] [runtime status]
[Actions: New Agent] [Swarm] [Model] [Approve] [Share]

[Agents]                     [Transcript / Task Detail]
[Runs + Task Lanes]          [Transcript / Task Detail]
[Activity]                   [Transcript / Task Detail]

[Input mode] [prompt field..............................................]
```

Key change: the right side should be the main working surface, not a decorative detail card.

## Priority Order

If I were prioritizing the work, I would do it in this order:

1. Kill the giant banner and implement real focus styling.
2. Make the bottom input area feel like an input, not a report row.
3. Remove manual separator junk and simplify panel internals.
4. Switch from string composition to `ratatui` widgets.
5. Replace synthetic prompt/session behavior with real ACP-backed state.
6. Add tests for operator comprehension, not just width safety.

## Bottom Line

The current TUI is failing for reasons deeper than color choice or spacing tweaks.

The UX is bad because the app does not feel like a real operator shell.
The UI is bad because manual string composition is doing the work that a widget/layout system should be doing.

If the goal is to make `vorker-2` a serious Copilot ACP supervisor, the right move is:

- do a quick pass to fix the obvious visual pain now
- then rebuild the TUI shell around `ratatui` + async runtime state

That is the shortest path from "ASCII dashboard mock" to "terminal-native operator console."
