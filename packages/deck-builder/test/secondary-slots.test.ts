import { test } from "node:test";
import assert from "node:assert/strict";
import { OWNER } from "../src/actor.ts";
import { createSlot, createTag, deleteSlot, fileCards, getDeck, organizeCard, shortlistCard } from "../src/deck/service.ts";
import { applyChanges, undo } from "../src/deck/membership.ts";
import { forkDeck } from "../src/deck/fork.ts";
import { acceptItems, createProposal, getProposal, pendingItemIds } from "../src/deck/proposals.ts";
import { search } from "../src/search/index.ts";
import { callTool } from "../src/mcp/tools.ts";
import { AGENT, add, cut, newDeck, testDb } from "./helpers.ts";

test("targets count primary copies only; secondary coverage excludes maybe and companion", () => {
  const db = testDb();
  try {
    const deck = newDeck(db);
    const ramp = createSlot(db, deck, "Ramp", 2, 2);
    const utility = createSlot(db, deck, "Utility", 1, 1);
    add(db, deck, "id-forest", { quantity: 3, slotId: ramp });
    add(db, deck, "id-solring", { slotId: utility });
    organizeCard(db, deck, "id-forest", { secondarySlotIds: [utility, utility] });
    organizeCard(db, deck, "id-solring", { secondarySlotIds: [ramp] });
    shortlistCard(db, deck, "id-counterspell");
    organizeCard(db, deck, "id-counterspell", { slotId: ramp, secondarySlotIds: [utility] });
    add(db, deck, "id-teferi", { role: "companion", slotId: ramp });
    organizeCard(db, deck, "id-teferi", { secondarySlotIds: [utility] });
    const view = getDeck(db, deck);
    const [r, u] = view.computed.slot_deltas;
    assert.deepEqual([r.count, r.primary_count, r.secondary_count, r.coverage_count, r.status, r.delta], [3, 3, 1, 4, "over", 1]);
    assert.deepEqual([u.count, u.secondary_count, u.coverage_count, u.status], [1, 3, 4, "ok"]);
    assert.equal(view.computed.card_count, 4);
    assert.deepEqual(view.cards.find(c => c.oracle_id === "id-forest")!.secondary_slot_ids, [utility]);
    createProposal(db, AGENT, deck, { items: [{ action: "cut", oracle_id: "id-teferi", reason: "Companion is outside the 100." }] });
    assert.deepEqual(getDeck(db, deck).computed.pending.by_slot, {});
  } finally { db.close(); }
});

test("filing validates deck scope and duplicates atomically, and tags remain independent", () => {
  const db = testDb();
  try {
    const deck = newDeck(db);
    const other = newDeck(db);
    const ramp = createSlot(db, deck, "Ramp");
    const utility = createSlot(db, deck, "Utility");
    const foreign = createSlot(db, other, "Foreign");
    const tag = createTag(db, deck, "Have");
    add(db, deck, "id-solring", { slotId: ramp });
    organizeCard(db, deck, "id-solring", { secondarySlotIds: [utility], tagIds: [tag] });
    const before = getDeck(db, deck);
    for (const ids of [[ramp], [foreign], [-1], [1.5], null]) {
      assert.throws(() => organizeCard(db, deck, "id-solring", { secondarySlotIds: ids as number[] }));
      assert.deepEqual(getDeck(db, deck), before);
    }
    assert.throws(() => fileCards(db, deck, [
      { oracle_id: "id-solring", secondary_slots: [], add_tags: ["Ordered"] },
      { oracle_id: "id-solring", secondary_slots: ["Unknown"] },
    ]), /no slot named/);
    assert.deepEqual(getDeck(db, deck), before);
    assert.throws(() => db.prepare("INSERT INTO deck_card_secondary_slots VALUES (?, ?, ?)").run(deck, "id-solring", foreign), /FOREIGN KEY/);
    assert.throws(() => db.prepare("INSERT INTO deck_card_secondary_slots VALUES (?, ?, ?)").run(deck, "id-solring", ramp), /primary slot cannot also be secondary/);
    assert.throws(() => db.prepare("UPDATE deck_card_secondary_slots SET slot_id = ? WHERE deck_id = ?").run(ramp, deck), /primary slot cannot also be secondary/);
    organizeCard(db, deck, "id-solring", { slotId: utility });
    const promoted = getDeck(db, deck).cards[0];
    assert.equal(promoted.slot_id, utility);
    assert.deepEqual(promoted.secondary_slot_ids, []);
    assert.deepEqual(promoted.tag_ids, [tag]);
    assert.equal(getDeck(db, deck).deck.revision, before.deck.revision);
  } finally { db.close(); }
});

test("parking, promotion, cut undo, slot deletion and forks preserve remaining purposes", () => {
  const db = testDb();
  try {
    const deck = newDeck(db);
    const ramp = createSlot(db, deck, "Ramp");
    const utility = createSlot(db, deck, "Utility");
    const recovery = createSlot(db, deck, "Recovery");
    add(db, deck, "id-solring", { slotId: ramp });
    organizeCard(db, deck, "id-solring", { secondarySlotIds: [utility, recovery] });
    const parked = cut(db, deck, "id-solring", { park: true })!;
    assert.equal(getDeck(db, deck).computed.slot_deltas[1].secondary_count, 0);
    undo(db, OWNER, parked);
    assert.deepEqual(getDeck(db, deck).cards[0].secondary_slot_ids, [utility, recovery]);
    const fork = forkDeck(db, AGENT, deck, "Purpose fork");
    const copied = getDeck(db, fork);
    assert.deepEqual(copied.cards[0].secondary_slot_ids.map(id => copied.slots.find(s => s.id === id)!.name), ["Utility", "Recovery"]);
    assert.ok(copied.cards[0].secondary_slot_ids.every(id => id !== utility && id !== recovery));
    const removed = cut(db, deck, "id-solring")!;
    deleteSlot(db, deck, utility);
    undo(db, OWNER, removed);
    assert.deepEqual(getDeck(db, deck).cards[0].secondary_slot_ids, [recovery]);
    deleteSlot(db, deck, ramp);
    const card = getDeck(db, deck).cards[0];
    assert.equal(card.slot_id, null);
    assert.deepEqual(card.secondary_slot_ids, [recovery]);
    assert.deepEqual(db.prepare("PRAGMA foreign_key_check").all(), []);
  } finally { db.close(); }
});

test("command-zone changes preserve purposes, and slot search matches secondary assignments", () => {
  const db = testDb();
  try {
    const deck = newDeck(db);
    const draw = createSlot(db, deck, "Card advantage");
    const utility = createSlot(db, deck, "Utility");
    add(db, deck, "id-teferi", { slotId: draw });
    organizeCard(db, deck, "id-teferi", { secondarySlotIds: [utility] });
    applyChanges(db, OWNER, deck, [{ action: "role", oracle_id: "id-teferi", role: "commander" }], { kind: "edit" });
    const view = getDeck(db, deck);
    assert.equal(view.cards[0].slot_id, draw);
    assert.deepEqual(view.cards[0].secondary_slot_ids, [utility]);
    assert.equal(view.computed.slot_deltas[0].count, 1);
    assert.equal(view.computed.slot_deltas[1].secondary_count, 1);
    assert.deepEqual(search(db, "slot:utility", { deckId: deck }).map(c => c.oracle_id), ["id-teferi"]);
  } finally { db.close(); }
});

test("proposals preserve secondary purposes without changing primary target projections", () => {
  const db = testDb();
  try {
    const deck = newDeck(db);
    const ramp = createSlot(db, deck, "Ramp");
    const utility = createSlot(db, deck, "Utility");
    const recovery = createSlot(db, deck, "Recovery");
    add(db, deck, "id-llanowar", { slotId: ramp });
    organizeCard(db, deck, "id-llanowar", { secondarySlotIds: [utility] });
    const proposal = createProposal(db, AGENT, deck, { items: [
      { action: "cut", oracle_id: "id-llanowar", to_maybe: true, reason: "Test cut." },
      { action: "add", oracle_id: "id-solring", slot_id: ramp, secondary_slot_ids: [utility, recovery], reason: "Test add." },
    ] });
    assert.deepEqual(getDeck(db, deck).computed.pending.by_slot, { [ramp]: 0 });
    assert.deepEqual(getProposal(db, proposal)!.items[1].secondary_slot_names, ["Utility", "Recovery"]);
    deleteSlot(db, deck, recovery);
    acceptItems(db, OWNER, pendingItemIds(db, proposal));
    const card = getDeck(db, deck).cards.find(c => c.oracle_id === "id-solring")!;
    assert.deepEqual(card.secondary_slot_ids, [utility]);
    const promote = createProposal(db, AGENT, deck, { items: [
      { action: "add", oracle_id: "id-llanowar", reason: "Return the parked card." },
    ] });
    assert.deepEqual(getDeck(db, deck).computed.pending.by_slot, { [ramp]: 1 });
    assert.equal(getProposal(db, promote)!.items[0].slot_id, ramp);
    acceptItems(db, OWNER, pendingItemIds(db, promote));
    assert.deepEqual(getDeck(db, deck).cards.find(c => c.oracle_id === "id-llanowar")!.secondary_slot_ids, [utility]);
  } finally { db.close(); }
});

test("MCP files named secondary purposes and renders roles separately from management tags", () => {
  const db = testDb();
  try {
    const deck = newDeck(db, "theorycrafted");
    const ramp = createSlot(db, deck, "Ramp", 1, 1);
    createSlot(db, deck, "Utility", 1, 1);
    const name = (db.prepare("SELECT name FROM cards WHERE oracle_id = 'id-solring'").get() as { name: string }).name;
    callTool({ db, actor: AGENT }, "edit_cards", { deck, add: [{ card: name, slot: "Ramp", secondary_slots: ["Utility"] }] });
    callTool({ db, actor: AGENT }, "file_cards", { deck, filings: [{ card: name, add_tags: ["Have"] }] });
    const output = callTool({ db, actor: AGENT }, "get_deck", { deck, sections: ["list", "state"] }).text;
    assert.match(output, /secondary slots: Utility/);
    assert.match(output, /tags: Have/);
    assert.match(output, /Utility: 0 primary \(target 1\) — UNDER by 1; 1 secondary; 1 total coverage/);
    const revision = getDeck(db, deck).deck.revision;
    callTool({ db, actor: AGENT }, "file_cards", { deck, filings: [{ card: name, secondary_slots: [] }] });
    assert.equal(getDeck(db, deck).deck.revision, revision);
    assert.equal(getDeck(db, deck).cards[0].slot_id, ramp);
    assert.deepEqual(getDeck(db, deck).cards[0].secondary_slot_ids, []);
  } finally { db.close(); }
});

test("explicit unused tag cleanup preserves status vocabulary and rolls back on an assigned tag", () => {
  const db = testDb();
  try {
    const deck = newDeck(db);
    const role = createTag(db, deck, "Old purpose");
    createTag(db, deck, "Ordered");
    add(db, deck, "id-solring");
    organizeCard(db, deck, "id-solring", { tagIds: [role] });
    const before = getDeck(db, deck);
    assert.throws(() => fileCards(db, deck, [{ oracle_id: "id-solring", add_tags: ["Have"] }], ["Old purpose"]), /still assigned/);
    assert.deepEqual(getDeck(db, deck), before);
    fileCards(db, deck, [{ oracle_id: "id-solring", remove_tags: ["Old purpose"], add_tags: ["Have"] }], ["Old purpose"]);
    assert.deepEqual(getDeck(db, deck).tags.map(t => t.name), ["Have", "Ordered"]);
    assert.equal(getDeck(db, deck).deck.revision, before.deck.revision);
  } finally { db.close(); }
});
