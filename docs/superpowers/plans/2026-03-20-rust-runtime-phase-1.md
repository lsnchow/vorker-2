# Rust Runtime Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the JavaScript TUI/CLI/supervisor foundation with a native Rust workspace that can render the operator dashboard, manage local runtime state, and expose the first Rust entrypoint without regressing the current operator flows.

**Architecture:** Phase 1 keeps the existing Next.js frontend in place and focuses on the Rust-native terminal path first. The Cargo workspace will introduce `vorker-core`, `vorker-tui`, and `vorker-cli` as the initial vertical slice, with parity fixtures derived from the current JavaScript runtime so the Rust rewrite stays aligned with existing behavior.

**Tech Stack:** Rust stable, Cargo workspace, `ratatui`, `crossterm`, `serde`, `serde_json`, `tokio`, `anyhow`, `thiserror`, `assert_cmd`, `insta`

---

## File Structure

### Rust workspace root

- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `.cargo/config.toml`
- Modify: `package.json`
- Modify: `README.md`

### Core domain crate

- Create: `crates/vorker-core/Cargo.toml`
- Create: `crates/vorker-core/src/lib.rs`
- Create: `crates/vorker-core/src/events.rs`
- Create: `crates/vorker-core/src/models.rs`
- Create: `crates/vorker-core/src/store.rs`
- Create: `crates/vorker-core/src/event_log.rs`
- Create: `crates/vorker-core/tests/store_snapshot.rs`
- Create: `crates/vorker-core/tests/event_log_replay.rs`

### TUI crate

- Create: `crates/vorker-tui/Cargo.toml`
- Create: `crates/vorker-tui/src/lib.rs`
- Create: `crates/vorker-tui/src/app.rs`
- Create: `crates/vorker-tui/src/actions.rs`
- Create: `crates/vorker-tui/src/navigation.rs`
- Create: `crates/vorker-tui/src/theme.rs`
- Create: `crates/vorker-tui/src/boot.rs`
- Create: `crates/vorker-tui/src/render.rs`
- Create: `crates/vorker-tui/tests/navigation.rs`
- Create: `crates/vorker-tui/tests/render_dashboard.rs`
- Create: `crates/vorker-tui/tests/boot_frame.rs`

### CLI crate

- Create: `crates/vorker-cli/Cargo.toml`
- Create: `crates/vorker-cli/src/main.rs`
- Create: `crates/vorker-cli/tests/help.rs`

### Fixtures and migration notes

- Create: `fixtures/rust/supervisor-snapshot.json`
- Create: `fixtures/rust/supervisor-events.ndjson`
- Create: `scripts/export-rust-fixtures.mjs`
- Modify: `docs/superpowers/specs/2026-03-20-rust-runtime-rewrite-design.md`

## Chunk 1: Workspace Bootstrap

### Task 1: Create the Cargo workspace skeleton and CLI smoke test

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `.cargo/config.toml`
- Create: `crates/vorker-cli/Cargo.toml`
- Create: `crates/vorker-cli/src/main.rs`
- Test: `crates/vorker-cli/tests/help.rs`
- Modify: `package.json`

- [ ] **Step 1: Write the failing CLI help test**

```rust
use assert_cmd::Command;

#[test]
fn cli_help_lists_tui_and_serve_commands() {
    let mut cmd = Command::cargo_bin("vorker").unwrap();
    cmd.arg("--help");
    cmd.assert().success().stdout(predicates::str::contains("tui"));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vorker-cli help -- --nocapture`
Expected: FAIL because the workspace and binary do not exist yet.

- [ ] **Step 3: Write the minimal Cargo workspace and CLI implementation**

Create a workspace with `vorker-cli`, define a `vorker` binary, and make `--help` print the current subcommands (`tui`, `serve`, `share`, `repl`, `chat`) plus the shared options copied from `src/index.js`.

- [ ] **Step 4: Run the CLI help test again**

Run: `cargo test -p vorker-cli help -- --nocapture`
Expected: PASS

- [ ] **Step 5: Wire Node scripts so Rust checks are easy to run**

Add `rust:check` and `rust:test` scripts in `package.json` that call `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.

- [ ] **Step 6: Commit the workspace bootstrap**

```bash
git add Cargo.toml rust-toolchain.toml .cargo/config.toml package.json crates/vorker-cli
git commit -m "feat: bootstrap Rust workspace and CLI"
```

## Chunk 2: Core Supervisor Parity

### Task 2: Port event types, records, snapshots, and replay into `vorker-core`

**Files:**
- Create: `crates/vorker-core/Cargo.toml`
- Create: `crates/vorker-core/src/lib.rs`
- Create: `crates/vorker-core/src/events.rs`
- Create: `crates/vorker-core/src/models.rs`
- Create: `crates/vorker-core/src/store.rs`
- Create: `crates/vorker-core/src/event_log.rs`
- Create: `crates/vorker-core/tests/store_snapshot.rs`
- Create: `crates/vorker-core/tests/event_log_replay.rs`
- Create: `fixtures/rust/supervisor-snapshot.json`
- Create: `fixtures/rust/supervisor-events.ndjson`
- Create: `scripts/export-rust-fixtures.mjs`

- [ ] **Step 1: Write the failing snapshot parity tests**

Write tests that deserialize the fixture NDJSON, apply events to `SupervisorStore`, and assert that the produced snapshot matches `fixtures/rust/supervisor-snapshot.json`.

- [ ] **Step 2: Generate fixtures from the current JavaScript store**

Run: `node scripts/export-rust-fixtures.mjs`
Expected: fixture files created under `fixtures/rust/`

- [ ] **Step 3: Run the Rust core tests to verify they fail**

Run: `cargo test -p vorker-core store_snapshot -- --nocapture`
Expected: FAIL because `vorker-core` does not exist yet.

- [ ] **Step 4: Implement the canonical Rust event/store model**

Port the behavior from:
- `src/supervisor/events.js`
- `src/supervisor/store.js`
- `src/supervisor/event-log.js`
- `src/supervisor/bootstrap.js`

Keep the JS-compatible NDJSON envelope and record ordering semantics.

- [ ] **Step 5: Re-run the focused Rust core tests**

Run: `cargo test -p vorker-core -- --nocapture`
Expected: PASS

- [ ] **Step 6: Commit the core supervisor port**

```bash
git add crates/vorker-core fixtures/rust scripts/export-rust-fixtures.mjs
git commit -m "feat: port supervisor core to Rust"
```

## Chunk 3: Native TUI Rendering and Navigation

### Task 3: Port the TUI state machine, boot animation, navigation, and dashboard renderer

**Files:**
- Create: `crates/vorker-tui/Cargo.toml`
- Create: `crates/vorker-tui/src/lib.rs`
- Create: `crates/vorker-tui/src/app.rs`
- Create: `crates/vorker-tui/src/actions.rs`
- Create: `crates/vorker-tui/src/navigation.rs`
- Create: `crates/vorker-tui/src/theme.rs`
- Create: `crates/vorker-tui/src/boot.rs`
- Create: `crates/vorker-tui/src/render.rs`
- Test: `crates/vorker-tui/tests/navigation.rs`
- Test: `crates/vorker-tui/tests/render_dashboard.rs`
- Test: `crates/vorker-tui/tests/boot_frame.rs`

- [ ] **Step 1: Write failing navigation and render tests**

Cover:
- action rail focus movement
- persistent model selection
- agent list selection
- swarm accent rendering
- boot frame lane/status output

- [ ] **Step 2: Verify the tests fail**

Run: `cargo test -p vorker-tui -- --nocapture`
Expected: FAIL because the crate and render pipeline do not exist yet.

- [ ] **Step 3: Implement the minimal TUI state model and renderer**

Port behavior from:
- `src/tui.js`
- `src/tui/navigation.js`
- `src/tui/controller.js`
- `src/tui/render.js`
- `src/tui/boot.js`
- `src/tui/theme.js`

Use `ratatui` + `crossterm`, preserve:
- top launch rail
- green base theme
- pink swarm accent
- arrow-key-first navigation
- modal model picker
- inline swarm-goal prompt

- [ ] **Step 4: Re-run the TUI crate tests**

Run: `cargo test -p vorker-tui -- --nocapture`
Expected: PASS

- [ ] **Step 5: Add snapshot-style render assertions**

Use `insta` or string buffer assertions to pin the launch rail, active-agent list, and run/task panes so later runtime work does not regress the TUI layout.

- [ ] **Step 6: Commit the native TUI port**

```bash
git add crates/vorker-tui
git commit -m "feat: add Rust TUI runtime"
```

## Chunk 4: CLI Assembly and First End-to-End Loop

### Task 4: Connect the Rust CLI to the Rust core/TUI and provide a working `tui` command

**Files:**
- Modify: `crates/vorker-cli/Cargo.toml`
- Modify: `crates/vorker-cli/src/main.rs`
- Modify: `crates/vorker-cli/tests/help.rs`
- Modify: `README.md`
- Modify: `package.json`

- [ ] **Step 1: Write the failing `tui` smoke test**

Add a smoke test that runs `cargo run -p vorker-cli -- tui --help` and asserts the command exists. Add a second test that renders one non-interactive frame from a fixture-backed snapshot in test mode.

- [ ] **Step 2: Verify the smoke tests fail**

Run: `cargo test -p vorker-cli -- --nocapture`
Expected: FAIL because the CLI does not yet wire the Rust TUI command.

- [ ] **Step 3: Implement CLI command dispatch into the Rust TUI**

Support:
- `vorker tui`
- `vorker serve` placeholder usage text
- `vorker share` placeholder usage text

Keep command names aligned with the Node CLI so package ergonomics stay stable during migration.

- [ ] **Step 4: Run workspace verification**

Run: `cargo fmt --check`
Expected: PASS

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS

Run: `cargo test --workspace`
Expected: PASS

- [ ] **Step 5: Update docs and package scripts**

Document the Rust-first TUI path in `README.md`, and keep Node scripts around only for the still-unported web/mobile runtime.

- [ ] **Step 6: Commit the first end-to-end Rust slice**

```bash
git add README.md package.json crates/vorker-cli
git commit -m "feat: wire Rust CLI to native TUI"
```

## Chunk 5: Migration Guardrails for Phase 2+

### Task 5: Capture the remaining runtime port boundary so subsequent Rust crates can land cleanly

**Files:**
- Modify: `docs/superpowers/specs/2026-03-20-rust-runtime-rewrite-design.md`
- Modify: `README.md`

- [ ] **Step 1: Document what is now Rust-native vs. still JavaScript**

Explicitly list:
- Rust: CLI, TUI, supervisor core
- JavaScript: ACP, orchestrator, git automation, tunnel, server, web control plane

- [ ] **Step 2: Add the next-crate order**

Document the next implementation sequence:
1. `vorker-git`
2. `vorker-acp`
3. `vorker-orchestrator`
4. `vorker-server`
5. `vorker-tunnel`

- [ ] **Step 3: Re-run the combined checks**

Run: `cargo test --workspace`
Expected: PASS

Run: `npm run test:unit`
Expected: PASS

- [ ] **Step 4: Commit the migration guardrails**

```bash
git add README.md docs/superpowers/specs/2026-03-20-rust-runtime-rewrite-design.md
git commit -m "docs: record Rust migration boundary"
```
