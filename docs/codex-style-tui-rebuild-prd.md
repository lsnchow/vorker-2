# PRD: Codex-Style TUI Rebuild For Vorker-2

## Status

Draft v1

## Goal

Rebuild the `vorker-2` terminal experience so it behaves like a normal CLI coding tool instead of a dashboard. The target is not merely "conversation-first". The target is a shell that feels effectively the same as Codex or Claude Code:

- compact top card
- optional one-line tip
- single scrolling transcript
- one anchored composer at the bottom
- slash commands opened from the composer with a filtered list directly below it
- agent output streamed inline as terminal chat
- temporary actions shown inline or as overlays, never as permanent dashboard furniture

This is a structural rewrite, not a styling pass.

## Why This Exists

The current Vorker TUI is hard to learn because the user lands in a multi-pane control plane instead of a conversation surface. The app asks the user to understand actions, sessions, runs, tasks, and events before they can do the obvious thing: talk to the agent.

That is the opposite of how Codex and Claude Code feel.

## Reference Behavior Observed In Codex

Observed locally on March 22, 2026 by running `codex` in this repository.

Startup behavior:

- a compact top card shows product name, model, and working directory
- the main empty state is the prompt line, not a dashboard
- the bottom status line is compact and secondary

Slash behavior:

- typing `/` in the composer opens an inline command list below the prompt
- typing additional characters filters the list immediately
- observed examples from the real UI:
  - `/` showed `/model`, `/fast`, `/permissions`, `/experimental`, `/skills`, `/review`, `/rename`, `/new`
  - `/m` narrowed the list to `/model`, `/mention`, `/mcp`
- the slash list is contextual help, not a separate mode or screen

Interaction behavior:

- the transcript remains the main surface
- commands are discovered from the composer instead of from a permanent action rail
- there is no equal-weight dashboard grid competing with the prompt

## Current Vorker Problems

Current code paths:

- Rust TUI: `crates/vorker-tui/src/app.rs`, `crates/vorker-tui/src/render.rs`
- Legacy JS TUI: `src/tui.js`, `src/tui/render.js`, `src/tui/commands.js`
- Agent backend: `src/copilot.js`

Structural issues:

- the Rust and JS TUIs are dashboard-first, not transcript-first
- the screen is split into actions, agents, runs, tasks, and activity before the user has done anything
- slash commands are line parser behavior only; there is no inline command menu
- the input area does not act like the main control surface
- multiple panes have equal visual weight, so the user cannot tell what is primary
- the backend model assumes `CopilotSession`; there is no provider abstraction for Codex CLI

Result:

- cluttered first-run experience
- poor discoverability
- weak sense of current target
- feels like a supervisor console instead of a coding CLI

## Product Decision

Vorker TUI should behave like a single-pane coding shell first, and like an orchestration console only when explicitly invoked.

That means:

- one visible transcript by default
- one visible composer by default
- orchestration details hidden unless requested
- slash-driven command discovery from the composer
- no multi-pane dashboard on startup

## Primary User Stories

### 1. Talk To An Agent Immediately

As a user, when I open the TUI, I should immediately see:

- current workspace
- current model/provider
- current target agent or chat
- a prompt composer ready for input

I should not need to understand swarms, runs, or task lanes before sending a message.

### 2. Discover Commands From The Prompt

As a user, if I type `/`, I should see a list of actions below the composer.

If I keep typing, the list should filter live.

Pressing `Enter` on a slash item should execute the command or open its sub-flow.

### 3. Read Agent Output Like A Chat

As a user, I should see:

- my prompt in the transcript
- streaming assistant output directly under it
- tool activity as compact inline status rows

I should not have to read a separate event panel to understand what is happening.

### 4. Use Advanced Features Without Clutter

As a user, advanced actions like swarm launch, task review, approvals, provider switching, and share/tunnel should exist, but they should stay out of the way until invoked.

## Non-Goals

- cloning Codex branding or exact ASCII chrome
- preserving the current launch rail / run board layout
- shipping native multi-provider orchestration in the first visual rewrite
- exposing every supervisor primitive in the default screen

## UX Requirements

### Exact Default Layout

The default screen should have 5 zones:

1. compact top card
2. optional one-line tip
3. main transcript
4. inline working/tool rows inside the transcript flow
5. bottom composer with footer status line

Example target shape:

```text
╭─────────────────────────────────────────────────╮
│ >_ OpenAI Codex (v0.116.0)                      │
│                                                 │
│ model:     gpt-5.4 xhigh   /model to change     │
│ directory: ~/Desktop/COMPANY PROJECTS /vorker-2 │
╰─────────────────────────────────────────────────╯

  Tip: New Use /fast to enable our fastest inference at 2X plan usage.

• Model changed to gpt-5.4 xhigh

› hello

◦ Working (4s • esc to interrupt)

────────────────────────────────────────────────────────────────

• Hello. What do you need help with?

› Implement {feature}

  gpt-5.4 xhigh · 96% left · ~/Desktop/COMPANY PROJECTS /vorker-2
```

### Top Card

Show only:

- app name
- version
- active model
- active workspace directory
- short command hint on the model row when relevant

Do not show:

- large branding art
- large status banners
- rails
- panel grids

### Sidebar Policy

No default sidebar.

If sessions, runs, tasks, or agents need to be shown, they should appear through:

- slash commands
- transient popups
- temporary overlays
- explicit secondary views

They should not be permanently visible in the default shell.

### Main Transcript

This is the primary surface.

It should show:

- user messages
- assistant messages
- inline tool progress rows
- approval requests
- command results
- lightweight event disclosures like `Read SKILL.md`

It should not show:

- permanent dashboard cards
- always-visible event feeds
- duplicate summaries of the same activity
- multi-column layouts
- fixed side panels

### Bottom Composer

The composer is always present.

It must support:

- plain text prompt entry
- slash command entry
- live slash filtering list
- footer status hint
- inline working state above the footer when a turn is active

The composer is the default focus on startup.

## Slash Command System

### Interaction Contract

When the composer value starts with `/`:

- show a command list directly below the composer
- highlight the active row
- filter results as the user types
- `Up` and `Down` move through command results
- `Enter` selects the active command
- `Esc` closes the command list and preserves or clears input based on command state

### Initial Slash Set

First-pass commands:

- `/model`
- `/provider`
- `/new`
- `/agents`
- `/runs`
- `/tasks`
- `/review`
- `/permissions`
- `/share`
- `/preflight`
- `/help`

Rules:

- commands must have one-line descriptions in the list
- commands can open overlays when extra input is needed
- commands should prefer inline chat replies for success/failure feedback
- slash suggestions must appear directly under the prompt, not in a separate command panel

### No Separate Command Screen

Slash must stay within the composer flow. Do not route the user into a different dashboard or modal just to discover commands.

## Visual Design Requirements

### Principles

- compact
- calm
- obvious focus state
- low ornament
- text alignment first

### Explicit Changes Required

- remove large action rails
- remove large ASCII hero treatment
- remove equal-weight multi-panel dashboard layout
- reduce border density
- use spacing to show hierarchy instead of many framed boxes
- keep one strong focus ring/highlight at a time
- keep most of the screen borderless except the top card and occasional separators

### Transcript Styling

- user and assistant turns must read as a chronological log
- tool rows should be visually subordinate to assistant text
- approvals should look interruptive but not destructive
- status lines should be single-line and compact
- working state should look like an inline progress row, not a dedicated dashboard panel
- "Explored / Read SKILL.md" style disclosures should be compact and collapsible in spirit even if rendered as simple text rows initially

## Keyboard Model

Default behavior:

- text input goes to the composer
- `/` opens the command list
- `Tab` cycles focus regions only if a temporary overlay is open
- `Esc` closes the current temporary UI first
- `Ctrl+C` exits

Avoid:

- making arrows jump unpredictably between unrelated panes
- forcing the user to understand pane focus before typing
- requiring navigation across multiple permanent regions

## Architecture Requirements

## 1. Replace Dashboard State Model

Current state is pane-centric:

- focused pane
- selected action
- active run/task/session grid navigation

New state should be shell-centric:

- active thread id
- active provider id
- composer value
- slash menu state
- overlay state
- transcript items
- footer status state
- active working row state

## 2. Introduce Provider Abstraction

Current backend is hardwired to `CopilotSession` and `CopilotManager`.

Add a provider layer:

- `AgentProvider`
- `AgentSession`
- `ProviderManager`

Initial implementations:

- `CopilotProvider`
- `CodexProvider`

`CodexProvider` should launch and manage the `codex` CLI instead of pretending Codex speaks ACP.

## 3. Separate Transcript Events From Supervisor Events

The main UI should render transcript-oriented events:

- user_prompt
- assistant_chunk
- assistant_message
- tool_started
- tool_updated
- tool_finished
- approval_requested
- approval_resolved
- system_notice
- lightweight progress disclosure rows

Supervisor/event-log detail can still exist, but not as the default main view.

## 4. Unify TUI Direction

There should be one primary TUI architecture.

Current duplication:

- JS TUI path
- Rust TUI path

The Rust TUI should become the primary product path. The JS dashboard path should be deprecated once the Rust path reaches feature parity for core interaction.

## Implementation Phases

### Phase 1: Interaction Shell

- remove dashboard-first default layout
- build top card, tip, transcript, working row, and composer shell
- add slash list triggered by `/`
- move current action rail features into slash commands or overlays
- keep existing provider backend if needed for first shell pass

### Phase 2: Transcript And Tool Activity

- render user and assistant turns chronologically
- stream assistant output inline
- render tool activity inline
- add approval prompt rows

### Phase 3: Sidebar And Secondary Surfaces

- add only explicit secondary surfaces for chats/agents/runs/tasks if they are still needed
- keep them hidden from the default shell
- hide event log from default layout

### Phase 4: Codex Backend

- add provider abstraction
- implement `CodexProvider`
- allow session creation against Copilot ACP or Codex CLI
- expose provider switching in `/provider`

### Phase 5: Cleanup

- remove dead dashboard code
- delete obsolete navigation assumptions
- simplify keyboard handling

## Acceptance Criteria

The rewrite is successful when all of the following are true:

- launching `npm run tui` opens a chat-first interface
- the composer is focused by default
- typing `/` opens a live filtered command list below the composer
- the user can type a normal prompt without understanding panes
- assistant output appears inline in transcript order
- tools and approvals appear inline with the conversation
- advanced orchestration data is accessible but not mandatory
- the default experience feels visually and behaviorally close to Codex/Claude Code
- no permanent multi-pane navigation is needed to send the first prompt
- the shell can show compact rows like `Working (4s • esc to interrupt)` and `Explored / Read SKILL.md` inline

## Migration Notes

What to keep:

- supervisor store and event model where still useful
- orchestrator concepts
- preflight capabilities
- web control plane

What to demote or remove from default TUI:

- launch rail
- run board as primary surface
- event log as primary surface
- pane-first navigation as the main interaction contract
- any always-visible sidebar

## Short Recommendation

Do not try to cosmetically tune the current layout into compliance.

Delete the dashboard mental model from the default TUI and rebuild around:

- compact top card
- transcript
- inline working rows
- composer
- slash list
