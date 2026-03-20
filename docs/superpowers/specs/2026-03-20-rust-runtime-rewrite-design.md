# Rust Runtime Rewrite Design

## Goal

Replace the current JavaScript runtime under `src/` with a Rust workspace while preserving user-visible behavior, with the Rust TUI as the first-class operator interface.

The rewrite must cover:

- TUI
- CLI entrypoint
- supervisor event/store layer
- Copilot ACP session management
- orchestrator
- git task worktrees and merge flow
- tunnel/share support
- server API used by the existing Next.js control plane

The existing React web UI in `app/`, `components/`, and `hooks/` stays in place for phase 1. It will talk to a Rust server instead of the current Node process.

## Why This Rewrite Exists

The current JavaScript runtime works, but it has reached the point where the terminal operator experience, runtime composition, and long-running process behavior should move to a more durable systems language. The explicit user request is to focus on the TUI, but the approved scope is a full `src/` rewrite, not a Rust wrapper around the current JS implementation.

That means this is not a cosmetic port. It is a runtime migration with parity requirements.

## Non-Goals

- Rewriting the Next.js frontend in Rust
- Changing the overall product shape from “local-first Copilot supervisor with web/mobile control plane”
- Changing durable event-log semantics unless required for correctness
- Replacing Copilot ACP with a different model provider during this rewrite
- Expanding the scope into unrelated product ideas

## Success Criteria

The rewrite is successful when all of the following are true:

1. `vorker tui` runs as a native Rust TUI and supports the current operator flows:
   - boot animation
   - action rail
   - persistent model selection
   - new agent creation
   - swarm launch
   - session/run/task/event navigation
   - prompt sending
2. `vorker serve` is provided by Rust and the current web UI can connect to it without frontend rewrites beyond transport or protocol adjustments that are strictly necessary.
3. Durable supervisor replay still works from `.vorker-2/logs/supervisor.ndjson`.
4. Task worktree creation, task commits, and task merges still function.
5. Cloudflare tunnel start/stop still functions.
6. Existing runtime behavior is covered by automated tests in Rust.
7. The Rust runtime can fully replace the JS `src/` tree in normal use.

## User Experience Requirements

### TUI

The Rust TUI is the primary operator surface and must preserve the current intent:

- launch rail at the top
- active agents in the left column
- run/task state in the lower left
- detail/event surfaces on the right
- green default accents
- pink swarm accent
- arrow-key-first interaction
- modal model picker
- inline swarm-goal prompt flow

The Rust TUI must not regress on the main complaint that triggered the redesign: it must feel immediately understandable without memorizing slash commands.

### Web Control Plane

The current web control plane must remain usable throughout the migration. Its transport and backing runtime may change, but the practical operator workflows should remain intact:

- connect
- list agents/runs/tasks
- inspect task metadata
- merge tasks/runs
- trigger tunnels

## Architecture

The Rust rewrite will be a Cargo workspace with the following crates.

### `crates/vorker-core`

Responsibilities:

- canonical supervisor event types
- event serialization/deserialization
- in-memory store and snapshots
- durable replay
- shared domain types for runs, tasks, sessions, skills, and share state

This crate is the runtime spine. All other crates depend on it for the canonical data model.

### `crates/vorker-acp`

Responsibilities:

- Copilot process lifecycle
- ACP transport setup
- session state
- prompt execution
- session mode/model changes
- permission handling
- terminal tool bridging if still needed by the ACP session

This crate replaces the behavior currently spread across `src/copilot.js`.

### `crates/vorker-git`

Responsibilities:

- task worktree provisioning
- task branch naming
- commit detection
- commit creation
- merge execution
- merge error/conflict reporting

This crate replaces `src/git/task-workspace.js`.

### `crates/vorker-orchestrator`

Responsibilities:

- run creation/update
- task creation/update
- planning flow
- dispatch flow
- auto-dispatch
- merge-completed flow
- run status recomputation

This crate depends on `vorker-core`, `vorker-acp`, and `vorker-git`.

### `crates/vorker-skills`

Responsibilities:

- skill discovery
- skill metadata parsing
- snippet loading

This crate replaces `src/skills.js`.

### `crates/vorker-tunnel`

Responsibilities:

- `cloudflared` process lifecycle
- share URL detection
- share state snapshots

This crate replaces `src/tunnel.js` and the runtime path used by `src/share.js`.

### `crates/vorker-server`

Responsibilities:

- HTTP server
- websocket session broadcasting
- auth/password handling compatible with current usage
- command handlers used by the web UI

This crate replaces `src/server.js`.

### `crates/vorker-tui`

Responsibilities:

- terminal rendering
- keyboard handling
- boot animation
- model picker
- swarm goal input flow
- action dispatch to the runtime

Recommended libraries:

- `ratatui`
- `crossterm`

### `crates/vorker-cli`

Responsibilities:

- top-level binary
- subcommands such as `tui`, `serve`, `share`
- configuration loading
- process assembly

## Process Model

The Rust runtime will run as one process for normal CLI usage.

Recommended internal structure:

- async runtime: `tokio`
- event fanout: `tokio::sync::broadcast` or `watch` depending on stream type
- command execution: `mpsc` request queues into state-owning services
- state owner: one supervisor service that records events and updates the canonical store

The canonical rule is:

1. runtime action occurs
2. typed supervisor event is emitted
3. store applies event
4. durable log appends event
5. TUI/server subscribers receive updated snapshots

This preserves the current “NDJSON is the universal bus” model while making it type-safe.

## Data Contracts

### Durable Event Log

The file `.vorker-2/logs/supervisor.ndjson` remains newline-delimited JSON.

Requirements:

- retain event-per-line semantics
- support replay across restarts
- support unknown-field tolerance where practical so the migration can read older JS-produced events
- use explicit schema versioning if a breaking format change becomes unavoidable

Recommendation:

- keep the current payload shape initially
- add a top-level version field only if needed for migration safety

### Runtime Snapshot

The canonical snapshot exposed to the TUI and server must include:

- runs with embedded tasks
- flat task list
- sessions
- skills
- share state
- recent events

This matches the current `SupervisorStore.snapshot()` behavior closely enough to keep frontend integration tractable.

### Server Protocol

The Rust server should initially preserve the current message vocabulary as much as possible:

- auth flow
- agents list/update payloads
- runs/tasks payloads
- merge commands
- share commands
- prompt send actions

The migration should avoid introducing a new protocol unless the existing one is clearly blocking or incoherent.

## Migration Strategy

The rewrite should proceed in vertical slices, not by building all crates in isolation and integrating at the end.

### Phase 1: Rust Core and TUI Shell

Build:

- workspace skeleton
- `vorker-core`
- `vorker-tui`
- `vorker-cli`

Temporary mode:

- TUI may use fixture or stub data until core wiring is live

Exit criteria:

- native Rust TUI renders the launch rail, agents, runs, tasks, detail, and events
- keyboard state machine matches the approved UX

### Phase 2: Supervisor/Event Replay

Build:

- typed events
- durable event log reader/writer
- store reducer

Exit criteria:

- Rust runtime can replay existing JS NDJSON logs
- snapshot parity against representative JS fixtures is covered by tests

### Phase 3: ACP/Copilot Sessions

Build:

- ACP client/session layer
- agent lifecycle
- prompt sending
- mode/model updates

Exit criteria:

- Rust can create a real Copilot-backed agent and exchange prompts
- model selection used by the TUI actually applies to created sessions

### Phase 4: Orchestrator and Git

Build:

- run/task lifecycle
- worktree manager
- commit and merge flows
- swarm launch path end to end

Exit criteria:

- Rust swarm launch creates planner/workers, plans a run, dispatches work, and commits outputs

### Phase 5: Server Replacement

Build:

- Rust HTTP/websocket server
- existing web UI connected to it

Exit criteria:

- `vorker serve` works with the current frontend

### Phase 6: Tunnel/Share and Final Parity

Build:

- Cloudflare share management
- parity cleanup
- operational hardening

Exit criteria:

- Rust runtime covers `tui`, `serve`, and `share`
- JS `src/` runtime is removable

## Boundary Rules

To keep the rewrite sane:

- Rust owns runtime logic under the new workspace.
- Existing JS `src/` is reference material only once a slice is ported.
- Frontend React code is not ported in this effort.
- The Rust TUI must not talk to a hidden JS daemon in the final architecture. Early scaffolding is acceptable only if explicitly temporary and removed before parity claims.

## Error Handling

Each crate must expose typed errors and preserve operator-readable context.

Requirements:

- ACP startup failures must identify missing binary, auth/setup failures, or protocol initialization failures distinctly.
- Git failures must preserve command, repository/worktree context, and stderr.
- Merge conflicts must be represented as first-class task/run outcomes, not opaque generic failures.
- Tunnel failures must preserve binary/process output.
- TUI input failures or terminal restore failures must leave the terminal in a sane state on exit.

Recommendation:

- use `thiserror` for domain errors
- use `anyhow` only at top-level command boundaries

## Testing Strategy

### Unit Tests

Cover:

- store reduction and snapshot generation
- navigation state machine
- action rail behavior
- run/task update logic
- branch/worktree naming
- NDJSON parsing/serialization

### Integration Tests

Cover:

- replaying an event log into a snapshot
- creating ACP-backed sessions with mocked ACP transport where needed
- orchestrator dispatch with test doubles
- git worktree/commit/merge behavior against temporary repos
- server command handling against websocket clients

### Golden/Fixture Tests

Use fixtures from the existing JS runtime where possible to assert parity for:

- session snapshots
- run/task snapshots
- rendered TUI states at key milestones

### Manual Verification Gates

Before removing the JS runtime, verify:

- create agent from model rail
- launch swarm from model rail
- send prompt to selected agent
- replay persisted history
- merge completed task
- start/stop tunnel
- load current web UI against Rust server

## Compatibility Risks

### ACP SDK Gap

The current runtime uses `@agentclientprotocol/sdk`. Rust may not have a directly equivalent mature client. This is the highest-risk integration point.

Mitigation:

- prototype ACP session startup early in phase 3
- do not postpone this risk until after broad crate buildout

### Event Format Drift

If Rust emits a materially different event shape too early, replay and web compatibility will fracture.

Mitigation:

- lock an explicit compatibility fixture set before changing event structures

### Terminal Behavior

The TUI is the main user-facing reason for the rewrite. Terminal restore bugs, raw-mode bugs, and resize glitches are unacceptable regressions.

Mitigation:

- isolate terminal lifecycle in `vorker-tui`
- add tests for navigation logic
- run explicit smoke tests in real terminals during implementation

## Open Decisions

These are expected implementation decisions, not spec gaps:

- exact internal channel topology between runtime services
- exact crate naming if minor renames improve clarity
- exact HTTP/websocket framework, likely `axum`
- exact ACP transport abstraction if a custom client layer is required

These do not block planning because they do not change the visible architecture or migration order.

## Recommended Tech Stack

- async runtime: `tokio`
- TUI: `ratatui`, `crossterm`
- server: `axum`, `tokio-tungstenite` if needed
- serialization: `serde`, `serde_json`
- errors: `thiserror`, `anyhow`
- process management: `tokio::process`
- git commands: shell out to `git` initially rather than introducing a git library

## First Implementation Slice

The first implementation slice should be:

1. Cargo workspace and top-level `vorker` binary
2. `vorker-core` typed events/store
3. `vorker-tui` with the current approved launch-rail UX
4. fixture-backed snapshot feed

This slice proves the TUI rewrite in Rust without blocking on ACP.

## Planning Readiness

This spec is ready for implementation planning because it defines:

- target scope
- crate boundaries
- migration order
- parity constraints
- error-handling expectations
- testing requirements
- highest-risk area to prototype early
