import { spawn } from "node:child_process";
import { once } from "node:events";
import { createToken, revokeToken } from "../src/tokens.ts";
import { agentConnections } from "../src/agent-status.ts";
import { uiPreferences } from "../src/preferences.ts";
import { OWNER } from "./actors.ts";
import { test, describe, before, after } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { AddressInfo } from "node:net";
import { Client, StreamableHTTPClientTransport } from "@modelcontextprotocol/client";
import { createApp } from "../src/app.ts";
import { INSTRUCTIONS } from "../src/mcp/playbook.ts";
import { add, newDeck, testDb } from "./helpers.ts";

const db = testDb();
const TOKEN = createToken(db, OWNER, "agent").token;
const tokenDir = mkdtempSync(join(tmpdir(), "http-token-"));
const tokenFile = join(tokenDir, "token");
writeFileSync(tokenFile, TOKEN, {mode:0o600});
const deck = newDeck(db, "paper", "Served deck");
add(db, deck, "id-teferi", { role: "commander" });

const app = createApp(db, {});
let base = "";

before(async () => {
  await new Promise<void>((resolve) => app.server.listen(0, "127.0.0.1", resolve));
  base = `http://127.0.0.1:${(app.server.address() as AddressInfo).port}`;
});
after(async () => { await app.close(); rmSync(tokenDir, { recursive:true, force:true }); });

// The official client speaks the 2025 protocol unless told otherwise, as most
// deployed clients still do; `modern` pins it to the 2026-07-28 revision.
async function connect(path = "/mcp", modern = false) {
  const client = new Client(
    { name: "test-client", version: "1.0.0" },
    modern ? { versionNegotiation: { mode: { pin: "2026-07-28" } } } : {},
  );
  await client.connect(
    new StreamableHTTPClientTransport(new URL(`${base}${path}`), {
      requestInit: { headers: { Authorization: `Bearer ${TOKEN}` } },
    }),
  );
  return client;
}

// A response may be plain JSON or a one-message event stream.

const textOf = (result: any) => result.content.map((c: any) => c.text).join("\n");

describe("the agent endpoint, through the official MCP client (spec §10.1)", () => {
  test("both protocol eras negotiate, and see the same tools", async () => {
    const legacy = await connect();
    const modern = await connect("/mcp", true);
    assert.equal(legacy.getNegotiatedProtocolVersion(), "2025-11-25");
    assert.equal(modern.getNegotiatedProtocolVersion(), "2026-07-28");
    assert.deepEqual(
      (await modern.listTools()).tools.map((t) => t.name),
      (await legacy.listTools()).tools.map((t) => t.name),
    );
    assert.equal(modern.getInstructions(), INSTRUCTIONS);
    const session = await modern.callTool({ name: "start_session", arguments: {} });
    assert.match(textOf(session), /# Playbook/);
    await legacy.close();
    await modern.close();
  });
});


test("read-only MCP contact is visible without claiming a write; revoked tokens disappear", async () => {
  const credential = createToken(db, OWNER, "read-only audit");
  const client = new Client({ name: "reader", version: "1" });
  try {
    assert.equal(agentConnections(db, OWNER).find(c => c.label === "read-only audit")?.last_used_at, null);
    await client.connect(new StreamableHTTPClientTransport(new URL(`${base}/mcp`), {
      requestInit: { headers: { Authorization: `Bearer ${credential.token}` } },
    }));
    const session = await client.callTool({ name: "start_session", arguments: {} });
    assert.equal(session.isError, undefined);
    assert.equal(uiPreferences(db, OWNER).has_agent_activity, false);
    const response = await fetch(`${base}/api/agents`);
    const connections = await response.json() as ReturnType<typeof agentConnections>;
    assert.ok(connections.find(c => c.label === "read-only audit")?.last_used_at);
    assert.ok(!JSON.stringify(connections).includes(credential.token));
    revokeToken(db, OWNER, credential.credential.id);
    assert.ok(!agentConnections(db, OWNER).some(c => c.label === "read-only audit"));
  } finally { await client.close(); }
});


test("stdio returns authentication failures to the original request", async () => {
  const file = join(tokenDir, "revoked");
  const credential = createToken(db, OWNER, "revoked shim");
  writeFileSync(file, credential.token, { mode: 0o600 });
  revokeToken(db, OWNER, credential.credential.id);
  const child = spawn(process.execPath, [new URL("../src/mcp/stdio.ts", import.meta.url).pathname, "--token-file", file], {
    env: { ...process.env, DECKBUILDER_MODE: "local", DECKBUILDER_PORT: new URL(base).port },
  });
  let stdout = "", stderr = "";
  child.stdout.on("data", data => { stdout += data; });
  child.stderr.on("data", data => { stderr += data; });
  const timeout = setTimeout(() => child.kill(), 5000);
  try {
    child.stdin.end(JSON.stringify({ jsonrpc: "2.0", id: 42, method: "tools/list", params: {} }) + "\n");
    const [code] = await once(child, "exit");
    assert.equal(code, 0, stderr);
    const response = JSON.parse(stdout.trim());
    assert.equal(response.id, 42);
    assert.match(response.error.message, /token/i);
  } finally { clearTimeout(timeout); child.kill(); }
});

test("UI commands replay safely and MCP retries recheck revoked credentials", async () => {
  const headers = { "Content-Type": "application/json", "Idempotency-Key": "http-create" };
  const body = JSON.stringify({ name: "Retry-safe UI deck" });
  const write = () => fetch(`${base}/api/decks`, { method: "POST", headers, body });
  const first = await (await write()).json() as any;
  const second = await (await write()).json() as any;
  assert.equal(second.state.deck.id, first.state.deck.id);
  assert.equal(second.command.replayed, true);
  assert.ok(first.command.resources.includes("decks"));
  const conflict = await fetch(`${base}/api/decks/${first.state.deck.id}`, { method: "PATCH", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ name: "Stale rename", expected_edit_version: 999 }) });
  assert.equal(conflict.status, 409);
  const credential = createToken(db, OWNER, "write retry");
  const client = new Client({ name: "write-retry", version: "1" });
  try {
    await client.connect(new StreamableHTTPClientTransport(new URL(`${base}/mcp`), { requestInit: { headers: { Authorization: `Bearer ${credential.token}` } } }));
    const args = { deck: first.state.deck.id, add: [{ card: "id-forest" }], idempotency_key: "write-once" };
    const result = await client.callTool({ name: "edit_cards", arguments: args });
    assert.equal((result.structuredContent as any).command.decks[0].computed.card_count, 1);
    assert.equal(((await client.callTool({ name: "edit_cards", arguments: args })).structuredContent as any).command.replayed, true);
    revokeToken(db, OWNER, credential.credential.id);
    await assert.rejects(() => client.callTool({ name: "edit_cards", arguments: args }), /401|revoked|token/i);
  } finally { await client.close(); }
});
