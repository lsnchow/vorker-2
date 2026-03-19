# Debug Notes

## Goal

Get local GitHub Copilot CLI prompt execution working so the remote websocket wrapper can stream real agent responses instead of surfacing Copilot startup errors.

## What was verified

### Wrapper and transport

- Local ACP wrapper starts and creates Copilot sessions successfully.
- Remote server starts over HTTP and HTTPS.
- Login endpoint works.
- Session cookie is set and reused.
- Websocket upgrade requires authenticated cookie and same-origin request.
- Agent creation works over websocket.
- Prompt events stream back to the client.

### Copilot CLI presence

- `copilot` is installed at `/opt/homebrew/bin/copilot`
- Version shown by CLI help/logs: `1.0.5`

### Network

- `curl` to `https://api.individual.githubcopilot.com/mcp/readonly` reaches the service and gets a normal `401` without auth.
- `curl` to `https://api.github.com` works.
- Plain Node `fetch()` to `https://api.individual.githubcopilot.com` works.
- `npx node@24` `fetch()` to `https://api.individual.githubcopilot.com` also works.

That means generic DNS/TLS/connectivity to the Copilot domain is not broken on this machine.

## Copilot CLI failure observed

Copilot CLI logs show prompt execution failing before any useful model response:

- `Failed to fetch models from https://api.individual.githubcopilot.com: TypeError: fetch failed`
- `Error loading models: Error: Failed to list models`

This reproduces in:

- ACP mode
- direct CLI prompt mode
- with built-in MCP disabled

So the failure is inside Copilot CLI startup/auth/model loading rather than in the websocket wrapper.

## Auth state observed

- `~/.copilot/config.json` shows a remembered login user:
  - host: `https://github.com`
  - login: `lsnchow`
- macOS keychain shows Copilot-related entries for:
  - service: `copilot-cli`
  - account: `https://github.com:lsnchow`

However, `copilot login` still starts a device-flow login instead of cleanly proceeding as authenticated.

That suggests one of:

1. stored credentials are stale or unusable
2. Copilot can see remembered user metadata but not a valid token
3. keychain access is broken for the current token entry

## Root cause found

Running Copilot with Node network debug enabled exposed the actual failure:

- `unable to get issuer certificate; if the root CA is installed locally, try running Node.js with --use-system-ca`

This happened specifically for:

- `https://api.individual.githubcopilot.com/models`
- `https://telemetry.individual.githubcopilot.com/...`

The packaged Copilot CLI runtime on this machine is not trusting the issuer chain for the `*.individual.githubcopilot.com` endpoints by default.

## Fix applied in vorker

`vorker` now:

1. opens a trusted TLS connection to `api.individual.githubcopilot.com`
2. captures the presented certificate chain
3. writes that chain to a temporary PEM file
4. spawns Copilot with `NODE_EXTRA_CA_CERTS` pointing at that PEM

That fixes:

- local `vorker chat`
- local `vorker repl`
- remote websocket agent prompts

## Final verification

Verified after fresh login and the CA workaround:

- `npm run check:all` passes
- `node src/index.js chat 'Reply with exactly: hi'` returns `hi`
- HTTPS server starts with local certs
- authenticated WSS client can:
  - log in
  - create an agent
  - receive model and mode metadata
  - send a prompt
  - stream `hi`
  - receive `prompt_finished`

## Remote robustness fix

While testing WSS manually, a malformed `send_prompt` payload with an empty body crashed the server because the rejected `agent.prompt()` promise was not handled.

That is now fixed by:

- validating prompt text before dispatch
- swallowing the already-broadcast rejection so the process stays up

## Previous likely next fix

Complete a fresh Copilot login and then retest prompt mode immediately:

```bash
copilot login
copilot --disable-builtin-mcps -p "Reply with exactly: hi" --allow-all-tools --no-alt-screen --no-mouse
```

This turned out not to be enough by itself. Re-login succeeded, but model fetches still failed until the CA-chain workaround was added.

## If re-login does not fix it

Next checks:

1. inspect whether Copilot is reading a bad token source from env
2. test with a fresh `COPILOT_HOME`
3. inspect keychain access permissions for the `copilot-cli` item
4. try explicit token auth with `COPILOT_GITHUB_TOKEN` if available
5. compare behavior on latest Copilot CLI version after `copilot update`
