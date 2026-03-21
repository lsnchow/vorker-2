# vorker-2

A terminal-first control plane for orchestrating GitHub Copilot CLI agents in ACP mode. `vorker-2` keeps the original local-first web/mobile surface, but adds a shared supervisor/event layer and a text UI so the same Copilot runtime can drive desktop terminal workflows, mobile control, and remote tunnel access.

**Not a scraper.** `vorker` runs the official Copilot CLI as an ACP server—no VS Code UI automation or private HTTP endpoint crawling. Agents run locally with restricted permissions that can be approved interactively.

**CA certificate handling.** On environments where Copilot CLI needs CA-chain configuration for `*.individual.githubcopilot.com`, `vorker` injects certificates automatically when spawning Copilot sessions.

## What is implemented

- Shared supervisor event model with NDJSON logging and state replay primitives
- Terminal dashboard command flow via `vorker tui`
- Git-backed per-task worktrees with task-specific execution agents
- Durable run/task replay from `.vorker-2/logs/supervisor.ndjson`
- Parallel auto-dispatch for ready tasks
- Local CLI chat and REPL
- Shared ACP session layer for spawning multiple Copilot agents
- Authenticated Next.js web control plane for phone or desktop access
- Cookie-authenticated websocket prompt streaming with long-polling fallback
- Agent profiles with role, notes, per-agent skill attachments, and live mode/model changes
- Run orchestration: arbitrator selection, worker pools, task planning, task editing, and dispatch
- In-app Cloudflare Quick Tunnel start/stop controls
- Tool permission flow over websocket or polling
- Secure defaults for remote access
- Automatic Copilot CA-chain workaround for the packaged CLI runtime
- `vorker share` for Cloudflare Quick Tunnels without paid hosting

## Commands

```bash
npm run tui
npm run tui:once
npm start -- repl
npm start -- chat "summarize this repo"
npm run serve
npm start -- share
```

You can also call the binary directly once linked:

```bash
vorker tui
vorker repl
vorker serve
vorker share
```

## Rust runtime (phase 1)

The Rust rewrite now owns the supervisor core, the terminal renderer, and a native `vorker` CLI entrypoint. The web/mobile control plane and the Copilot/orchestrator runtime are still on the JavaScript side for now.

Run the Rust TUI:

```bash
source "$HOME/.cargo/env"
cargo run -p vorker-cli -- tui
```

Render a single frame without entering the interactive loop:

```bash
source "$HOME/.cargo/env"
cargo run -p vorker-cli -- tui --once
```

Verify the Rust workspace:

```bash
source "$HOME/.cargo/env"
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Terminal Dashboard

Run the terminal-native dashboard:

```bash
npm run tui
```

That command uses the Rust launcher script and auto-sources `~/.cargo/env` when it exists, so you do not need a separate `source "$HOME/.cargo/env"` step.

One-shot render for quick checks:

```bash
npm run tui:once
```

The old Node TUI is still available if you need it:

```bash
npm run tui:js
```

If you prefer a shell script directly:

```bash
./scripts/run-rust-tui.sh
```

The TUI is built on top of the same supervisor state that will eventually feed the mobile and web control planes. Current commands:

- `/agent <name>` creates and selects an agent
- `/use <session-id>` switches the active agent
- `/run <name> | <goal>` creates and selects a run
- `/run-use <run-id>` switches the active run
- `/plan` plans the active run
- `/dispatch` dispatches ready tasks in the active run
- `/merge` merges completed task branches for the active run
- `/merge-task <task-id>` merges one completed task branch
- `/share start` or `/share stop` controls the Cloudflare tunnel wrapper
- plain text sends a prompt to the active agent

When a task is dispatched, `vorker-2` now creates or reuses a git worktree under `.vorker-2/worktrees`, spawns a task-specific Copilot agent rooted in that workspace, and records the workspace path, branch name, and execution agent id in the task state.
If the worker changed files, `vorker-2` also creates a task-branch commit automatically so the task branch is immediately inspectable and ready for later merge/review workflows.
Completed task branches can now be merged back into the base branch from the TUI or the existing task inspector; merges stay supervisor-owned instead of being left to ad hoc shell work.

## Remote web control plane

Run the local server:

```bash
VORKER_PASSWORD=your-password npm run serve
```

Then open:

```text
http://127.0.0.1:4173
```

If `VORKER_PASSWORD` is not set, the server generates a one-time pairing password and prints it on startup.

Once logged in, the UI gives you:

- local access status and pairing password
- Cloudflare Quick Tunnel controls
- agent creation and editing
- optional skill attachment per agent
- run planning with an arbitrator agent
- task creation and worker dispatch
- a live graph of agents, runs, tasks, and share state
- a per-agent console and transcript
- recent activity feed

## Share Over Cloudflare Quick Tunnel

`vorker share` keeps the app on the user's machine and opens a public HTTPS URL through Cloudflare Quick Tunnel.

Requirements:

- `cloudflared` installed locally
- GitHub Copilot CLI installed locally

Example:

```bash
VORKER_PASSWORD=your-password vorker share
```

That command:

- starts the local server on `127.0.0.1`
- trusts forwarded HTTPS metadata from the tunnel
- launches `cloudflared tunnel --protocol http2 --edge-ip-version auto --url http://127.0.0.1:<port>`
- prints a public `https://...trycloudflare.com?transport=poll` URL

The shared URL uses long-polling by default so it does not depend on websocket support in the tunnel path.
`http2` is the default Cloudflare edge protocol for `vorker share` because some networks block or degrade QUIC/UDP.
`auto` is the default Cloudflare edge IP mode so `cloudflared` can use IPv6 when IPv4 paths to Cloudflare Tunnel are flaky.

To install `cloudflared`, follow Cloudflare's install docs for your platform:

- <https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/>

## Security model

The server is designed to be safe by default:

- It binds to `127.0.0.1` by default.
- It uses an `HttpOnly` session cookie with `SameSite=Strict`.
- It binds every authenticated browser session to a per-session CSRF token.
- Websocket upgrades require a valid authenticated session cookie.
- Websocket upgrades also require the matching per-session CSRF token.
- Websocket upgrades also require a same-origin browser request.
- Long-polling command and event endpoints also require an authenticated session cookie.
- State-changing HTTP requests require both a same-origin browser request and the matching CSRF token.
- Login attempts are rate-limited in memory.
- Auth probes, command posts, event polling, and websocket upgrades are all rate-limited.
- Anonymous `/api/bootstrap` access is refused.
- Public plaintext HTTP is refused unless you explicitly pass `--allow-insecure-http`.
- When running behind a trusted tunnel or reverse proxy, `--trust-proxy` lets `vorker` honor forwarded HTTPS metadata so cookies stay `Secure` and origin checks stay strict.

For phone access on a local network or the internet, use TLS.

## TLS / WSS

Generate a local development certificate:

```bash
npm run cert
```

That writes:

- `certs/dev-cert.pem`
- `certs/dev-key.pem`

Then start the server with HTTPS/WSS:

```bash
VORKER_PASSWORD=your-password npm run serve -- --host 0.0.0.0 --tls-key certs/dev-key.pem --tls-cert certs/dev-cert.pem
```

For real internet exposure, put this behind a proper reverse proxy or tunnel that terminates trusted TLS.

If you use a tunnel that terminates TLS before reaching `vorker`, run the server with `--trust-proxy` so `vorker` treats the external request as HTTPS.

## Server options

```bash
vorker serve \
  --host 127.0.0.1 \
  --port 4173 \
  --tls-key certs/dev-key.pem \
  --tls-cert certs/dev-cert.pem
```

Options:

- `--cwd <path>` sets the default workspace for new agents
- `--copilot-bin <path>` points to a custom Copilot binary
- `--host <host>` sets the bind address
- `--port <port>` sets the listen port
- `--tls-key <path>` enables HTTPS/WSS with the given private key
- `--tls-cert <path>` enables HTTPS/WSS with the given certificate
- `--trust-proxy` trusts forwarded HTTPS metadata from a local reverse proxy or tunnel
- `--allow-insecure-http` allows a public bind without TLS
- `--cloudflared-bin <path>` points `vorker share` at a custom `cloudflared` binary
- `--cloudflared-protocol <name>` sets the Cloudflare edge protocol for `vorker share` (`http2` by default)
- `--cloudflared-edge-ip-version <mode>` sets the Cloudflare edge IP family for `vorker share` (`auto` by default)

## Architecture

- [src/supervisor/events.js](./src/supervisor/events.js), [src/supervisor/store.js](./src/supervisor/store.js), and [src/supervisor/service.js](./src/supervisor/service.js) define the shared event model, reducer, and runtime bridge
- [src/supervisor/bootstrap.js](./src/supervisor/bootstrap.js) restores durable run/task state from the supervisor NDJSON log
- [src/tui.js](./src/tui.js) and [src/tui](./src/tui) implement the terminal dashboard and command surface
- [src/git/task-workspace.js](./src/git/task-workspace.js) manages per-task git worktrees and branch allocation
- [src/copilot.js](./src/copilot.js) contains the reusable ACP session and agent manager
- [src/cli.js](./src/cli.js) contains the local REPL and one-shot chat flow
- [src/server.js](./src/server.js) contains the authenticated HTTP/WebSocket server
- [src/orchestrator.js](./src/orchestrator.js) contains run/task orchestration on top of Copilot agents
- [src/skills.js](./src/skills.js) discovers local skills from `.agents/skills`, `.github/skills`, and `$CODEX_HOME/skills`
- [src/tunnel.js](./src/tunnel.js) manages in-process Cloudflare Quick Tunnel lifecycle
- [src/share.js](./src/share.js) contains the Cloudflare Quick Tunnel share flow
- [app/page.jsx](./app/page.jsx), [components/control-plane.jsx](./components/control-plane.jsx), and [components/agent-graph.jsx](./components/agent-graph.jsx) implement the responsive orchestration dashboard and live graph

## Notes

- The wrapper now works end-to-end for local CLI and remote websocket prompts.
- Polling mode works end-to-end for remote tunneled prompts and approvals.
- The bare `copilot` binary on this machine still needs extra CA configuration if you run it directly outside `vorker`.
- `vorker share` requires `cloudflared`; if it is missing, the command fails cleanly without leaving the local server running.
