import { test } from "node:test";
import assert from "node:assert/strict";
import { callTool } from "../src/mcp/tools.ts";
import { getDeck } from "../src/deck/service.ts";
import { AGENT, newDeck, testDb } from "./helpers.ts";

test("MCP nested argument typos fail before editing", () => {
  const db = testDb();
  try {
    const deck = newDeck(db, "theorycrafted");
    assert.throws(() => callTool({ db, actor: AGENT }, "edit_cards", {
      deck, add: [{ card: "id-solring", quantitty: 4 }],
    }), /quantitty/);
    assert.equal(getDeck(db, AGENT, deck).computed.card_count, 0);
  } finally { db.close(); }
});

test("MCP write and its audit record roll back together on a late failure", () => {
  const db = testDb();
  try {
    const deck = newDeck(db, "theorycrafted");
    db.exec("CREATE TEMP TRIGGER reject_activity BEFORE INSERT ON activity BEGIN SELECT RAISE(ABORT, 'audit unavailable'); END");
    assert.throws(() => callTool({ db, actor: AGENT }, "edit_cards", {
      deck, add: [{ card: "id-solring" }],
    }), /audit unavailable/);
    const view = getDeck(db, AGENT, deck);
    assert.equal(view.computed.card_count, 0);
    assert.equal(view.deck.revision, 0);
    assert.equal(db.prepare("SELECT id FROM log WHERE deck_id=?").get(deck), undefined);
  } finally { db.close(); }
});

test("agent edit notes cannot store an unresolved card reference", () => {
  const db = testDb();
  try {
    const deck = newDeck(db, "theorycrafted");
    assert.throws(() => callTool({ db, actor: AGENT }, "edit_cards", {
      deck, add: [{ card: "id-solring" }], note: "Supports [[not-a-real-card-reference]].",
    }), /not-a-real-card-reference/);
    assert.equal(getDeck(db, AGENT, deck).computed.card_count, 0);
  } finally { db.close(); }
});

test("connection status includes unused grants, orders mixed timestamps, and respects revocation and tenants", async () => {
  const { agentConnections } = await import("../src/agent-status.ts");
  const { createToken, verifyToken } = await import("../src/tokens.ts");
  const { OWNER } = await import("./actors.ts");
  const db = testDb();
  try {
    db.prepare("INSERT INTO users(id,handle,display_name) VALUES(2,'other','Other')").run();
    db.prepare('INSERT INTO user(id,name,email,emailVerified,createdAt,updatedAt) VALUES(?,?,?,1,?,?)')
      .run('audit-owner', 'Owner', 'audit@example.invalid', Date.now(), Date.now());
    db.prepare("UPDATE users SET auth_id='audit-owner' WHERE id=1").run();
    db.prepare("INSERT INTO oauthClient(id,clientId,name,redirectUris) VALUES('audit-client','audit-client','OAuth reader','[]')").run();
    db.prepare("INSERT INTO oauthConsent(id,clientId,userId,scopes,createdAt,updatedAt) VALUES('audit-consent','audit-client','audit-owner','[\"deck-builder\"]',0,0)").run();
    assert.deepEqual(agentConnections(db, OWNER).map(c => ({ ...c })), [{ kind: "oauth", label: "OAuth reader", last_used_at: null }]);
    const token = createToken(db, OWNER, "Token reader");
    verifyToken(db, token.token, Date.parse("2026-10-30T12:00:00Z"));
    db.prepare("INSERT INTO oauth_usage(user_id,client_id,label,last_used_at) VALUES(1,'audit-client','OAuth reader','2026-10-30 13:00:00')").run();
    assert.deepEqual(agentConnections(db, OWNER).map(c => c.label), ["OAuth reader", "Token reader"]);
    assert.deepEqual(agentConnections(db, { ...OWNER, user_id: 2 }), []);
    assert.throws(() => agentConnections(db, AGENT), /Only the owner|app|UI/i);
    db.prepare("DELETE FROM oauthConsent WHERE id='audit-consent'").run();
    assert.deepEqual(agentConnections(db, OWNER).map(c => c.label), ["Token reader"]);
  } finally { db.close(); }
});

test("commands migrate existing decks, reject stale edits and replay once across restart and expiry", async () => {
  const { mkdtempSync, rmSync } = await import("node:fs");
  const { tmpdir } = await import("node:os");
  const { join } = await import("node:path");
  const { legacyDb, removeCommands } = await import("./legacy-db.ts");
  const { openDb } = await import("../src/db.ts");
  const { ingestCards } = await import("../src/ingest.ts");
  const { FIXTURES } = await import("./fixtures.ts");
  const { createToken } = await import("../src/tokens.ts");
  const { OWNER } = await import("./actors.ts");
  const dir = mkdtempSync(join(tmpdir(), "agent-commands-")), path = join(dir, "commands.db");
  let db = legacyDb(path);
  try {
    ingestCards(db, FIXTURES);
    const deck = newDeck(db, "theorycrafted"), before = getDeck(db, OWNER, deck);
    removeCommands(db);
    db.exec("UPDATE meta SET value='16' WHERE key='schema_version'");
    db.close(); db = openDb(path);
    assert.deepEqual(getDeck(db, OWNER, deck), before);
    const credential = createToken(db, OWNER, "Retry client");
    const actor = { ...AGENT, token_id: credential.credential.id };
    const args = { deck, add: [{ card: "id-forest" }], expected_revision: 0, expected_edit_version: 0, idempotency_key: "first-write" };
    const first = callTool({ db, actor }, "edit_cards", args);
    assert.equal(first.command?.decks[0].delta?.card_count, 1);
    assert.equal(first.command?.decks[0].computed?.card_count, 1);
    assert.ok(first.command?.decks[0].warnings?.length);
    assert.equal(first.command?.activity_ids.length, 1);
    assert.equal(typeof first.structuredContent?.log_id, "number");
    callTool({ db, actor }, "define_slots", { deck, slots: [{ name: "Utility" }] });
    assert.equal(getDeck(db, actor, deck).deck.revision, 1);
    assert.throws(() => callTool({ db, actor }, "edit_cards", { ...args, expected_revision: 1, expected_edit_version: first.command!.decks[0].edit_version, idempotency_key: "stale" }), { status: 409 });
    assert.throws(() => callTool({ db, actor }, "edit_cards", { ...args, add: [{ card: "id-forest", quantity: 2 }] }), { status: 409 });
    db.close(); db = openDb(path);
    const retry = callTool({ db, actor }, "edit_cards", args);
    assert.ok(retry.text.endsWith(first.text));
    assert.match(retry.text, /no additional write/);
    assert.equal(retry.command?.replayed, true);
    assert.equal(retry.changed, false);
    assert.equal(getDeck(db, actor, deck).computed.card_count, 1);
    assert.equal(db.prepare("SELECT count(*) n FROM activity WHERE action='edit_cards'").get()!.n, 1);
    // Equal labels are not identity. Another credential has its own key space.
    const other = { ...actor, token_id: createToken(db, OWNER, "Retry client").credential.id };
    const memory = { scope: "global", text: "A lasting preference", idempotency_key: "first-write" };
    assert.equal(callTool({ db, actor: other }, "remember", memory).command?.replayed, false);
    assert.throws(() => callTool({ db, actor: { ...actor, user_id: 999 } }, "edit_cards", args), { status: 404 });
    db.prepare("UPDATE command_receipts SET expires_at=0 WHERE credential=?").run(`token:${actor.token_id}`);
    assert.throws(() => callTool({ db, actor }, "edit_cards", args), /expired/);
    assert.equal(db.prepare("SELECT result_json FROM command_receipts WHERE credential=?").get(`token:${actor.token_id}`)!.result_json, null);
    const deleted = callTool({ db, actor }, "delete_deck", { deck, idempotency_key: "delete" });
    assert.equal(deleted.command?.decks[0].deleted, true);
    assert.equal(callTool({ db, actor }, "delete_deck", { deck, idempotency_key: "delete" }).command?.replayed, true);
    assert.deepEqual(db.prepare("PRAGMA foreign_key_check").all(), []);
  } finally { db.close(); rmSync(dir, { recursive: true, force: true }); }
});

test("focus compares tab ownership and sequence, retains stale reads and rejects stale implicit writes", async t => {
  const { setFocus, getFocus } = await import("../src/focus.ts");
  const { OWNER } = await import("./actors.ts");
  const db = testDb();
  let now = Date.now();
  t.mock.method(Date, "now", () => now);
  try {
    const a = newDeck(db, "theorycrafted"), b = newDeck(db, "theorycrafted");
    const tab = (id: string, sequence: number, claim = false) => ({ tab_id: id, sequence, claim });
    assert.equal(setFocus(db, OWNER, a, [], tab("a", 1)), true);
    assert.equal(setFocus(db, OWNER, b, [], tab("b", 1)), false);
    assert.equal(setFocus(db, OWNER, b, [], tab("b", 2, true)), true);
    assert.equal(setFocus(db, OWNER, null, [], tab("a", 2)), false);
    assert.equal(setFocus(db, OWNER, a, [], tab("a", 3)), false);
    assert.equal(getFocus(db, OWNER)?.deck_id, b);
    now += 4 * 60_000;
    assert.equal(getFocus(db, OWNER)?.stale, true);
    assert.match(callTool({ db, actor: AGENT }, "get_deck", { deck: "focus", sections: ["brief"] }).text, /last.viewed/i);
    assert.throws(() => callTool({ db, actor: AGENT }, "rename_deck", { deck: "focus", name: "Old context" }), /explicit ID/);
    assert.equal(setFocus(db, OWNER, b, [], tab("b", 3)), true);
    assert.equal(getFocus(db, OWNER)?.stale, false);
    assert.equal(setFocus(db, OWNER, null, [], tab("b", 5)), true);
    assert.equal(setFocus(db, OWNER, b, [], tab("b", 4, true)), false);
    assert.equal(getFocus(db, OWNER), null);
    assert.equal(setFocus(db, OWNER, a, [], tab("a", 4)), true);
    now += 31 * 60_000;
    assert.equal(getFocus(db, OWNER), null);
    assert.throws(() => setFocus(db, AGENT, a), { status: 403 });
  } finally { db.close(); }
});
