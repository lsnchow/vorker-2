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

## Ownership Model

The runtime has a single source of truth for state transitions: the supervisor pipeline.

Rules:

1. Runtime commands enter through `vorker-cli`, `vorker-tui`, or `vorker-server`.
2. Those commands call service entrypoints in `vorker-orchestrator`, `vorker-acp`, or `vorker-tunnel`.
3. Services do not mutate shared snapshots directly.
4. Services emit typed supervisor events through `vorker-core`.
5. `vorker-core` applies events to the canonical store.
6. The canonical store publishes the current snapshot to TUI and server subscribers.
7. The durable event log is appended from the same supervisor event stream.

This means:

- `vorker-core` owns event definitions, event application, snapshot production, and durable replay.
- `vorker-orchestrator` owns run/task command handling and derives the next events to emit, but does not own the canonical snapshot.
- `vorker-acp` owns Copilot session side effects and emits session-related events, but does not own the canonical snapshot.
- `vorker-git` owns git side effects and returns structured outcomes to `vorker-orchestrator`.
- `vorker-server` and `vorker-tui` are clients of the runtime state, not independent state owners.

## Service Interfaces

The planning boundary should assume the following crate-level public interfaces.

### `vorker-core`

Public responsibilities:

- `SupervisorEvent`
- `SupervisorStore`
- `Snapshot`
- `EventLogReader`
- `EventLogWriter`
- compatibility decode for JS-produced NDJSON events

### `vorker-acp`

Public responsibilities:

- `CopilotManager`
- `CopilotSessionHandle`
- commands:
  - create session
  - close session
  - set mode
  - set model
  - prompt session
- event outputs:
  - session registered
  - session updated
  - prompt started
  - prompt finished

### `vorker-git`

Public responsibilities:

- `TaskWorkspaceManager`
- commands:
  - ensure task workspace
  - commit task workspace
  - merge task branch
- structured results:
  - workspace path
  - branch name
  - base branch
  - changed files
  - commit sha
  - merge result

### `vorker-orchestrator`

Public responsibilities:

- `Orchestrator`
- commands:
  - create run
  - update run
  - create task
  - update task
  - plan run
  - dispatch task
  - auto-dispatch ready tasks
  - merge task
  - merge completed tasks
- event outputs:
  - run created
  - run updated
  - task created
  - task updated

### `vorker-server`

Public responsibilities:

- authenticated websocket clients
- request handlers that call orchestrator/acp/tunnel commands
- snapshot fanout to web clients
- protocol compatibility layer for the current web UI

### `vorker-tui`

Public responsibilities:

- local operator interaction
- rendering current snapshot
- dispatching runtime commands
- no direct mutation of shared state

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

## Command and Event Flow

The runtime command path must be explicit.

### Agent Creation

1. TUI or server sends `CreateAgent { name, role, model, mode, cwd }`
2. `vorker-acp` starts the session
3. `vorker-acp` emits `session.registered`
4. `vorker-core` updates the store
5. snapshot subscribers refresh

### Prompt Send

1. TUI or server sends `PromptAgent { session_id, text }`
2. `vorker-acp` emits `session.prompt.started`
3. ACP roundtrip completes
4. `vorker-acp` emits `session.prompt.finished`
5. `vorker-core` appends transcript and updates snapshot

### Swarm Launch

1. TUI or server sends `LaunchSwarm { goal, model }`
2. `vorker-acp` creates planner and worker sessions
3. `vorker-orchestrator` creates run and initial run metadata
4. `vorker-orchestrator` emits run/task events during planning
5. `vorker-orchestrator` dispatches ready tasks
6. `vorker-git` provisions worktrees and merge inputs for task execution
7. completion events update the canonical store

### Merge Task

1. TUI or server sends `MergeTask { task_id }`
2. `vorker-orchestrator` validates task state
3. `vorker-git` executes merge
4. `vorker-orchestrator` emits `task.updated` and `run.updated`
5. `vorker-core` updates snapshot and durable log

The plan should treat these as the primary end-to-end flows to preserve.

## Data Contracts

### Durable Event Log

The file `.vorker-2/logs/supervisor.ndjson` remains newline-delimited JSON.

Requirements:

- retain event-per-line semantics
- support replay across restarts
- support explicit compatibility decoding for older JS-produced events
- use explicit schema versioning if a breaking format change becomes unavoidable

Recommendation:

- keep the current payload shape initially
- add a top-level version field only if needed for migration safety

### Replay Compatibility Policy

The Rust runtime must be able to read existing JS-produced event logs that contain at least these event families:

- `run.created`
- `run.updated`
- `task.created`
- `task.updated`
- `session.registered`
- `session.updated`
- `session.prompt.started`
- `session.prompt.finished`
- `skills.updated`
- `share.updated`

Compatibility rules:

1. Unknown top-level fields are ignored.
2. Unknown payload fields are ignored.
3. Missing fields fall back to the same logical defaults used by the current JS store, not new Rust-specific defaults.
4. Rust-emitted events should remain shape-compatible with the current JS event families during the migration period.
5. A true breaking event-schema change requires a top-level version field and an explicit migration reader.

Planning assumption:

- phase 2 includes fixture-based replay tests against real or representative JS NDJSON logs checked into the Rust test suite.

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

The Rust server should preserve the current web UI contract closely enough that the existing frontend can remain in place.

#### Connection Lifecycle

1. client opens websocket
2. server sends initial auth requirement or current authenticated state
3. client authenticates with password if required
4. server sends initial snapshot payloads
5. client sends action messages
6. server broadcasts incremental updates and refreshed snapshots

#### Minimum Auth Flow

Required messages:

- client to server:
  - `authenticate`
- server to client:
  - `auth_required`
  - `auth_ok`
  - `auth_error`

Payload requirements:

- `authenticate`
  - `password`
- `auth_required`
  - `passwordHint` or equivalent current metadata if present
- `auth_error`
  - operator-readable message

#### Minimum Runtime Message Vocabulary

The Rust server must support, at minimum, the message families already implied by the current runtime:

- snapshot and list messages
  - agents
  - runs
  - tasks
  - share
  - skills
- agent commands
  - create agent
  - update agent mode/model
  - prompt agent
- run/task commands
  - create run
  - select or inspect run/task state
  - plan run
  - dispatch run
  - merge run
  - merge task
- share commands
  - start tunnel
  - stop tunnel

#### Payload Policy

Rules:

1. Message names remain stable during the migration wherever practical.
2. Run, task, session, share, and skills payload shapes should mirror the canonical snapshot structures from `vorker-core`.
3. Incremental update messages may be emitted alongside full snapshots, but the frontend must always be able to recover from a fresh snapshot.
4. The planning phase must enumerate the exact current server message names from the JS implementation and map each one to the Rust equivalent before phase 5 begins.

The migration should avoid inventing a brand-new protocol unless a compatibility adapter is added intentionally and scoped.

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
