import { test } from "node:test";
import assert from "node:assert/strict";
import { PermissionError } from "../src/actor.ts";
import { OWNER, agent } from "./actors.ts";
import { getBrief, setEngine, updateBrief } from "../src/deck/brief.ts";
import { amendProposal, createProposal, getProposal, listProposals, pendingItemIds, acceptItems, withdrawProposal } from "../src/deck/proposals.ts";
import { getDeck, listDecks, setDeckCategory } from "../src/deck/service.ts";
import { listDocumentEdits, proposeDocumentEdit, proposeDocumentGroup, ruleDocumentEdit, withdrawDocumentEdit } from "../src/documents.ts";
import { deckState } from "../src/state.ts";
import { callTool } from "../src/mcp/tools.ts";
import { AGENT, add, cut, newDeck, testDb } from "./helpers.ts";

const other = agent("another-agent");

test("a replacement plan supersedes the entire old group, preserves history, and applies all changes only on owner approval", () => {
  const db = testDb();
  try {
    const id = newDeck(db, "paper");
    add(db, id, "id-solring");
    setEngine(db, OWNER, id, "Old mana", "Old plan", [{ oracle_id: "id-solring" }]);
    updateBrief(db, OWNER, id, { thesis: "Old thesis" });
    cut(db, id, "id-solring");
    const old = proposeDocumentGroup(db, AGENT, id, [
      { kind: "engine_remove", payload: { engine_name: "Old mana" }, reason: "Owner removed the package." },
      { kind: "thesis", payload: { content: "First revised finish" }, reason: "Reflect the cuts." },
    ], { expected_revision: 2 });
    const result = proposeDocumentGroup(db, AGENT, id, [
      { kind: "engine_remove", payload: { engine_name: "Old mana" }, reason: "The removed package remains obsolete." },
      { kind: "engine_set", payload: { engine_name: "Budget finish", pieces: [{ oracle_id: "id-llanowar" }] }, reason: "Owner reduced the budget." },
      { kind: "thesis", payload: { content: "Budget finish" }, reason: "Matches the replacement engine." },
    ], { supersedes: [old.edit_ids[1]], expected_revision: 2 });
    const pending = listDocumentEdits(db, OWNER, { deckId: id, status: "pending" });
    assert.equal(pending.length, 3);
    assert.ok(pending.every(e => e.group_id === result.group_id && e.is_current));
    assert.equal(pending[0].group_edits.length, 3);
    assert.equal(pending[0].base_revision, 2);
    const history = listDocumentEdits(db, OWNER, { deckId: id });
    for (const oldId of old.edit_ids) {
      const row = history.find(e => e.id === oldId)!;
      assert.equal(row.disposition, "superseded");
      assert.equal(row.superseded_by, result.group_id);
      assert.equal(row.is_current, false);
      assert.throws(() => ruleDocumentEdit(db, OWNER, oldId, "accept"), /superseded/);
    }
    assert.deepEqual(history.find(e => e.id === result.group_id)!.supersedes, old.edit_ids);
    assert.equal(getBrief(db, OWNER, id).thesis, "Old thesis");
    assert.throws(() => ruleDocumentEdit(db, AGENT, result.group_id!, "accept"), PermissionError);
    ruleDocumentEdit(db, OWNER, result.edit_ids[2], "accept", "Use the budget plan.", { expected_revision: 2 });
    const state = deckState(db, OWNER, id);
    assert.equal(state.brief.thesis, "Budget finish");
    assert.deepEqual(state.brief.engines.map(e => e.name), ["Budget finish"]);
    assert.ok(state.checks.some(c => c.key.startsWith("engine_incomplete")));
    assert.equal(state.document_edits.length, 0);
    assert.throws(() => ruleDocumentEdit(db, OWNER, old.edit_ids[0], "accept"), /superseded/);
    assert.equal(getBrief(db, OWNER, id).thesis, "Budget finish");
  } finally { db.close(); }
});

test("stale membership revisions block approval until reviewed; mismatched expected revisions always fail", () => {
  const db = testDb();
  try {
    const id = newDeck(db);
    const proposal = proposeDocumentEdit(db, AGENT, id, "thesis", { content: "Proposed plan" }, "Owner asked.");
    add(db, id, "id-solring");
    assert.equal(listDocumentEdits(db, OWNER)[0].stale, true);
    assert.throws(() => ruleDocumentEdit(db, OWNER, proposal.edit_id!, "accept"), /stale/);
    assert.throws(() => ruleDocumentEdit(db, OWNER, proposal.edit_id!, "accept", "", { acknowledge_stale: true, expected_revision: 0 }), /revision changed/);
    assert.equal(getBrief(db, OWNER, id).thesis, "");
    ruleDocumentEdit(db, OWNER, proposal.edit_id!, "accept", "Reviewed", { acknowledge_stale: true, expected_revision: 1 });
    assert.equal(getBrief(db, OWNER, id).thesis, "Proposed plan");
  } finally { db.close(); }
});

test("engine edits and removals detect changed engine contents, even with unchanged deck revision", () => {
  const db = testDb();
  try {
    const id = newDeck(db);
    setEngine(db, OWNER, id, "Mana", "Before", [{ oracle_id: "id-solring" }]);
    const edit = proposeDocumentEdit(db, AGENT, id, "engine_remove", { engine_name: "Mana" }, "Obsolete.");
    setEngine(db, OWNER, id, "Mana", "Owner replaced the engine", [{ oracle_id: "id-llanowar" }]);
    assert.equal(getDeck(db, OWNER, id).deck.revision, 0);
    assert.equal(listDocumentEdits(db, OWNER)[0].stale, true);
    assert.throws(() => ruleDocumentEdit(db, OWNER, edit.edit_id!, "accept"), /stale/);
    assert.equal(getBrief(db, OWNER, id).engines[0].description, "Owner replaced the engine");
  } finally { db.close(); }
});

test("another agent's newer accepted text makes the older draft stale, rather than silently restoring it", () => {
  const db = testDb();
  try {
    const id = newDeck(db);
    const old = proposeDocumentEdit(db, AGENT, id, "thesis", { content: "Old direction" }, "Earlier request");
    const newer = proposeDocumentEdit(db, other, id, "thesis", { content: "New direction" }, "Latest request");
    ruleDocumentEdit(db, OWNER, newer.edit_id!, "accept");
    assert.throws(() => ruleDocumentEdit(db, OWNER, old.edit_id!, "accept"), /stale/);
    assert.equal(getBrief(db, OWNER, id).thesis, "New direction");
  } finally { db.close(); }
});

test("failed replacement rolls back supersession and duplicate pending fields require explicit cleanup", () => {
  const db = testDb();
  try {
    const id = newDeck(db);
    const first = proposeDocumentEdit(db, AGENT, id, "thesis", { content: "First" }, "Request");
    assert.throws(() => proposeDocumentEdit(db, AGENT, id, "thesis", { content: "Duplicate" }, "Request"), /supersede/);
    assert.throws(() => proposeDocumentGroup(db, AGENT, id, [{ kind: "engine_remove", payload: { engine_name: "Missing engine" }, reason: "Bad edit" }], { supersedes: [first.edit_id!] }), /no engine/);
    assert.equal(listDocumentEdits(db, OWNER, { status: "pending" })[0].id, first.edit_id);
    assert.equal(listDocumentEdits(db, OWNER)[0].disposition, "current");
  } finally { db.close(); }
});

test("group rejection and withdrawal affect the whole group, preserving proposed text and the accepted brief", () => {
  const db = testDb();
  try {
    const id = newDeck(db);
    const changes = [
      { kind: "thesis" as const, payload: { content: "Draft" }, reason: "Request" },
      { kind: "constraints" as const, payload: { content: "Budget" }, reason: "Request" },
    ];
    const first = proposeDocumentGroup(db, AGENT, id, changes);
    assert.throws(() => withdrawDocumentEdit(db, other, first.group_id!, "No"), PermissionError);
    withdrawDocumentEdit(db, AGENT, first.edit_ids[1], "Budget changed");
    assert.ok(listDocumentEdits(db, OWNER).every(e => e.disposition === "withdrawn"));
    assert.throws(() => ruleDocumentEdit(db, OWNER, first.group_id!, "accept"), /withdrawn/);
    const next = proposeDocumentGroup(db, AGENT, id, changes);
    ruleDocumentEdit(db, OWNER, next.edit_ids[1], "reject");
    assert.equal(listDocumentEdits(db, OWNER, { status: "pending" }).length, 0);
    assert.equal(getBrief(db, OWNER, id).thesis, "");
  } finally { db.close(); }
});

test("group application is atomic if a later member fails validation at approval", () => {
  const db = testDb();
  try {
    const id = newDeck(db);
    setEngine(db, OWNER, id, "Old engine", "", []);
    const proposal = proposeDocumentGroup(db, AGENT, id, [
      { kind: "thesis", payload: { content: "Should roll back" }, reason: "Request" },
      { kind: "engine_remove", payload: { engine_name: "Old engine" }, reason: "Request" },
    ]);
    db.prepare("DELETE FROM engines WHERE deck_id = ?").run(id);
    assert.throws(() => ruleDocumentEdit(db, OWNER, proposal.group_id!, "accept", "Reviewed", { acknowledge_stale: true, expected_revision: 0 }), /no engine/);
    assert.equal(getBrief(db, OWNER, id).thesis, "");
    assert.equal(listDocumentEdits(db, OWNER, { status: "pending" }).length, 2);
  } finally { db.close(); }
});

test("card amendments preserve the draft and remove superseded items from app-computed projections", () => {
  const db = testDb();
  try {
    const id = newDeck(db);
    const first = createProposal(db, AGENT, id, { items: [{ action: "add", oracle_id: "id-solring", reason: "Ramp" }] });
    assert.throws(() => amendProposal(db, other, first, { items: [{ action: "add", oracle_id: "id-solring", reason: "No" }] }), PermissionError);
    const next = amendProposal(db, AGENT, first, { expected_revision: 0, items: [{ action: "add", oracle_id: "id-solring", reason: "New reasoning" }, { action: "add", oracle_id: "id-llanowar", reason: "More ramp" }] });
    assert.equal(getProposal(db, OWNER, first)!.superseded_by, next);
    assert.deepEqual(getProposal(db, OWNER, next)!.supersedes, [first]);
    assert.equal(getDeck(db, OWNER, id).computed.pending.adds, 2);
    assert.equal(listDecks(db, OWNER)[0].pending_items, 2);
    assert.equal(listProposals(db, OWNER, { status: "open" }).length, 1);
    assert.throws(() => acceptItems(db, OWNER, pendingItemIds(db, OWNER, first)), /superseded/);
    acceptItems(db, OWNER, pendingItemIds(db, OWNER, next));
    assert.equal(getDeck(db, OWNER, id).computed.card_count, 2);
    assert.equal(getProposal(db, OWNER, first)!.items[0].reason, "Ramp");
    assert.throws(() => withdrawProposal(db, AGENT, next, "Changed mind"), /with pending items/);
  } finally { db.close(); }
});

test("card withdrawal and failed amendments retain history and don't invent owner rulings", () => {
  const db = testDb();
  try {
    const id = newDeck(db);
    const first = createProposal(db, AGENT, id, { items: [{ action: "add", oracle_id: "id-solring", reason: "Ramp" }] });
    assert.throws(() => amendProposal(db, AGENT, first, { items: [{ action: "cut", oracle_id: "id-solring", reason: "Not present" }] }), /not in the deck/);
    assert.equal(getProposal(db, OWNER, first)!.is_current, true);
    withdrawProposal(db, AGENT, first, "No longer needed", 0);
    assert.equal(getProposal(db, OWNER, first)!.disposition, "withdrawn");
    assert.equal(getDeck(db, OWNER, id).computed.pending.adds, 0);
    assert.equal(deckState(db, OWNER, id).log.length, 0);
  } finally { db.close(); }
});

test("Legacy and owner-only lifecycle/approval permissions hold for every cleanup path", () => {
  const db = testDb();
  try {
    const id = newDeck(db);
    const doc = proposeDocumentEdit(db, AGENT, id, "thesis", { content: "Draft" }, "Request").edit_id!;
    const cards = createProposal(db, AGENT, id, { items: [{ action: "add", oracle_id: "id-solring", reason: "Ramp" }] });
    setDeckCategory(db, OWNER, id, "legacy");
    for (const actor of [OWNER, AGENT]) {
      assert.throws(() => withdrawDocumentEdit(db, actor, doc, "Obsolete"), PermissionError);
      assert.throws(() => proposeDocumentGroup(db, actor, id, [{ kind: "thesis", payload: { content: "New" }, reason: "Request" }], { supersedes: [doc] }), PermissionError);
      assert.throws(() => withdrawProposal(db, actor, cards, "Obsolete"), PermissionError);
      assert.throws(() => amendProposal(db, actor, cards, { items: [{ action: "add", oracle_id: "id-solring", reason: "New" }] }), PermissionError);
      assert.throws(() => ruleDocumentEdit(db, actor, doc, "accept"), PermissionError);
    }
    assert.throws(() => setDeckCategory(db, AGENT, id, "paper"), PermissionError);
    assert.equal(listDocumentEdits(db, OWNER)[0].read_only, true);
  } finally { db.close(); }
});

test("MCP document amendment exposes one current group with history and no agent approval tool", () => {
  const db = testDb();
  try {
    const id = newDeck(db);
    const ctx = { db, actor: AGENT };
    const first = callTool(ctx, "propose_document_changes", { deck: id, expected_revision: 0,
      changes: [{ kind: "thesis", text: "First plan", reason: "Request" }, { kind: "constraints", text: "Budget", reason: "Request" }] });
    const oldId = first.structuredContent!.group_id as number;
    const next = callTool(ctx, "revise_proposal", { type: "document", id: oldId, action: "amend", reason: "Revised budget", expected_revision: 0,
      changes: [{ kind: "thesis", text: "New plan", reason: "Budget changed" }] });
    assert.ok(next.structuredContent!.group_id);
    const current = callTool(ctx, "list_proposals", { deck: id });
    assert.equal((current.structuredContent!.document_edits as unknown[]).length, 1);
    assert.equal((callTool(ctx, "list_proposals", { deck: id, include_history: true }).structuredContent!.document_edits as unknown[]).length, 3);
    assert.equal(getBrief(db, OWNER, id).thesis, "");
    assert.throws(() => callTool(ctx, "ruleDocumentEdit", {}), /Unknown tool/);
  } finally { db.close(); }
});

test("amending partially ruled cards preserves accepted items and replaces only the pending plan", () => {
  const db = testDb();
  try {
    const id = newDeck(db);
    const first = createProposal(db, AGENT, id, { items: [
      { action: "add", oracle_id: "id-solring", reason: "Ramp" },
      { action: "add", oracle_id: "id-llanowar", reason: "More ramp" },
    ] });
    acceptItems(db, OWNER, [pendingItemIds(db, OWNER, first)[0]]);
    const next = amendProposal(db, AGENT, first, { expected_revision: 1,
      items: [{ action: "add", oracle_id: "id-seedborn", reason: "Revised remaining work" }] });
    assert.equal(getProposal(db, OWNER, first)!.items[0].status, "accepted");
    assert.equal(getDeck(db, OWNER, id).computed.card_count, 1);
    assert.equal(getDeck(db, OWNER, id).computed.pending.adds, 1);
    acceptItems(db, OWNER, pendingItemIds(db, OWNER, next));
    assert.deepEqual(getDeck(db, OWNER, id).cards.map(c => c.name).sort(), ["Seedborn Muse", "Sol Ring"]);
  } finally { db.close(); }
});

test("reviewed document snapshots guard a second text change at the same membership revision", () => {
  const db = testDb();
  try {
    const id = newDeck(db);
    const editId = proposeDocumentEdit(db, AGENT, id, "thesis", { content: "Proposed" }, "Request").edit_id!;
    updateBrief(db, OWNER, id, { thesis: "First owner change" });
    const reviewed = listDocumentEdits(db, OWNER)[0].current;
    updateBrief(db, OWNER, id, { thesis: "Second owner change" });
    assert.throws(() => ruleDocumentEdit(db, OWNER, editId, "accept", "Reviewed", {
      acknowledge_stale: true, expected_revision: 0, expected_documents: { [editId]: reviewed },
    }), /changed since it was reviewed/);
    assert.equal(getBrief(db, OWNER, id).thesis, "Second owner change");
  } finally { db.close(); }
});

test("brief and power-target changes in one group both await owner approval", () => {
  const db = testDb();
  try {
    const id = newDeck(db, "theorycrafted");
    const ctx = { db, actor: AGENT };
    const result = callTool(ctx, "propose_document_changes", { deck: id, expected_revision: 0, changes: [
      { kind: "thesis", text: "Social games", reason: "Owner request" },
      { kind: "power_target", target: { profile: { intent: "social" } }, reason: "Owner wants social games." },
    ] });
    assert.equal(deckState(db, OWNER, id).power.target, null);
    assert.equal(getBrief(db, OWNER, id).thesis, "");
    ruleDocumentEdit(db, OWNER, result.structuredContent!.group_id as number, "accept", "Agreed", { expected_revision: 0 });
    assert.equal(getBrief(db, OWNER, id).thesis, "Social games");
    assert.equal(deckState(db, OWNER, id).power.target!.profile.intent, "social");
  } finally { db.close(); }
});
