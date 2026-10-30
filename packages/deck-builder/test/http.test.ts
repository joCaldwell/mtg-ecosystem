import { createToken } from "../src/tokens.ts";
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
