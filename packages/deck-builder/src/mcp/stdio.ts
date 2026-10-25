#!/usr/bin/env node
// The stdio shim (spec §10.1), for clients that only launch stdio servers.
//
// It is a pipe, not a second server: each JSON-RPC message read from stdin is
// POSTed to the running app's agent endpoint and the reply is written to
// stdout. The app stays the one process that owns the database. This works
// message by message because the endpoint is stateless — it holds nothing
// between requests for the shim to preserve.
//
//   node src/mcp/stdio.ts [--name <label>]
//
// stdio transport framing: one JSON-RPC message per line, no embedded
// newlines (https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/stdio).

import { createInterface } from "node:readline";
import { getAgentToken } from "../token.ts";
import { MCP_PATH } from "./http.ts";

if (!getAgentToken()) {
  console.error("No agent token is configured. Start the deck builder app first, or configure DECKBUILDER_TOKEN.");
  process.exit(1);
}

const PORT = Number(process.env.DECKBUILDER_PORT ?? 8787);
const nameArg = process.argv.indexOf("--name");
const label = nameArg > -1 ? process.argv[nameArg + 1] : "";
const ENDPOINT = `http://127.0.0.1:${PORT}${MCP_PATH}${label ? `/${encodeURIComponent(label)}` : ""}`;

const write = (message: unknown): void => void process.stdout.write(`${JSON.stringify(message)}\n`);

// The 2025 protocol's handshake names the revision; later requests must carry
// it in a header (https://modelcontextprotocol.io/specification/2025-06-18/basic/transports#protocol-version-header).
// A 2026-07-28 client carries its version inside every message instead, and
// never sends initialize, so this stays unset for it.
let negotiated: string | undefined;

async function forward(line: string): Promise<void> {
  let message: any;
  try {
    message = JSON.parse(line);
  } catch {
    return write({ jsonrpc: "2.0", id: null, error: { code: -32700, message: "Parse error" } });
  }
  const first = Array.isArray(message) ? message[0] : message;
  const expectsReply = first && "id" in first && "method" in first;

  const token = getAgentToken();
  if (!token) throw new Error("No agent token is configured; start the deck builder app first");
  let res: Response;
  try {
    res = await fetch(ENDPOINT, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json, text/event-stream",
        Authorization: `Bearer ${token}`,
        ...(negotiated && first?.method !== "initialize" ? { "MCP-Protocol-Version": negotiated } : {}),
      },
      body: line,
    });
  } catch {
    if (expectsReply)
      write({
        jsonrpc: "2.0",
        id: first.id,
        error: {
          code: -32000,
          message:
            `The deck builder app is not running on port ${PORT}. ` +
            `Start it (npm start in packages/deck-builder) and try again.`,
        },
      });
    return;
  }

  const body = await res.text();
  if (!body.trim()) return; // a notification: accepted, nothing to relay
  const messages = (res.headers.get("content-type") ?? "").includes("text/event-stream")
    ? body
        .split(/\r?\n/)
        .filter((l) => l.startsWith("data:"))
        .map((l) => l.slice(5).trim())
        .filter(Boolean)
    : [body.trim()];
  for (const m of messages) {
    try {
      const parsed = JSON.parse(m);
      if (first?.method === "initialize" && parsed?.result?.protocolVersion)
        negotiated = parsed.result.protocolVersion;
      // Re-serialized so a pretty-printed body can never break line framing.
      write(parsed);
    } catch {
      /* not a JSON-RPC message; nothing a stdio client could use */
    }
  }
}

const pending = new Set<Promise<void>>();
const lines = createInterface({ input: process.stdin, crlfDelay: Infinity });
lines.on("line", (line) => {
  if (!line.trim()) return;
  const p = forward(line).finally(() => pending.delete(p));
  pending.add(p);
});
// The client closing stdin is the shutdown signal; let in-flight replies land.
lines.on("close", () => void Promise.allSettled([...pending]).then(() => process.exit(0)));
