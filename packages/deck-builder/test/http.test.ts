import { createToken } from "../src/tokens.ts";
import { OWNER } from "./actors.ts";
import { test, describe, before, after } from "node:test";
import assert from "node:assert/strict";
import { request } from "node:http";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { PKG_ROOT } from "../src/db.ts";
import type { AddressInfo } from "node:net";
import { Client, StreamableHTTPClientTransport } from "@modelcontextprotocol/client";
import { StdioClientTransport } from "@modelcontextprotocol/client/stdio";
import { createApp } from "../src/app.ts";
import { listActivity } from "../src/activity.ts";
import { agentName } from "../src/mcp/http.ts";
import { INSTRUCTIONS } from "../src/mcp/playbook.ts";
import { TOOLS } from "../src/mcp/tools.ts";
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
      name: "edit_cards",
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
  test("arguments are strict: an unknown key is named, a D9 rename says what it became", async () => {
    const client = await connect("/mcp", true);
    const renamed = await client.callTool({ name: "remember", arguments: { text: "Strict arguments", scope: "global", source_ruling: 1 } });
    assert.equal(renamed.isError, true);
    assert.match(textOf(renamed), /'source_ruling' was renamed to 'source_decision'/);
    const section = await client.callTool({ name: "get_deck", arguments: { deck, sections: ["rulings"] } });
    assert.equal(section.isError, true);
    assert.match(textOf(section), /'rulings' was renamed to 'decisions'/);
    const unknown = await client.callTool({ name: "start_session", arguments: { verbose: true } });
    assert.equal(unknown.isError, true);
    assert.match(textOf(unknown), /'verbose' is not an argument of start_session/);
    // The protocol's _meta travels beside arguments, never inside them.
    const withMeta = await client.callTool({ name: "start_session", arguments: {}, _meta: { progressToken: "strict-1" } });
    assert.ok(!withMeta.isError, textOf(withMeta));
    assert.equal((db.prepare("SELECT COUNT(*) n FROM memories WHERE text = 'Strict arguments'").get() as { n: number }).n, 0);
    await client.close();
  });
  test("a connected client cannot take the sample deck's reserved label", async () => {
    const client = await connect("/mcp/sample-agent");
    const wrote = await client.callTool({ name: "remember", arguments: { text: "Reserved label check", scope: "global" } });
    assert.ok(!wrote.isError, textOf(wrote));
    assert.equal(listActivity(db, OWNER)[0].actor_label, "sample-agent (connected)");
    await client.close();
  });
  test("an agent can create, rename and delete through the official client", async () => {
    const client = await connect("/mcp/deck-manager", true);
    const run = async (name: string, args: Record<string, unknown>) => {
      const result = await client.callTool({ name, arguments: args });
      assert.ok(!result.isError, textOf(result));
      return textOf(result);
    };
    await run("create_deck", { name: "MCP managed deck" });
    await run("rename_deck", { deck: "MCP managed deck", name: "MCP renamed deck" });
    assert.match(await run("get_deck", { deck: "MCP renamed deck", sections: ["state"] }), /lifecycle theorycrafted/);
    assert.match(await run("delete_deck", { deck: "MCP renamed deck" }), /This cannot be undone/);
    assert.equal(listActivity(db, OWNER)[0].actor, "deck-manager");
    assert.equal(listActivity(db, OWNER)[0].action, "delete_deck");
    await client.close();
  });
  test("a write is attributed to the client's label and reaches the open page", async () => {
    // The page's event stream.
    const events = await fetch(`${base}/api/events`);
    const reader = events.body!.getReader();
    const first = new TextDecoder().decode((await reader.read()).value);
    assert.equal(first, 'data: {"kind":"connected"}\n\n');

    const client = await connect("/mcp/claude-code");
    const saved = await client.callTool({
      name: "remember",
      arguments: { text: "Likes [[Counterspell]] effects.", scope: "global" },
    });
    assert.match(textOf(saved), /Saved\./);
    const pushed = new TextDecoder().decode((await reader.read()).value);
    assert.equal(pushed, 'data: {"kind":"remember"}\n\n');
    assert.notEqual(pushed, first);
    await reader.cancel();

    assert.equal(listActivity(db, OWNER)[0].actor, "claude-code");
    await client.close();
  });
  // A 2026-07-28 client names itself on every request; a 2025 client only
  // did so in its handshake, which a stateless server does not keep, so it is
  // attributed by the URL label or falls back to "agent".
  test("attribution without a label depends on the protocol era", async () => {
    const write = { name: "mark_decisions_reviewed", arguments: { through: 0 } };
    const modern = await connect("/mcp", true);
    await modern.callTool(write);
    assert.equal(listActivity(db, OWNER)[0].actor, "test-client");
    const legacy = await connect();
    await legacy.callTool(write);
    assert.equal(listActivity(db, OWNER)[0].actor, "agent");
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
      assert.equal(res.headers.get("www-authenticate"), `Bearer resource_metadata="${base}/.well-known/oauth-protected-resource/mcp"${auth === "Bearer nope" ? ", error=\"invalid_token\"" : ""}`);
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

  test("exports current matches with independently optional maybe-list inclusion", async () => {
    const created = await (await post("/api/decks", { name: "Filtered export endpoint" })).json();
    const id = created.state.deck.id;
    await post(`/api/decks/${id}/cards`, { oracle_id: "id-solring" });
    const parked = await (await post(`/api/decks/${id}/cards`, { oracle_id: "id-forest", maybe: true })).json();
    const name = parked.state.cards.find((c: any) => c.oracle_id === "id-forest").name;
    const read = async (query: string) => {
      const res = await api(`/api/decks/${id}/export?${query}`);
      assert.equal(res.status, 200);
      return res.json();
    };
    assert.equal((await read("")).card_count, 1);
    assert.equal((await read("include_maybe=on")).card_count, 2);
    assert.equal((await read("oracle_ids=id-forest")).text, "");
    const filtered = await read("oracle_ids=id-forest&include_maybe=on&categories=off");
    assert.equal(filtered.text, `1 ${name}\n`);
    assert.equal(filtered.card_count, 1);
    assert.equal(filtered.line_count, 1);
    assert.equal((await read("oracle_ids=&include_maybe=on")).text, "");
    assert.equal((await read("oracle_ids=not-in-this-deck&include_maybe=on")).text, "");
    const categorized = await read("oracle_ids=id-forest&include_maybe=on");
    assert.equal(categorized.text, `1 ${name} [Maybeboard]\n`);
    const unchanged = await (await api(`/api/decks/${id}`)).json();
    assert.equal(unchanged.state.deck.revision, parked.state.deck.revision);
    assert.deepEqual(unchanged.state.cards, parked.state.cards);
  });

  test("primary and secondary purposes round-trip through HTTP and the official MCP client", async () => {
    const created = await (await post("/api/decks", { name: "Purpose endpoint deck" })).json();
    const id = created.state.deck.id;
    const client = await connect("/mcp/purposes-test");
    try {
      const slots = await client.callTool({ name: "define_slots", arguments: { deck: id, slots: [
        { name: "Ramp", target_min: 1, target_max: 1 }, { name: "Utility", target_min: 1, target_max: 1 },
      ] } });
      assert.ok(!slots.isError, textOf(slots));
      const name = (db.prepare("SELECT name FROM cards WHERE oracle_id = 'id-solring'").get() as { name: string }).name;
      const added = await client.callTool({ name: "edit_cards", arguments: { deck: id, add: [{ card: name, slot: "Ramp", secondary_slots: ["Utility"] }] } });
      assert.ok(!added.isError, textOf(added));
      assert.match(textOf(added), /100/);
      const state = (await (await api(`/api/decks/${id}`)).json()).state;
      const utility = state.slots.find((s: any) => s.name === "Utility").id;
      assert.deepEqual(state.cards[0].secondary_slot_ids, [utility]);
      assert.equal(state.computed.slot_deltas.find((s: any) => s.slot_id === utility).count, 0);
      const patch = (body: unknown) => api(`/api/decks/${id}/cards/id-solring`, { method: "PATCH", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) });
      assert.equal((await patch({ quantity: 2, secondary_slot_ids: [-1] })).status, 400);
      const unchanged = (await (await api(`/api/decks/${id}`)).json()).state;
      assert.equal(unchanged.cards[0].quantity, 1);
      assert.equal(unchanged.deck.revision, state.deck.revision);
      assert.equal((await patch({ secondary_slot_ids: [] })).status, 200);
      const read = await client.callTool({ name: "get_deck", arguments: { deck: id, sections: ["state"] } });
      assert.match(textOf(read), /Utility: 0 primary .*0 secondary/);
    } finally { await client.close(); }
  });

  test("assembly uses ordinary tags through HTTP and MCP; obsolete paper endpoints are absent", async () => {
    const created = await (await post("/api/decks", { name: "Tagged assembly", category: "paper" })).json();
    const id = created.state.deck.id;
    await post(`/api/decks/${id}/cards`, { oracle_id: "id-solring" });
    await api(`/api/decks/${id}`, { method: "PATCH", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ status: "active" }) });
    const before = await (await api(`/api/decks/${id}`)).json();
    assert.equal("in_paper" in before.state.cards[0], false);
    assert.equal((await api(`/api/decks/${id}/paper`)).status, 404);
    assert.equal((await api(`/api/decks/${id}/cards`, { method: "PATCH", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ in_paper: true }) })).status, 404);
    const client = await connect("/mcp/assembly-test");
    try {
      assert.equal((await client.listTools()).tools.some(t => t.name === "paper_status"), false);
      const filed = await client.callTool({ name: "file_cards", arguments: { deck: id, filings: [{ card: "Sol Ring", add_tags: ["Need to get"] }] } });
      assert.ok(!filed.isError, textOf(filed));
      const after = await (await api(`/api/decks/${id}`)).json();
      assert.equal(after.state.deck.category, "paper");
      assert.equal(after.state.deck.revision, before.state.deck.revision);
      assert.deepEqual(after.state.tags.filter((t: any) => after.state.cards[0].tag_ids.includes(t.id)).map((t: any) => t.name), ["Need to get"]);
    } finally {
      await client.close();
    }
  });

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
    assert.ok(!("status" in created.state.deck));
    const patch = (body: unknown) => api(`/api/decks/${id}`, {
      method: "PATCH", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body),
    });
    const updated = await (await patch({ category: "archived" })).json();
    assert.equal(updated.state.deck.category, "archived");
    assert.equal((await (await api(`/api/decks/${id}`)).json()).state.deck.category, "archived");
    assert.equal((await (await api("/api/decks")).json()).find((d: any) => d.id === id).category, "archived");
    await patch({ category: "paper" });
    const invalid = await patch({ name: "Should roll back", category: "unknown" });
    assert.equal(invalid.status, 400);
    assert.match((await invalid.json()).error, /Deck category must be/);
    const unchanged = await (await api(`/api/decks/${id}`)).json();
    assert.equal(unchanged.state.deck.name, "Paper deck");
    assert.equal(unchanged.state.deck.category, "paper");
    for (const category of [null, "unknown", 1]) {
      assert.equal((await post("/api/decks", { name: "Invalid category", category })).status, 400);
    }
    assert.ok(!(await (await api("/api/decks")).json()).some((d: any) => d.name === "Invalid category"));
    const defaulted = await (await post("/api/decks", { name: "Default category" })).json();
    assert.equal(defaulted.state.deck.category, "theorycrafted");
  });

  test("Archived is read-only through HTTP and MCP, and owner restoration reopens pending work", async () => {
    const created = await (await post("/api/decks", { name: "Archived API deck", category: "paper" })).json();
    const id = created.state.deck.id;
    await post(`/api/decks/${id}/cards`, { oracle_id: "id-solring" });
    const client = await connect();
    try {
      await client.callTool({ name: "propose_changes", arguments: { deck: id,
        items: [{ action: "cut", card: "Sol Ring", reason: "Owner requested a slower deck." }] } });
      await client.callTool({ name: "propose_brief_edit", arguments: { deck: id,
        kind: "thesis", text: "Slower plan", reason: "Owner requested slower games." } });
      const pending = await (await api("/api/inbox")).json();
      const item = pending.proposals.find((p: any) => p.deck_id === id).items[0].id;
      const edit = pending.document_edits.find((e: any) => e.deck_id === id).id;
      const patch = (body: unknown) => api(`/api/decks/${id}`, {
        method: "PATCH", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body),
      });
      assert.equal((await patch({ category: "archived" })).status, 200);
      for (const result of [
        await post(`/api/decks/${id}/cards`, { oracle_id: "id-forest" }),
        await api(`/api/decks/${id}/brief`, { method: "PUT", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ thesis: "Changed" }) }),
        await patch({ name: "Changed archive" }),
        await api(`/api/decks/${id}`, { method: "DELETE" }),
        await post(`/api/decks/${id}/items/rule`, { item_ids: [item], outcome: "accept" }),
        await post(`/api/document-edits/${edit}/rule`, { outcome: "accept" }),
      ]) assert.equal(result.status, 403);
      const blocked = await client.callTool({ name: "edit_cards", arguments: { deck: id,
        remove: [{ card: "Sol Ring" }] } });
      assert.equal(blocked.isError, true);
      assert.match(textOf(blocked), /read-only/);
      const hidden = await (await api("/api/inbox")).json();
      assert.ok(!hidden.proposals.some((p: any) => p.deck_id === id));
      assert.ok(!hidden.document_edits.some((e: any) => e.deck_id === id));
      assert.equal((await post(`/api/decks/${id}/fork`, { name: "Archived API fork" })).status, 200);
      assert.equal((await patch({ category: "theorycrafted" })).status, 200);
      const restored = await (await api("/api/inbox")).json();
      assert.ok(restored.proposals.some((p: any) => p.deck_id === id));
      assert.ok(restored.document_edits.some((e: any) => e.deck_id === id));
      assert.equal((await post(`/api/document-edits/${edit}/rule`, { outcome: "reject" })).status, 200);
      assert.equal((await post(`/api/decks/${id}/items/rule`, { item_ids: [item], outcome: "reject" })).status, 200);
    } finally { await client.close(); }
  });

  test("acts as the owner: adds a card, rules on a proposal, undoes it", async () => {
    const added = await (await post(`/api/decks/${deck}/cards`, { oracle_id: "id-counterspell" })).json();
    assert.equal(added.state.computed.card_count, 2);
    assert.equal(added.state.deck.category, "paper");
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
    assert.equal(ruled.state.log[0].decision_reason, "Fine.");

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
    assert.equal(settings.agent.token, undefined);
    assert.equal(settings.agent.mode, "local");
    assert.equal(settings.agent.tokens[0].label, "agent");
    assert.equal(settings.agent.tokens[0].token_hash, undefined);
    assert.match(settings.agent.url, /^http:\/\/127\.0\.0\.1:\d+\/mcp$/);
    assert.equal(settings.agent.stdio_command, `node ${join(PKG_ROOT, "src", "mcp", "stdio.ts")}`);
    assert.deepEqual(settings.budgets, { global: 1500, deck: 1000 });
  });
});

describe("the stdio shim (spec §10.1)", () => {
  const shim = (extraEnv: Record<string, string> = {}) =>
    new StdioClientTransport({
      command: process.execPath,
      args: [join(PKG_ROOT, "src", "mcp", "stdio.ts"), "--name", "desktop-app", "--token-file", tokenFile],
      cwd: tmpdir(),
      env: {
        ...(process.env as Record<string, string>),
        DECKBUILDER_PORT: String((app.server.address() as AddressInfo).port),
        ...extraEnv,
      },
    });
  test("a stdio client launched outside the package reaches the app under its label", async () => {
    const client = new Client({ name: "stdio-client", version: "1.0.0" });
    await client.connect(shim());
    assert.equal((await client.listTools()).tools.length, TOOLS.length);
    const result = await client.callTool({
      name: "remember",
      arguments: { text: "Came in over stdio.", scope: "global" },
    });
    assert.match(textOf(result), /Saved\./);
    assert.equal(listActivity(db, OWNER)[0].actor, "desktop-app");
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

describe("sourcing and proposal cleanup end to end", () => {
  const api = (path: string, init: RequestInit = {}) => fetch(`${base}${path}`, init);
  const post = (path: string, body: unknown) => api(path, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) });
  test("both MCP protocol eras carry structured sourcing records", async () => {
    const target = newDeck(db, "theorycrafted", "Protocol source target");
    const source = newDeck(db, "paper", "Protocol source");
    add(db, target, "id-solring"); add(db, source, "id-solring");
    for (const modern of [false, true]) {
      const client = await connect("/mcp/source-agent", modern);
      try {
        const result = await client.callTool({ name: "compare_deck_sources", arguments: { deck: target, sources: [source] } });
        assert.equal(result.isError, undefined);
        const data = result.structuredContent as any;
        assert.equal(data.shared_cards[0].oracle_id, "id-solring");
        assert.equal(data.shared_cards[0].source_decks[0].quantity, 1);
        assert.equal(data.shared_cards[0].ownership.quantity, null);
        assert.match(textOf(result), /1 shared/);
      } finally { await client.close(); }
    }
  });

  test("owner HTTP approvals apply groups, expose history and refreshed checks, and reject superseded/stale drafts", async () => {
    const target = newDeck(db, "paper", "Protocol cleanup target");
    const client = await connect("/mcp/cleanup-agent");
    try {
      const initial = await client.callTool({ name: "propose_document_changes", arguments: { deck: target, expected_revision: 0,
        changes: [{ kind: "thesis", text: "Old draft", reason: "Request" }, { kind: "constraints", text: "Budget", reason: "Request" }] } });
      const oldId = (initial.structuredContent as any).group_id;
      const replacement = await client.callTool({ name: "revise_proposal", arguments: { type: "document", id: oldId, action: "amend", reason: "New budget", expected_revision: 0,
        changes: [{ kind: "thesis", text: "Current plan", reason: "Budget" }, { kind: "engine_set", engine_name: "Ramp", pieces: [{ card: "Sol Ring" }], reason: "Plan" }] } });
      const currentId = (replacement.structuredContent as any).group_id;
      assert.equal((await post(`/api/document-edits/${oldId}/rule`, { outcome: "accept" })).status, 400);
      add(db, target, "id-forest");
      assert.equal((await post(`/api/document-edits/${currentId}/rule`, { outcome: "accept" })).status, 409);
      const response = await post(`/api/document-edits/${currentId}/rule`, { outcome: "accept", expected_revision: 1, acknowledge_stale: true });
      assert.equal(response.status, 200);
      const result = await response.json();
      assert.equal(result.state.brief.thesis, "Current plan");
      assert.equal(result.state.brief.engines[0].name, "Ramp");
      assert.ok(result.state.checks.some((c: any) => c.key.startsWith("engine_incomplete")));
      assert.equal(result.state.document_edits.length, 0);
      const history = await (await api("/api/proposal-history")).json();
      assert.equal(history.document_edits.find((e: any) => e.id === oldId).superseded_by, currentId);
      assert.equal(history.document_edits.find((e: any) => e.id === currentId).status, "accepted");
    } finally { await client.close(); }
  });
});

for (const modern of [false, true]) test(`personal price sources reach real MCP clients without contaminating non-price tools (${modern ? "modern" : "archived"})`, async () => {
  const { cheapestPrintings, ingestPrices, setPriceSource } = await import("../src/prices.ts");
  const { createDeck } = await import("../src/deck/service.ts");
  const { applyChanges } = await import("../src/deck/membership.ts");
  const { exportDeck } = await import("../src/deck/interop.ts");
  const fresh = testDb();
  fresh.exec("INSERT INTO users(id,handle,display_name) VALUES(2,'euro-user','Euro user')");
  const euro = { ...OWNER, user_id: 2 };
  setPriceSource(fresh, euro, { currency: "EUR", store: "cardmarket" });
  await ingestPrices(fresh, await cheapestPrintings([{ oracle_id: "id-solring", set: "test", collector_number: "1", games: ["paper"], prices: { usd: "1.19", eur: "0.80" } }]), "2026-10-25T21:05:44.018+00:00");
  const target = createDeck(fresh, euro, "EUR HTTP prices", { category: "theorycrafted" });
  const source = createDeck(fresh, euro, "EUR source", { category: "paper" });
  applyChanges(fresh, euro, target, [{ action: "add", oracle_id: "id-solring" }], { kind: "edit" });
  const token = createToken(fresh, euro, "price reader").token;
  const service = createApp(fresh);
  await new Promise<void>(resolve => service.server.listen(0, "127.0.0.1", resolve));
  const url = new URL(`http://127.0.0.1:${(service.server.address() as AddressInfo).port}/mcp/prices-test`);
  const client = new Client({ name: "prices-test", version: "1" }, modern ? { versionNegotiation: { mode: { pin: "2026-07-28" } } } : {});
  try {
    await client.connect(new StreamableHTTPClientTransport(url, { requestInit: { headers: { Authorization: `Bearer ${token}` } } }));
    const call = async (name: string, args: Record<string, unknown> = {}) => {
      const result = await client.callTool({ name, arguments: args });
      assert.ok(!result.isError, textOf(result));
      return result;
    };
    const line = "Prices: Cardmarket (EUR), Scryfall daily estimates as of 2026-10-25.";
    for (const [name, args] of [
      ["start_session", {}], ["get_deck", { deck: target }], ["get_deck", { deck: target, sections: ["list"] }],
      ["get_cards", { cards: ["Sol Ring"] }], ["search_cards", { query: "eur<=1" }],
      ["compare_deck_sources", { deck: target, sources: [source] }], ["list_proposals", { deck: target }],
    ] as const) {
      const result = await call(name, args);
      const text = textOf(result);
      assert.equal(text.split(line).length - 1, 1, name);
      assert.ok(text.endsWith(line), name);
      if (["get_cards", "search_cards", "compare_deck_sources"].includes(name)) assert.match(text, /€0\.80/);
      if (name === "get_cards") {
        const card = (result.structuredContent as any).cards[0];
        assert.equal(card.currency, "EUR"); assert.equal(card.store, "cardmarket"); assert.equal(card.cents, 80);
      }
    }
    const exported = exportDeck(fresh, euro, target);
    assert.equal(textOf(await call("export_deck", { deck: target })), `${exported.card_count} card(s) at revision ${exported.revision}:\n\n${exported.text}`);
    for (const [name, args] of [["list_decisions", { deck: target }], ["search_memories", {}],
      ["remember", { scope: "global", text: "Prefers careful budget estimates" }],
      ["revise_memory", { id: 1, action: "update", text: "Prefers daily budget estimates" }],
    ] as const) assert.doesNotMatch(textOf(await call(name, args)), /^Prices:/m, name);
  } finally { await client.close(); await service.close(); fresh.close(); }
});

test("settings price source and budgets save atomically", async () => {
  const { getPriceSource } = await import("../src/prices.ts");
  const { getBudgets } = await import("../src/memory.ts");
  const source = getPriceSource(db, OWNER), budgets = getBudgets(db, OWNER);
  const put = (body: unknown) => fetch(`${base}/api/settings`, { method: "PUT", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) });
  const bad = await put({ price_source: { currency: "EUR", store: "cardmarket" }, budgets: { global: 2000, deck: 1 } });
  assert.equal(bad.status, 400);
  assert.deepEqual(getPriceSource(db, OWNER), source);
  assert.deepEqual(getBudgets(db, OWNER), budgets, "an earlier valid budget must also roll back");
  try {
    const good = await put({ price_source: { currency: "EUR", store: "cardmarket" }, budgets: { global: 2000, deck: 2000 } });
    assert.equal(good.status, 200);
    const settings = await good.json();
    assert.deepEqual(settings.price_source, { currency: "EUR", store: "cardmarket" });
    assert.deepEqual(settings.budgets, { global: 2000, deck: 2000 });
  } finally { await put({ price_source: source, budgets }); }
});

// Both settings mutations invalidate open views.
test("budget and price settings writes each send a change event", async () => {
  const { getPriceSource } = await import("../src/prices.ts");
  const source = getPriceSource(db, OWNER);
  const put = (body: unknown) => fetch(`${base}/api/settings`, { method: "PUT", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) });
  const events = await fetch(`${base}/api/events`);
  const reader = events.body!.getReader();
  assert.equal(new TextDecoder().decode((await reader.read()).value), 'data: {"kind":"connected"}\n\n');
  try {
    const { getBudgets } = await import("../src/memory.ts");
    assert.equal((await put({ budgets: getBudgets(db, OWNER) })).status, 200);
    assert.equal(new TextDecoder().decode((await reader.read()).value), 'data: {"kind":"settings"}\n\n');
    assert.equal((await put({ price_source: source })).status, 200);
    assert.equal(new TextDecoder().decode((await reader.read()).value), 'data: {"kind":"settings"}\n\n');
  } finally { await reader.cancel(); }
});
