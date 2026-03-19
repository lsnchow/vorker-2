#!/usr/bin/env node

import path from "node:path";
import process from "node:process";
import { parseArgs } from "node:util";
import { runChat, runRepl } from "./cli.js";
import { startRemoteServer } from "./server.js";
import { runShare } from "./share.js";
import { runTui } from "./tui.js";

function printUsage() {
  console.log(`vorker

Usage:
  vorker tui [options]
  vorker repl [options]
  vorker chat [options] "<prompt>"
  vorker serve [options]
  vorker share [options]
  vorker help

Shared options:
  --cwd <path>           Working directory for Copilot sessions
  --copilot-bin <path>   Copilot CLI binary to launch (default: copilot)
  --mode <id>            Set an ACP session mode after startup
  --model <id>           Set an ACP model after startup
  --auto-approve         Auto-select the most permissive tool approval option
  --debug                Print extra ACP status updates

Server options:
  --host <host>          Bind address for the web server (default: 127.0.0.1)
  --port <port>          Port for the web server (default: 4173)
  --tls-key <path>       TLS private key for HTTPS/WSS
  --tls-cert <path>      TLS certificate for HTTPS/WSS
  --trust-proxy          Trust forwarded proto headers from a reverse proxy/tunnel
  --allow-insecure-http  Allow binding a public interface without TLS

Share options:
  --cloudflared-bin <path>  Cloudflared binary to launch (default: cloudflared)
  --cloudflared-protocol <name>  Cloudflared edge protocol for share (default: http2)
  --cloudflared-edge-ip-version <mode>  Cloudflared edge IP mode for share (default: auto)

Security:
  Use VORKER_PASSWORD to set the phone/webapp login password.
  If omitted, a random pairing password is generated at startup.

Examples:
  vorker tui
  vorker repl
  vorker chat "summarize this repo"
  VORKER_PASSWORD=secret vorker serve --host 127.0.0.1 --port 4173
  VORKER_PASSWORD=secret vorker share
  VORKER_PASSWORD=secret vorker serve --host 0.0.0.0 --tls-key certs/dev-key.pem --tls-cert certs/dev-cert.pem
`);
}

function parseCli(argv) {
  const { values, positionals } = parseArgs({
    args: argv,
    allowPositionals: true,
    options: {
      cwd: { type: "string" },
      "copilot-bin": { type: "string" },
      mode: { type: "string" },
      model: { type: "string" },
      "auto-approve": { type: "boolean", default: false },
      debug: { type: "boolean", default: false },
      host: { type: "string" },
      port: { type: "string" },
      "tls-key": { type: "string" },
      "tls-cert": { type: "string" },
      "trust-proxy": { type: "boolean", default: false },
      "allow-insecure-http": { type: "boolean", default: false },
      "cloudflared-bin": { type: "string" },
      "cloudflared-protocol": { type: "string" },
      "cloudflared-edge-ip-version": { type: "string" },
      help: { type: "boolean", short: "h", default: false },
    },
  });

  const [command = "repl", ...promptParts] = positionals;

  return {
    command,
    promptParts,
    cwd: path.resolve(values.cwd ?? process.cwd()),
    copilotBin: values["copilot-bin"] ?? process.env.COPILOT_BIN ?? "copilot",
    mode: values.mode ?? null,
    model: values.model ?? null,
    autoApprove: values["auto-approve"],
    debug: values.debug,
    host: values.host ?? "127.0.0.1",
    port: values.port ?? "4173",
    tlsKey: values["tls-key"] ?? null,
    tlsCert: values["tls-cert"] ?? null,
    trustProxy: values["trust-proxy"],
    allowInsecureHttp: values["allow-insecure-http"],
    cloudflaredBin: values["cloudflared-bin"] ?? process.env.CLOUDFLARED_BIN ?? "cloudflared",
    cloudflaredProtocol: values["cloudflared-protocol"] ?? process.env.CLOUDFLARED_PROTOCOL ?? "http2",
    cloudflaredEdgeIpVersion:
      values["cloudflared-edge-ip-version"] ?? process.env.CLOUDFLARED_EDGE_IP_VERSION ?? "auto",
    help: values.help,
  };
}

function formatError(error) {
  if (error && typeof error === "object") {
    const message = "message" in error ? String(error.message) : null;
    const code = "code" in error ? String(error.code) : null;
    if (code && message) {
      return `${message} (code: ${code})`;
    }
    if (message) {
      return message;
    }
  }

  return String(error);
}

async function main() {
  const options = parseCli(process.argv.slice(2));

  if (options.help || options.command === "help") {
    printUsage();
    return;
  }

  if (options.command === "chat") {
    await runChat(options);
    return;
  }

  if (options.command === "tui") {
    await runTui(options);
    return;
  }

  if (options.command === "repl") {
    await runRepl(options);
    return;
  }

  if (options.command === "serve") {
    await startRemoteServer(options);
    return;
  }

  if (options.command === "share") {
    await runShare(options);
    return;
  }

  throw new Error(`Unknown command: ${options.command}`);
}

main().catch((error) => {
  process.stderr.write(`${formatError(error)}\n`);
  if (String(error).includes("ENOENT")) {
    process.stderr.write(
      "Install GitHub Copilot CLI first, then rerun. You can also pass --copilot-bin to point at a custom binary.\n",
    );
  }
  process.exitCode = 1;
});
