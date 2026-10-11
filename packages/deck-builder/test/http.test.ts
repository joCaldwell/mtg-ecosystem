import { test, describe, before, after } from "node:test";
import assert from "node:assert/strict";
import { request } from "node:http";
import type { AddressInfo } from "node:net";
import { Client, StreamableHTTPClientTransport } from "@modelcontextprotocol/client";
import { StdioClientTransport } from "@modelcontextprotocol/client/stdio";
import { createApp } from "../src/app.ts";
import { listActivity } from "../src/activity.ts";
import { agentName } from "../src/mcp/http.ts";
import { INSTRUCTIONS } from "../src/mcp/playbook.ts";
import { TOOLS } from "../src/mcp/tools.ts";
import { add, newDeck, testDb } from "./helpers.ts";

const TOKEN = "test-token-0123456789";
const db = testDb();
const deck = newDeck(db, "active", "Served deck");
add(db, deck, "id-teferi", { role: "commander" });

const app = createApp(db, { token: TOKEN });
let base = "";

before(async () => {
  await new Promise<void>((resolve) => app.server.listen(0, "127.0.0.1", resolve));
  base = `http://127.0.0.1:${(app.server.address() as AddressInfo).port}`;
});
after(() => app.close());

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
async function rpcResult(res: Response): Promise<any> {
  const body = await res.text();
  const data = body.startsWith("{") ? body : body.split("\n").find((l) => l.startsWith("data: "))!.slice(6);
  return JSON.parse(data);
}

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
  test("lists every tool in a fixed order, with honest annotations and the instructions", async () => {
    const client = await connect();
    const { tools } = await client.listTools();
    assert.deepEqual(tools.map((t) => t.name), TOOLS.map((t) => t.name));
    const byName = Object.fromEntries(tools.map((t) => [t.name, t]));
    assert.equal(byName.get_deck.annotations?.readOnlyHint, true);
    assert.equal(byName.remember.annotations?.readOnlyHint, false);
    assert.deepEqual(tools.filter((t) => t.annotations?.destructiveHint).map((t) => t.name), ["delete_deck"]);
    assert.equal(byName.delete_deck.annotations?.readOnlyHint, false);
    assert.deepEqual((byName.get_deck.inputSchema as any).required, ["deck"]);
    assert.match((byName.search_cards.inputSchema as any).properties.include_my_decks.description, /Off by default/);
    assert.equal(client.getInstructions(), INSTRUCTIONS);
    await client.close();
  });
  test("a read returns text; a failure is a tool error the agent can act on", async () => {
    const client = await connect();
    const session = await client.callTool({ name: "start_session", arguments: {} });
    assert.match(textOf(session), /# Playbook[\s\S]*- deck \d+ "Served deck"/);
    assert.ok(!session.isError);

    const failed = await client.callTool({ name: "get_cards", arguments: { cards: [] } });
    assert.equal(failed.isError, true);

    const refused = await client.callTool({
      name: "edit_draft",
      arguments: { deck, add: [{ card: "Sol Ring" }] },
    });
    assert.equal(refused.isError, true);
    assert.match(textOf(refused), /Use propose_changes/);

    const unknown = await client.callTool({
      name: "propose_changes",
      arguments: { deck, items: [{ action: "add", card: "Sol Ringz", reason: "x" }] },
    });
    assert.equal(unknown.isError, true);
    assert.match(textOf(unknown), /Did you mean: Sol Ring/);
    await client.close();
  });
  test("an agent can create, rename, classify and delete through the official client", async () => {
    const client = await connect("/mcp/deck-manager", true);
    const run = async (name: string, args: Record<string, unknown>) => {
      const result = await client.callTool({ name, arguments: args });
      assert.ok(!result.isError, textOf(result));
      return textOf(result);
    };
    await run("create_deck", { name: "MCP managed deck", category: "paper" });
    await run("rename_deck", { deck: "MCP managed deck", name: "MCP renamed deck" });
    await run("set_deck_category", { deck: "MCP renamed deck", category: "legacy" });
    assert.match(await run("get_deck", { deck: "MCP renamed deck", sections: ["state"] }), /category legacy/);
    assert.match(await run("delete_deck", { deck: "MCP renamed deck" }), /This cannot be undone/);
    assert.equal(listActivity(db)[0].actor, "deck-manager");
    assert.equal(listActivity(db)[0].action, "delete_deck");
    await client.close();
  });
  test("a write is attributed to the client's label and reaches the open page", async () => {
    // The page's event stream.
    const events = await fetch(`${base}/api/events`);
    const reader = events.body!.getReader();
    const first = new TextDecoder().decode((await reader.read()).value);
    assert.match(first, /^data: \d+\n\n$/);

    const client = await connect("/mcp/claude-code");
    const saved = await client.callTool({
      name: "remember",
      arguments: { text: "Likes [[Counterspell]] effects.", scope: "global" },
    });
    assert.match(textOf(saved), /Saved\./);
    const pushed = new TextDecoder().decode((await reader.read()).value);
    assert.match(pushed, /^data: \d+\n\n$/);
    assert.notEqual(pushed, first);
    await reader.cancel();

    assert.equal(listActivity(db)[0].actor, "claude-code");
    await client.close();
  });
  // A 2026-07-28 client names itself on every request; a 2025 client only
  // did so in its handshake, which a stateless server does not keep, so it is
  // attributed by the URL label or falls back to "agent".
  test("attribution without a label depends on the protocol era", async () => {
    const write = { name: "mark_rulings_reviewed", arguments: { through: 0 } };
    const modern = await connect("/mcp", true);
    await modern.callTool(write);
    assert.equal(listActivity(db)[0].actor, "test-client");
    const legacy = await connect();
    await legacy.callTool(write);
    assert.equal(listActivity(db)[0].actor, "agent");
    await modern.close();
    await legacy.close();
  });
});

const rpc = (body: unknown, headers: Record<string, string> = {}) =>
  fetch(`${base}/mcp`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Accept: "application/json, text/event-stream",
      Authorization: `Bearer ${TOKEN}`,
      ...headers,
    },
    body: JSON.stringify(body),
  });

describe("who may reach the agent endpoint", () => {
  const list = { jsonrpc: "2.0", id: 1, method: "tools/list", params: {} };
  test("no token or a wrong one is refused", async () => {
    for (const auth of ["", "Bearer nope", `Basic ${TOKEN}`]) {
      const res = await rpc(list, { Authorization: auth });
      assert.equal(res.status, 401, auth);
      assert.equal(res.headers.get("www-authenticate"), "Bearer");
      assert.match((await res.json()).error.message, /Settings page/);
    }
  });
  // A page in the owner's browser can reach a loopback port. With the right
  // token it would still be a cross-site request, and is refused as one.
  test("a web page elsewhere is refused even with the token", async () => {
    assert.equal((await rpc(list, { Origin: "https://evil.example" })).status, 403);
    assert.equal((await rpc(list, { Origin: "http://localhost:5173" })).status < 400, true);
  });
  test("a client speaking the 2025 protocol is served by the same endpoint", async () => {
    const init = await rpc({
      jsonrpc: "2.0",
      id: 1,
      method: "initialize",
      params: {
        protocolVersion: "2025-06-18",
        capabilities: {},
        clientInfo: { name: "old-client", version: "0.1" },
      },
    });
    assert.equal(init.status, 200);
    const initBody = await rpcResult(init);
    assert.equal(initBody.result.protocolVersion, "2025-06-18");
    assert.equal(initBody.result.instructions, INSTRUCTIONS);
    // No session is issued: the next request stands on its own.
    assert.equal(init.headers.get("mcp-session-id"), null);

    const tools = await rpc(list, { "MCP-Protocol-Version": "2025-06-18" });
    assert.equal((await rpcResult(tools)).result.tools.length, TOOLS.length);
  });
});

describe("the server instructions", () => {
  // Codex documents that the first 512 characters are what it can rely on.
  test("fit in 512 characters and send the agent to start_session", () => {
    assert.ok(INSTRUCTIONS.length <= 512, `${INSTRUCTIONS.length} characters`);
    assert.match(INSTRUCTIONS, /Call start_session first/);
  });
});

describe("naming the caller, for attribution only", () => {
  test("the owner's label in the URL wins, then the client's own name, then its User-Agent", () => {
    const meta = { params: { _meta: { "io.modelcontextprotocol/clientInfo": { name: "Claude Code" } } } };
    assert.equal(agentName("/mcp/my-laptop", meta, "x/1"), "my-laptop");
    assert.equal(agentName("/mcp", meta, "x/1"), "Claude Code");
    assert.equal(agentName("/mcp", { params: { clientInfo: { name: "Cursor" } } }, undefined), "Cursor");
    assert.equal(agentName("/mcp", { params: {} }, "claude-code/2.1 (cli)"), "claude-code");
    assert.equal(agentName("/mcp", {}, "node"), "agent");
    // What Codex 0.162 sends when its URL carries no label.
    assert.equal(agentName("/mcp", { params: {} }, "codex-mcp-client/0.162.1"), "codex-mcp-client");
    assert.equal(agentName("/mcp/<script>", {}, undefined), "script");
  });
});

describe("the UI's API", () => {
  const api = (path: string, init: RequestInit = {}) => fetch(`${base}${path}`, init);
  const post = (path: string, body: unknown, headers: Record<string, string> = {}) =>
    api(path, { method: "POST", headers: { "Content-Type": "application/json", ...headers }, body: JSON.stringify(body) });

  test("power target approval and assessments work across MCP and HTTP with revision checks", async () => {
    const created = await (await post("/api/decks", { name: "Power endpoint deck" })).json();
    const id = created.state.deck.id;
    const patch = await api(`/api/decks/${id}`, { method: "PATCH", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ status: "active" }) });
    assert.equal(patch.status, 200);
    const client = await connect("/mcp/power-test");
    const proposed = await client.callTool({ name: "propose_power_target", arguments: { deck: id, target: { profile: { intent: "synergy" }, game_changers_max: 3 }, reason: "Owner requested powered-up casual play." } });
    assert.ok(!proposed.isError, textOf(proposed));
    assert.match(textOf(proposed), /awaiting the owner/);
    const before = await (await api(`/api/decks/${id}`)).json();
    assert.equal(before.state.power.target, null);
    const edit = before.state.document_edits.find((e: any) => e.kind === "power_target");
    const ruled = await (await post(`/api/document-edits/${edit.id}/rule`, { outcome: "accept" })).json();
    assert.equal(ruled.state.power.target.mapping.bracket, 3);
    const input = { profile: { intent: "social", win_turn: { min: 9, max: 11 } }, revision: 0, evidence: "list", confidence: "tentative", notes: "Untested list estimate." };
    const assessed = await client.callTool({ name: "assess_deck_power", arguments: { deck: id, assessment: input } });
    assert.ok(!assessed.isError, textOf(assessed));
    assert.match(textOf(assessed), /tentative, evidence: list/);
    const owner = await (await post(`/api/decks/${id}/power/assessments`, input)).json();
    assert.equal(owner.state.power.assessment.owner_touched, true);
    const blocked = await client.callTool({ name: "assess_deck_power", arguments: { deck: id, assessment: input } });
    assert.equal(blocked.isError, true);
    assert.match(textOf(blocked), /owner has assessed this revision/);
    const put = (target: unknown) => api(`/api/decks/${id}/power/target`, { method: "PUT", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ target }) });
    const invalid = await put({ profile: { win_turn: { min: 9, max: 6 } } });
    assert.equal(invalid.status, 422);
    assert.equal((await (await put(null)).json()).state.power.target, null);
    await post(`/api/decks/${id}/cards`, { oracle_id: "id-solring" });
    const stale = await post(`/api/decks/${id}/power/assessments`, input);
    assert.equal(stale.status, 409);
    const read = await client.callTool({ name: "get_deck", arguments: { deck: id, sections: ["power"] } });
    assert.match(textOf(read), /STALE: cards changed/);
    await client.close();
  });

  test("creates, updates and lists deck categories; rejects invalid values atomically", async () => {
    const created = await (await post("/api/decks", { name: "Paper deck", category: "paper" })).json();
    const id = created.state.deck.id;
    assert.equal(created.state.deck.category, "paper");
    assert.equal(created.state.deck.status, "draft");
    const patch = (body: unknown) => api(`/api/decks/${id}`, {
      method: "PATCH", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body),
    });
    const updated = await (await patch({ category: "legacy" })).json();
    assert.equal(updated.state.deck.category, "legacy");
    assert.equal((await (await api(`/api/decks/${id}`)).json()).state.deck.category, "legacy");
    assert.equal((await (await api("/api/decks")).json()).find((d: any) => d.id === id).category, "legacy");
    const invalid = await patch({ name: "Should roll back", category: "unknown" });
    assert.equal(invalid.status, 400);
    assert.match((await invalid.json()).error, /Deck category must be/);
    const unchanged = await (await api(`/api/decks/${id}`)).json();
    assert.equal(unchanged.state.deck.name, "Paper deck");
    assert.equal(unchanged.state.deck.category, "legacy");
    for (const category of [null, "unknown", 1]) {
      assert.equal((await post("/api/decks", { name: "Invalid category", category })).status, 400);
    }
    assert.ok(!(await (await api("/api/decks")).json()).some((d: any) => d.name === "Invalid category"));
    const defaulted = await (await post("/api/decks", { name: "Default category" })).json();
    assert.equal(defaulted.state.deck.category, "theorycrafted");
  });

  test("acts as the owner: adds a card, rules on a proposal, undoes it", async () => {
    const added = await (await post(`/api/decks/${deck}/cards`, { oracle_id: "id-counterspell" })).json();
    assert.equal(added.state.computed.card_count, 2);
    assert.equal(added.state.deck.status, "active");
    assert.ok(Array.isArray(added.state.checks));

    const client = await connect();
    await client.callTool({
      name: "propose_changes",
      arguments: { deck, items: [{ action: "add", card: "Sol Ring", reason: "Ramp." }] },
    });
    await client.close();
    const inbox = await (await api("/api/inbox")).json();
    assert.equal(inbox.proposals.length, 1);
    assert.equal(inbox.proposals[0].author, "agent");

    const ruled = await (
      await post(`/api/decks/${deck}/proposals/${inbox.proposals[0].id}/rule`, { outcome: "accept", reason: "Fine." })
    ).json();
    assert.equal(ruled.state.computed.card_count, 3);
    assert.equal(ruled.inbox.proposals.length, 0);
    assert.equal(ruled.state.log[0].ruling_reason, "Fine.");

    const undone = await (await post(`/api/decks/${deck}/log/${ruled.state.log[0].id}/undo`, {})).json();
    assert.equal(undone.state.computed.card_count, 2);
    assert.deepEqual(undone.skipped, []);
  });
  test("errors carry their message and status", async () => {
    const res = await post(`/api/decks/${deck}/cards`, { oracle_id: "id-nope" });
    assert.equal(res.status, 404);
    assert.match((await res.json()).error, /Unknown oracle_id/);
    assert.equal((await api("/api/nope")).status, 404);
  });
  test("a cross-site page cannot call it", async () => {
    const res = await post(`/api/decks`, { name: "Injected" }, { Origin: "https://evil.example" });
    assert.equal(res.status, 403);
    // DNS rebinding: the request arrives on loopback but names another host.
    const rebind = await new Promise<number>((resolve, reject) => {
      const port = (app.server.address() as AddressInfo).port;
      const req = request({ host: "127.0.0.1", port, path: "/api/decks", headers: { Host: "evil.example" } }, (res) => {
        res.resume();
        resolve(res.statusCode ?? 0);
      });
      req.on("error", reject);
      req.end();
    });
    assert.equal(rebind, 403);
  });
  test("settings show how to connect an agent", async () => {
    const settings = await (await api("/api/settings")).json();
    assert.equal(settings.agent.token, TOKEN);
    assert.match(settings.agent.url, /^http:\/\/127\.0\.0\.1:\d+\/mcp$/);
    assert.match(settings.agent.stdio_command, /^node \/.*\/src\/mcp\/stdio\.ts$/);
    assert.deepEqual(settings.budgets, { global: 1500, deck: 1000 });
  });
});

describe("the stdio shim (spec §10.1)", () => {
  const shim = (extraEnv: Record<string, string> = {}) =>
    new StdioClientTransport({
      command: process.execPath,
      args: ["src/mcp/stdio.ts", "--name", "desktop-app"],
      env: {
        ...(process.env as Record<string, string>),
        DECKBUILDER_PORT: String((app.server.address() as AddressInfo).port),
        DECKBUILDER_TOKEN: TOKEN,
        ...extraEnv,
      },
    });
  test("a stdio client reaches the running app through it, under its label", async () => {
    const client = new Client({ name: "stdio-client", version: "1.0.0" });
    await client.connect(shim());
    assert.equal((await client.listTools()).tools.length, TOOLS.length);
    const result = await client.callTool({
      name: "remember",
      arguments: { text: "Came in over stdio.", scope: "global" },
    });
    assert.match(textOf(result), /Saved\./);
    assert.equal(listActivity(db)[0].actor, "desktop-app");
    await client.close();
  });
  test("when the app is not running it says so instead of hanging", async () => {
    const client = new Client({ name: "stdio-client", version: "1.0.0" });
    await assert.rejects(
      client.connect(shim({ DECKBUILDER_PORT: "1" })),
      /The deck builder app is not running on port 1/,
    );
  });
});
