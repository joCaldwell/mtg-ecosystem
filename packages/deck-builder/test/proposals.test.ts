import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { openDb } from "../src/db.ts";
import { ingestCards } from "../src/ingest.ts";
import { FIXTURES } from "./fixtures.ts";
import { search } from "../src/search/index.ts";
import { addCard, createDeck, createSlot, createTag, getDeck, updateCard } from "../src/deck/service.ts";
import {
  acceptItem,
  addCardNote,
  createProposal,
  getCardHistory,
  getLog,
  listCardNotes,
  listHardFilters,
  listProposals,
  rejectItem,
  removeHardFilter,
  undoDecision,
} from "../src/deck/proposals.ts";

const db = openDb(":memory:");
ingestCards(db, FIXTURES);

function deckWith(name: string, oracleIds: string[] = []): number {
  const id = createDeck(db, name);
  for (const oid of oracleIds) addCard(db, id, oid);
  return id;
}

function openItems(deckId: number) {
  return listProposals(db, deckId, "open").flatMap((p) => p.items);
}

describe("proposal creation", () => {
  test("validates items: rationale required, cut must be in deck, add must not be", () => {
    const id = deckWith("Create", ["id-solring"]);
    assert.throws(
      () => createProposal(db, id, [{ action: "add", oracle_id: "id-seedborn", rationale: " " }]),
      /rationale/,
    );
    assert.throws(
      () => createProposal(db, id, [{ action: "cut", oracle_id: "id-seedborn", rationale: "x" }]),
      /not in the deck/,
    );
    assert.throws(
      () => createProposal(db, id, [{ action: "add", oracle_id: "id-solring", rationale: "x" }]),
      /already in the deck/,
    );
    assert.throws(
      () => createProposal(db, id, [{ action: "add", oracle_id: "nope", rationale: "x" }]),
      /Unknown oracle_id/,
    );
  });
  // Nothing is exempt from the cap now that imports apply directly (§9)
  // rather than arriving here as a 99-item proposal.
  test("cap of 5 items, no exemptions", () => {
    const id = deckWith("Cap");
    const many = ["id-llanowar", "id-seedborn", "id-counterspell", "id-solring", "id-goyf", "id-ball"];
    const items = many.map((o) => ({ action: "add" as const, oracle_id: o, rationale: "r" }));
    assert.throws(() => createProposal(db, id, items), /at most 5/);
    assert.throws(() => createProposal(db, id, items, { source: "import" }), /at most 5/);
    assert.ok(createProposal(db, id, items.slice(0, 5)) > 0);
  });
});

describe("accept", () => {
  test("applies the mutation, logs it, bumps revision, resolves the proposal", () => {
    const id = deckWith("Accept");
    const slot = createSlot(db, id, "Ramp");
    createProposal(db, id, [
      { action: "add", oracle_id: "id-solring", slot_id: slot, rationale: "best ramp in the format" },
    ]);
    const [item] = openItems(id);
    acceptItem(db, item.id);

    const state = getDeck(db, id);
    assert.equal(state.cards[0].name, "Sol Ring");
    assert.equal(state.cards[0].slot_id, slot);
    assert.equal(listProposals(db, id, "open").length, 0);

    const log = getLog(db, id) as any[];
    assert.equal(log[0].kind, "accept");
    assert.equal(log[0].card_name, "Sol Ring");
    assert.equal(log[0].rationale, "best ramp in the format");
    assert.equal(log[0].revision, 1);
  });
  test("atomic groups apply together or not at all", () => {
    const id = deckWith("Groups", ["id-counterspell"]);
    createProposal(db, id, [
      { action: "cut", oracle_id: "id-counterspell", rationale: "swap", group_id: "swap1" },
      { action: "add", oracle_id: "id-seedborn", rationale: "swap", group_id: "swap1" },
    ]);
    const items = openItems(id);
    acceptItem(db, items[0].id); // ruling on one item rules the group
    const cards = getDeck(db, id).cards.map((c) => c.name);
    assert.deepEqual(cards, ["Seedborn Muse"]);
    assert.equal(listProposals(db, id, "open").length, 0);
  });
  test("group acceptance rolls back wholly on failure", () => {
    const id = deckWith("Group rollback", ["id-counterspell"]);
    createProposal(db, id, [
      { action: "cut", oracle_id: "id-counterspell", rationale: "swap", group_id: "g" },
      { action: "add", oracle_id: "id-seedborn", rationale: "swap", group_id: "g" },
    ]);
    // Make the add fail by sneaking the card in before ruling
    addCard(db, id, "id-seedborn");
    const items = openItems(id);
    assert.throws(() => acceptItem(db, items[0].id), /already in the deck/);
    // The cut must NOT have been applied
    assert.ok(getDeck(db, id).cards.some((c) => c.name === "Counterspell"));
  });
});

describe("typed rejections route (spec §7.2)", () => {
  test("rejection requires a typed reason", () => {
    const id = deckWith("Reject");
    createProposal(db, id, [{ action: "add", oracle_id: "id-solring", rationale: "r" }]);
    const [item] = openItems(id);
    assert.throws(() => rejectItem(db, item.id, "soft", ""), /requires a reason/);
    assert.throws(() => rejectItem(db, item.id, "nope" as any, "reason"), /must be one of/);
  });
  test("hard_filter blacklists the card: search-level and re-propose blocked", () => {
    const id = deckWith("Hard filter");
    createProposal(db, id, [{ action: "add", oracle_id: "id-solring", rationale: "r" }]);
    rejectItem(db, openItems(id)[0].id, "hard_filter", "budget: already in three decks");

    assert.equal((listHardFilters(db, id) as any[])[0].card_name, "Sol Ring");
    assert.throws(
      () => createProposal(db, id, [{ action: "add", oracle_id: "id-solring", rationale: "again" }]),
      /hard-filtered/,
    );
    const visible = search(db, '!"sol ring"', { deckId: id, excludeHardFilters: true });
    assert.equal(visible.length, 0);
    // UI search without the flag still sees it
    assert.equal(search(db, '!"sol ring"', { deckId: id }).length, 1);

    removeHardFilter(db, id, "id-solring");
    assert.equal(
      createProposal(db, id, [{ action: "add", oracle_id: "id-solring", rationale: "ok now" }]) > 0,
      true,
    );
  });
  test("playtest_finding creates a durable card note", () => {
    const id = deckWith("Playtest", ["id-seedborn"]);
    createProposal(db, id, [{ action: "cut", oracle_id: "id-seedborn", rationale: "underperforms?" }]);
    rejectItem(db, openItems(id)[0].id, "playtest_finding", "won two games on untap value; stays");
    const notes = listCardNotes(db, id) as any[];
    assert.equal(notes[0].card_name, "Seedborn Muse");
    assert.match(notes[0].note, /won two games/);
  });
  test("thesis_change flags the log entry for brief review", () => {
    const id = deckWith("Thesis");
    createProposal(db, id, [{ action: "add", oracle_id: "id-counterspell", rationale: "protection" }]);
    rejectItem(db, openItems(id)[0].id, "thesis_change", "this deck doesn't run counterspells");
    const log = getLog(db, id) as any[];
    assert.equal(log[0].rejection_type, "thesis_change");
    assert.equal(log[0].brief_flag, 1);
  });
});

describe("pending delta", () => {
  test("computed state projects open proposals against 100 and slots", () => {
    const id = deckWith("Pending", ["id-counterspell"]);
    const slot = createSlot(db, id, "Ramp", 2, 4);
    updateCard(db, id, "id-counterspell", { slotId: null });
    createProposal(db, id, [
      { action: "add", oracle_id: "id-solring", slot_id: slot, rationale: "r" },
      { action: "add", oracle_id: "id-llanowar", slot_id: slot, rationale: "r" },
      { action: "cut", oracle_id: "id-counterspell", rationale: "r" },
    ]);
    const { computed } = getDeck(db, id);
    assert.equal(computed.pending.adds, 2);
    assert.equal(computed.pending.cuts, 1);
    assert.equal(computed.pending.projected_count, 2);
    assert.deepEqual(computed.pending.by_slot, { [slot]: 2 });
  });
});

describe("undo via log reversal (spec §7.4)", () => {
  test("undoing a cut restores slot, tags, owned, and quantity", () => {
    const id = deckWith("Undo cut");
    const slot = createSlot(db, id, "Ramp");
    const tag = createTag(db, id, "fast-mana");
    addCard(db, id, "id-solring", { slotId: slot });
    updateCard(db, id, "id-solring", { owned: true, tagIds: [tag] });

    createProposal(db, id, [{ action: "cut", oracle_id: "id-solring", rationale: "test cut" }]);
    acceptItem(db, openItems(id)[0].id);
    assert.equal(getDeck(db, id).cards.length, 0);

    const cutEntry = (getLog(db, id) as any[]).find((e) => e.kind === "accept");
    undoDecision(db, id, cutEntry.id);

    const card = getDeck(db, id).cards[0];
    assert.equal(card.name, "Sol Ring");
    assert.equal(card.slot_id, slot);
    assert.equal(card.owned, 1);
    assert.deepEqual(card.tag_ids, [tag]);
    // Original entry marked; double-undo refused
    assert.throws(() => undoDecision(db, id, cutEntry.id), /already been undone/);
  });
  test("undoing an add removes the card; only applied decisions can be undone", () => {
    const id = deckWith("Undo add");
    createProposal(db, id, [{ action: "add", oracle_id: "id-seedborn", rationale: "r" }]);
    acceptItem(db, openItems(id)[0].id);
    const entry = (getLog(db, id) as any[])[0];
    undoDecision(db, id, entry.id);
    assert.equal(getDeck(db, id).cards.length, 0);

    const rejectId = deckWith("Undo reject");
    createProposal(db, rejectId, [{ action: "add", oracle_id: "id-solring", rationale: "r" }]);
    rejectItem(db, openItems(rejectId)[0].id, "soft", "not yet");
    const rejectEntry = (getLog(db, rejectId) as any[])[0];
    assert.throws(() => undoDecision(db, rejectId, rejectEntry.id), /Only accepted/);
  });
});

describe("history and notes", () => {
  test("per-card history supports re-proposal citations (spec §7.3)", () => {
    const id = deckWith("History");
    createProposal(db, id, [{ action: "add", oracle_id: "id-seedborn", rationale: "v1" }]);
    rejectItem(db, openItems(id)[0].id, "soft", "mana base first");
    createProposal(db, id, [{ action: "add", oracle_id: "id-seedborn", rationale: "v2: mana settled" }]);
    acceptItem(db, openItems(id)[0].id);
    const history = getCardHistory(db, id, "id-seedborn") as any[];
    assert.deepEqual(
      history.map((h) => h.kind),
      ["accept", "reject"],
    );
  });
  test("manual card notes", () => {
    const id = deckWith("Notes", ["id-seedborn"]);
    addCardNote(db, id, "id-seedborn", "never cast before turn 6 across five games");
    assert.equal((listCardNotes(db, id) as any[]).length, 1);
  });
});
