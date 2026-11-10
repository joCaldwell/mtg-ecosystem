#!/usr/bin/env node
// The stdio shim (spec §10.1), for clients that only launch stdio servers.
//
// It is a pipe, not a second server: each JSON-RPC message read from stdin is
// POSTed to the running app's agent endpoint and the reply is written to
// stdout. The app stays the one process that owns the database. This works
// message by message because the endpoint is stateless — it holds nothing
// between requests for the shim to preserve.
//
//   node src/mcp/stdio.ts --token-file <private-file> [--name <label>]
//
// stdio transport framing: one JSON-RPC message per line, no embedded
// newlines (https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/stdio).

import { createInterface } from "node:readline";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { DATA_DIR } from "../db.ts";
import { getMode } from "../mode.ts";
import { MCP_PATH } from "./http.ts";

const arg = (key: string) => { const i = process.argv.indexOf(key); return i < 0 ? undefined : process.argv[i + 1]; };
const readToken = () => {
  const file = arg("--token-file") || process.env.DECKBUILDER_TOKEN_FILE ||
    (getMode() === "local" ? join(DATA_DIR, "stdio-token") : undefined);
  if (!file) return undefined;
  try { return readFileSync(file, "utf8").trim(); }
  catch (e: any) { if (e.code === "ENOENT") return undefined; throw e; }
};
try {
  if (process.argv.includes("--token")) throw new Error("--token is not supported; save the secret privately and use --token-file <file>.");
  if (process.argv.includes("--token-file") && !arg("--token-file")) throw new Error("--token-file needs a file path");
  if (!readToken()) throw new Error("No agent token is configured. Create one in Settings, save it privately and pass --token-file <file>.");
} catch (e) { console.error((e as Error).message); process.exit(1); }

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
  if (!message || typeof message !== "object" || Array.isArray(message))
    return write({ jsonrpc: "2.0", id: null, error: { code: -32600, message: "Expected one JSON-RPC message" } });
  const first = message;
  const expectsReply = "id" in first && "method" in first;
  const fail = (message: string) => {
    if (expectsReply) write({ jsonrpc: "2.0", id: first.id, error: { code: -32000, message } });
  };

  try {
    const token = readToken();
    if (!token) return fail("No agent token is configured; pass --token-file <file>");
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
      return fail(`Could not reach the deck builder on port ${PORT}. Check the app, then read the current state before retrying a write.`);
    }

    const body = await res.text();
    if (!res.ok) {
      // HTTP auth/rate-limit errors have id:null because they precede RPC
      // dispatch. Restore this request's id so the stdio caller can recover.
      let reason = `The deck builder returned HTTP ${res.status}.`;
      try { const error = JSON.parse(body)?.error; if (typeof error?.message === "string") reason = error.message; } catch { /* a proxy may return HTML */ }
      return fail(reason);
    }
    if (!body.trim()) {
      if (expectsReply) fail("The deck builder returned an empty response. Read the current state before retrying a write.");
      return;
    }
    const messages = (res.headers.get("content-type") ?? "").includes("text/event-stream")
      ? body
          .split(/\r?\n/)
          .filter((l) => l.startsWith("data:"))
          .map((l) => l.slice(5).trim())
          .filter(Boolean)
      : [body.trim()];
    let replied = false;
    for (const m of messages) {
      try {
        const parsed = JSON.parse(m);
        if (first?.method === "initialize" && parsed?.result?.protocolVersion)
          negotiated = parsed.result.protocolVersion;
        // Re-serialized so a pretty-printed body can never break line framing.
        write(parsed);
        if (parsed?.id === first.id && ("result" in parsed || "error" in parsed)) replied = true;
      } catch {
        /* not a JSON-RPC message; nothing a stdio client could use */
      }
    }
    if (expectsReply && !replied) fail("The deck builder returned no matching response. Read the current state before retrying a write.");
  } catch {
    // A removed/unreadable token file or interrupted response must not kill
    // the pipe or silently abandon the client's outstanding request.
    fail("Could not complete the request. Check the token file and app, then read the current state before retrying a write.");
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
