# Vorker 2 TUI Intuitive UX Flow

Date: `2026-03-21`  
Project: `vorker-2`  
Reference: `/Users/lucas/Desktop/COMPANY PROJECTS /reel-scroller/openai-codex-tui-notes.md`

## Goal

Make `vorker-2` feel immediately understandable to a new operator without losing its multi-agent ACP supervisor identity.

This is not a proposal to copy Codex's visuals. It is a proposal to borrow the parts of Codex that make its flow intuitive:

- one obvious main task at a time
- one primary work surface
- strong input anchoring
- clear current target
- overlays for temporary decisions
- background activity separated from foreground work

## Core Recommendation

`vorker-2` should move from a dashboard-first interaction model to a conversation-first operator model.

That means:

- the center of the app should be the active session or task detail, not a grid of equal-weight boxes
- the bottom input/composer should become the obvious primary control
- the left side should answer "what can I operate on?"
- the top bar should answer "where am I and what state is the system in?"
- temporary flows like model switching, swarm launch, approvals, and share setup should appear as overlays or focused sheets, not as extra rows inside the main dashboard

Codex works because the operator always understands three things:

1. what thread or target is active
2. what input will do right now
3. what background work is happening outside the active thread

Vorker should adopt that same clarity.

## Three Possible Directions

### 1. Dashboard-First Cleanup

Keep the current multi-panel dashboard shape and improve hierarchy, copy, and focus handling.

Pros:

- smallest product shift
- preserves current mental model
- easiest migration path

Cons:

- still feels like operating a control board rather than doing work
- input remains visually secondary
- hard to make truly intuitive because the user is still choosing between too many equal-looking regions

### 2. Conversation-First Operator Console

Make the active transcript or task detail the main pane, with sessions and runs as navigation around it.

Pros:

- matches how users already think about AI tools
- closer to Codex's strongest UX pattern
- gives Vorker one obvious center of gravity
- makes prompting, approval, and recovery flows much clearer

Cons:

- bigger conceptual shift from the current dashboard
- requires stronger separation between navigation and detail surfaces

### 3. Split-Mode Command Center

Have a hard toggle between `Agent Mode` and `Swarm Mode`, each with a different screen layout.

Pros:

- very explicit
- reduces mode confusion inside each screen

Cons:

- risks adding one more major decision at the top of the experience
- creates more branching, more code paths, and more navigation overhead

## Recommended Direction

Approach 2 is the right move.

It preserves Vorker's unique value, multi-agent ACP supervision, but wraps it in a flow users already understand:

- pick a target
- read the latest context
- type into the composer
- respond to interrupts
- inspect background work when needed

That is much closer to how Codex feels, even though the product surface is different.

## High-Level Information Architecture

The TUI should be organized into four layers.

### 1. Top Status Bar

Purpose: answer "where am I?"

Should show:

- active workspace
- active model
- active target
- current runtime state
- tunnel state
- approval state

Example:

```text
[vorker] [workspace: reel-scroller] [target: Agent 2] [model: gpt-5.4] [tunnel: idle] [approval: none]
```

### 2. Left Navigation Column

Purpose: answer "what can I operate on?"

Should contain:

- agents
- runs
- tasks under the selected run

Rules:

- one selected row at a time
- selection is always visible
- agents and runs are not equal peers in the same scan path unless the screen makes that relationship explicit

Recommended grouping:

- `Agents`
- `Runs`
- `Tasks`

### 3. Main Work Surface

Purpose: answer "what am I looking at right now?"

This is the most important part of the screen.

It should show one of:

- active agent transcript
- active task detail
- active run summary
- approval request detail
- error/recovery detail

The key rule is simple:

Only one thing should feel primary at a time.

### 4. Bottom Composer

Purpose: answer "what happens if I press Enter?"

Should always show:

- current target
- current mode
- current action hint
- actual input field

Example:

```text
[Prompting Agent 2 on gpt-5.4] > Ask the agent to inspect the latest task failure...
```

This needs to feel like the anchor of the whole app, not like another panel.

## Target Interaction Principles

These are the behavioral rules Vorker should adopt from Codex-like products.

### 1. One Primary Action Per State

At any point, the operator should know the default Enter action.

Examples:

- on the main screen: Enter sends the composer text
- with empty composer and a selected action chip: Enter triggers that action
- in a model picker overlay: Enter confirms model selection
- in an approval overlay: Enter confirms the highlighted approval choice

The current product muddies this by overloading Enter across a grid of dashboard regions.

### 2. Overlays Should Be Real Interruptions

The following flows should not appear as extra inline rows:

- model picker
- swarm launch
- approval requests
- share/tunnel setup
- destructive confirmation

They should be temporary overlays or sheets that fully own the user's attention until dismissed.

That is how Codex keeps temporary decisions from polluting the main transcript surface.

### 3. Background Activity Must Not Compete With Foreground Work

The event feed should be secondary, not equal to the active transcript.

The active operator loop is:

1. select target
2. inspect current output or detail
3. type next instruction
4. handle interrupt if needed

The activity stream is important, but it should support that loop rather than split attention away from it.

### 4. State Names Must Be Literal

Use plain labels instead of branded metaphors.

Recommended labels:

- `Actions`
- `Agents`
- `Runs`
- `Tasks`
- `Activity`
- `Input`
- `Approvals`
- `Share`

Avoid labels that sound like marketing copy instead of operating surfaces.

### 5. The Current Target Must Always Be Visible

The user should never wonder:

- which agent they are prompting
- which run they are inspecting
- which task they are acting on
- which model is active for the next action

Codex is good at this because the active thread is obvious. Vorker needs the same level of target clarity.

## Proposed Screen Model

## Screen A: Empty Startup

This is the first-run or no-active-agent state.

### Intent

Get the user into their first meaningful action quickly.

### Layout

- top status bar
- left navigation mostly empty
- main pane shows a short getting-started state
- bottom composer is disabled until an agent exists

### Main-pane content

Show only:

- `Create an agent`
- `Launch a swarm`
- a short sentence on what each one does

Not:

- big branding
- multiple explanatory paragraphs
- empty panels with decorative borders

### Desired flow

1. user lands on empty startup
2. `Create agent` is the default action
3. Enter opens a small creation overlay
4. user confirms model and role
5. app returns to transcript-first layout with that new agent selected

## Screen B: Agent Conversation

This should be the default steady-state screen.

### Intent

Let the operator do focused work with one agent.

### Layout

- left: agent list and run list
- main: transcript
- right or lower secondary region: activity or task inspector
- bottom: composer

### Desired behavior

- Up and Down move through the current navigation list
- Tab switches between navigation areas and main surface
- Enter in composer sends
- Esc closes overlays first, never clears core context unexpectedly

### Why this is more intuitive

This maps to the mental model users already have from Codex, Claude Code, ChatGPT, Copilot Chat, and terminal chat tools:

- select thread
- read transcript
- type next prompt

Vorker stays special because the side navigation includes runs and tasks, not just conversations.

## Screen C: Run Overview

When the operator selects a run, the main pane should switch from transcript view to run view.

### Main-pane content

- run goal
- status
- task distribution
- active workers
- merge readiness
- blockers

### Secondary region

- recent run events
- selected task summary

### Desired behavior

- selecting a task in the left column updates the task inspector
- Enter on a task can either open full task detail or shift the main pane into task mode

This avoids forcing task state into a cramped lower-left mini-panel.

## Screen D: Task Detail

This is where Vorker can differentiate itself from Codex.

### Main-pane content

- task title
- assigned agent
- workspace path
- branch
- latest commit
- task status timeline
- recent transcript excerpt from the task agent

### Composer behavior

The composer should make the target explicit:

```text
[Task task-12 / Agent worker-3] > Ask the worker to rebase and rerun tests...
```

This is a more intuitive handoff than burying task state in a summary panel while leaving the prompt field generic.

## Screen E: Approval Interrupt

Approval flows should behave like a true interruption.

### Trigger

- tool call requiring approval
- shell execution requiring confirmation
- risky merge action

### Overlay content

- what is requesting approval
- which agent requested it
- why it needs approval
- what the operator can choose

### Actions

- `Approve`
- `Reject`
- `View detail`

### Key UX rule

When approval is active, the rest of the app should visually dim into the background.

This is one of the best Codex interaction patterns to copy. Interruptions should look like interruptions.

## Screen F: Swarm Launch

Swarm launch should be a short guided flow, not a buried input mode.

### Current problem

Right now swarm launch is just another action plus an inline mode switch.

### Better flow

1. user selects `Launch swarm`
2. overlay opens with three fields:
   - goal
   - planning model
   - execution strategy
3. Enter confirms and creates the run
4. app switches directly into the new run overview

### Why this is better

- the user understands they are starting a structured workflow
- the app can ask for a few important inputs without making the main screen confusing
- the transition into run mode becomes explicit

## Screen G: Share / Tunnel Flow

Share state matters, but it should not occupy permanent prime real estate unless active.

### Recommended pattern

- top bar always shows tunnel status
- opening Share brings up a focused overlay
- when the tunnel is active, the URL appears in the overlay and in a compact top-bar badge

This keeps sharing available without turning it into a permanent dashboard distraction.

## Navigation Model

The current app mixes action selection, pane focus, and prompting in a way that is hard to learn.

Recommended model:

### Arrow Keys

- Up and Down move within the active list
- Left moves outward in hierarchy
- Right moves inward in hierarchy

Example:

- from `Runs`, Right moves into `Tasks`
- from `Tasks`, Right moves into `Task Detail`
- from `Task Detail`, Left returns to `Tasks`

### Tab

Tab should cycle between major regions only:

1. left navigation
2. main work surface
3. secondary region
4. composer

### Enter

Enter activates the focused control or sends composer text.

### Esc

Esc should always do this in order:

1. close overlay
2. cancel transient mode
3. clear composer only if already focused and empty-state safe

Esc should not feel destructive.

## Recommended Copy Changes

These copy changes will make the flow easier to learn.

Replace:

- `LAUNCH RAIL` with `ACTIONS`
- `COMMAND DECK` with `INPUT`
- `EVENT FEED` with `ACTIVITY`
- `AGENT DETAIL` with `DETAIL`

Use action hints like:

- `Enter to create agent`
- `Enter to switch model`
- `Enter to launch swarm`
- `Type to prompt Agent 2`

Avoid:

- `pink lane`
- `swarm pink`
- `command deck`
- `launch rail`

## Example End-to-End Flows

## Flow 1: First Prompt To A New Agent

1. user opens Vorker
2. empty startup screen shows `Create agent` as the default action
3. Enter opens a create-agent overlay
4. user confirms role and model
5. app lands in transcript view for the new agent
6. composer gains focus automatically
7. user types a prompt and sends it
8. transcript streams in the main pane
9. activity updates in the background

This should be the smoothest path in the app.

## Flow 2: Switch From Agent Work To Swarm Supervision

1. user is in an agent transcript
2. user triggers `Launch swarm`
3. swarm overlay opens
4. user enters the goal and confirms
5. app switches to the new run overview
6. selected task appears in the inspector
7. composer target changes from agent to run or task context

The key is that the app should make the mode transition visible.

## Flow 3: Handle Approval Mid-Run

1. user is watching a run or transcript
2. approval overlay appears
3. overlay states agent, task, requested action, and risk
4. user approves or rejects
5. overlay closes
6. app returns to the previous work surface without losing context

This should feel interruptive but not disorienting.

## Flow 4: Inspect A Failed Task

1. activity shows task failure
2. user selects the run
3. user moves into the failed task
4. task detail becomes the main pane
5. composer target switches to the task's execution agent
6. user asks for logs, retry, or remediation

This is where Vorker's multi-agent UX can feel more powerful than Codex rather than merely derivative.

## Implementation Order For UX Work

Even without changing the full architecture immediately, the UX work should be staged in this order:

1. make the composer the visual anchor
2. make target identity explicit in the top bar and composer
3. convert model picker, swarm launch, and approvals into overlays
4. make transcript or task detail the main pane
5. demote activity into a clearly secondary surface
6. simplify copy and remove metaphor-heavy labels
7. tighten the navigation model so arrows, Tab, Enter, and Esc each have stable jobs

## Success Criteria

The new flow is successful if a first-time user can do these without explanation:

- create an agent
- understand which agent is active
- send a prompt
- switch models
- launch a swarm
- inspect a task
- handle an approval

If the user has to remember layout trivia or special-case key rules, the flow is still too clever.

## Bottom Line

Vorker should not try to become "Codex with runs." It should become "an ACP supervisor that borrows Codex's clarity."

That means:

- one primary work surface
- one anchored composer
- one obvious current target
- overlays for temporary decisions
- literal labels
- background activity that supports, rather than competes with, the foreground task

If Vorker adopts that interaction model, the app will feel much more intuitive without losing what makes it distinct.
