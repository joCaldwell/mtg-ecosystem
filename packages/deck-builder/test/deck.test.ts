import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { openDb } from "../src/db.ts";
import { ingestCards } from "../src/ingest.ts";
import { FIXTURES } from "./fixtures.ts";
import { search } from "../src/search/index.ts";
import {
  ServiceError,
  addCard,
  copyLimit,
  createDeck,
  createSlot,
  createTag,
  deleteDeck,
  deleteSlot,
  deleteTag,
  getDeck,
  listDecks,
  removeCard,
  renameDeck,
  updateCard,
  updateSlot,
  validateVocabName,
} from "../src/deck/service.ts";

const db = openDb(":memory:");
ingestCards(db, FIXTURES);

function freshDeck(name: string): number {
  return createDeck(db, name);
}

describe("deck CRUD", () => {
  test("create, list, rename, delete", () => {
    const id = freshDeck("Test CRUD");
    assert.ok(listDecks(db).some((d) => d.id === id && d.name === "Test CRUD"));
    renameDeck(db, id, "Renamed");
    assert.equal(getDeck(db, id).deck.name, "Renamed");
    deleteDeck(db, id);
    assert.throws(() => getDeck(db, id), ServiceError);
  });
  test("duplicate names rejected", () => {
    freshDeck("Dupe");
    assert.throws(() => freshDeck("dupe"), /already exists/);
  });
});

describe("commanders and color identity", () => {
  test("commander sets deck identity; violations surface, never block", () => {
    const id = freshDeck("Atraxa");
    addCard(db, id, "id-atraxa", { role: "commander" });
    assert.equal(getDeck(db, id).deck.color_identity, "WUBG");

    addCard(db, id, "id-ball"); // red card in a WUBG deck
    const { computed } = getDeck(db, id);
    assert.deepEqual(
      computed.identity_violations.map((v) => v.name),
      ["Ball Lightning"],
    );

    // removing the commander clears identity and therefore violations
    removeCard(db, id, "id-atraxa");
    assert.equal(getDeck(db, id).computed.identity_violations.length, 0);
    assert.equal(getDeck(db, id).deck.ci_mask, null);
  });
  test("at most two command-zone cards, one companion", () => {
    const id = freshDeck("Partners");
    addCard(db, id, "id-atraxa", { role: "commander" });
    addCard(db, id, "id-kenrith", { role: "commander" });
    assert.throws(() => addCard(db, id, "id-teferi", { role: "commander" }), /at most two/);
    addCard(db, id, "id-goyf", { role: "companion" });
    addCard(db, id, "id-teferi");
    assert.throws(
      () => updateCard(db, id, "id-teferi", { role: "companion" }),
      /at most one companion/,
    );
  });
  test("role change recomputes identity", () => {
    const id = freshDeck("Role change");
    addCard(db, id, "id-teferi");
    updateCard(db, id, "id-teferi", { role: "commander" });
    assert.equal(getDeck(db, id).deck.color_identity, "U");
  });
});

describe("slots", () => {
  test("targets, deltas, and statuses", () => {
    const id = freshDeck("Slots");
    const ramp = createSlot(db, id, "Ramp", 2, 3);
    const draw = createSlot(db, id, "Card Advantage", 2, null);
    addCard(db, id, "id-llanowar", { slotId: ramp });
    addCard(db, id, "id-solring", { slotId: ramp });
    addCard(db, id, "id-seedborn");

    const { computed } = getDeck(db, id);
    const byName = Object.fromEntries(computed.slot_deltas.map((s) => [s.name, s]));
    assert.equal(byName["Ramp"].status, "ok");
    assert.equal(byName["Card Advantage"].status, "under");
    assert.equal(byName["Card Advantage"].delta, -2);
    assert.equal(computed.unslotted_count, 1);

    updateSlot(db, id, ramp, { targetMin: 0, targetMax: 1 });
    assert.equal(
      getDeck(db, id).computed.slot_deltas.find((s) => s.slot_id === ramp)!.status,
      "over",
    );
  });
  test("reserved and duplicate names rejected; min>max rejected", () => {
    const id = freshDeck("Slot names");
    assert.throws(() => createSlot(db, id, "land"), /reserved/);
    assert.throws(() => createSlot(db, id, "Or"), /reserved/);
    assert.throws(() => validateVocabName("tag"), /reserved/);
    createSlot(db, id, "Interaction");
    assert.throws(() => createSlot(db, id, "interaction"), /already exists/);
    assert.throws(() => createSlot(db, id, "Wipes", 5, 2), /min cannot exceed max/);
  });
  test("deleting a slot unslots its cards", () => {
    const id = freshDeck("Slot delete");
    const s = createSlot(db, id, "Removal");
    addCard(db, id, "id-counterspell", { slotId: s });
    deleteSlot(db, id, s);
    const { cards, computed } = getDeck(db, id);
    assert.equal(cards[0].slot_id, null);
    assert.equal(computed.unslotted_count, 1);
  });
});

describe("tags", () => {
  test("controlled vocabulary with reserved names; cascade on delete", () => {
    const id = freshDeck("Tags");
    const t = createTag(db, id, "wincon");
    assert.throws(() => createTag(db, id, "creature"), /reserved/);
    addCard(db, id, "id-atraxa");
    updateCard(db, id, "id-atraxa", { tagIds: [t] });
    assert.deepEqual(getDeck(db, id).cards[0].tag_ids, [t]);
    deleteTag(db, id, t);
    assert.deepEqual(getDeck(db, id).cards[0].tag_ids, []);
  });
});

describe("quantities and singleton rules", () => {
  test("copyLimit: basics and printed exceptions", () => {
    assert.equal(copyLimit({ type_line: "Basic Land — Forest" }), Infinity);
    assert.equal(copyLimit({ type_line: "Creature — Rat", oracle_text: "A deck can have any number of cards named Relentless Rats." }), Infinity);
    assert.equal(copyLimit({ type_line: "Creature — Dwarf", oracle_text: "A deck can have up to seven cards named Seven Dwarves." }), 7);
    assert.equal(copyLimit({ type_line: "Artifact" }), 1);
  });
  test("re-adding bumps quantity; violations computed against the limit", () => {
    const id = freshDeck("Quantities");
    for (let i = 0; i < 5; i++) addCard(db, id, "id-forest");
    addCard(db, id, "id-solring");
    addCard(db, id, "id-solring");
    for (let i = 0; i < 8; i++) addCard(db, id, "id-dwarves");

    const { computed, cards } = getDeck(db, id);
    assert.equal(cards.find((c) => c.oracle_id === "id-forest")!.quantity, 5);
    assert.equal(computed.card_count, 15);
    const violations = Object.fromEntries(
      computed.singleton_violations.map((v) => [v.name, v]),
    );
    assert.ok(!("Forest" in violations));
    assert.equal(violations["Sol Ring"].limit, 1);
    assert.equal(violations["Seven Dwarves"].limit, 7);

    updateCard(db, id, "id-dwarves", { quantity: 7 });
    assert.ok(
      !getDeck(db, id).computed.singleton_violations.some((v) => v.name === "Seven Dwarves"),
    );
  });
});

describe("computed state", () => {
  test("counts, curve, pips, lands; companion excluded from the 100", () => {
    const id = freshDeck("Computed");
    addCard(db, id, "id-atraxa", { role: "commander" }); // {G}{W}{U}{B} cmc 4
    addCard(db, id, "id-counterspell"); // {U}{U} cmc 2
    addCard(db, id, "id-forest");
    addCard(db, id, "id-goyf", { role: "companion" });

    const { computed } = getDeck(db, id);
    assert.equal(computed.card_count, 3);
    assert.equal(computed.delta_to_100, -97);
    assert.equal(computed.land_count, 1);
    assert.equal(computed.curve["2"], 1);
    assert.equal(computed.curve["4"], 1);
    assert.equal(computed.pips.U, 3);
    assert.equal(computed.pips.G, 1);
  });
  test("owned flag stored and returned", () => {
    const id = freshDeck("Owned");
    addCard(db, id, "id-solring");
    updateCard(db, id, "id-solring", { owned: true });
    assert.equal(getDeck(db, id).cards[0].owned, 1);
  });
});

describe("deck-context search (slot:/tag:, identity pre-filter)", () => {
  test("slot:, slot:none, and tag: filters", () => {
    const id = freshDeck("Search ctx");
    const ramp = createSlot(db, id, "Ramp");
    const t = createTag(db, id, "combo");
    addCard(db, id, "id-llanowar", { slotId: ramp });
    addCard(db, id, "id-seedborn");
    updateCard(db, id, "id-seedborn", { tagIds: [t] });

    assert.deepEqual(search(db, "slot:ramp", { deckId: id }).map((r) => r.name), ["Llanowar Elves"]);
    assert.deepEqual(search(db, "slot:none", { deckId: id }).map((r) => r.name), ["Seedborn Muse"]);
    assert.deepEqual(search(db, "tag:combo", { deckId: id }).map((r) => r.name), ["Seedborn Muse"]);
  });
  test("deckId applies the deck's identity as pre-filter (spec §6.3)", () => {
    const id = freshDeck("Identity filter");
    addCard(db, id, "id-teferi", { role: "commander" }); // mono-U
    const r = search(db, "t:creature or t:instant", { deckId: id }).map((x) => x.name);
    assert.ok(r.includes("Counterspell"));
    assert.ok(!r.includes("Llanowar Elves"));
    // no commanders -> unfiltered
    const id2 = freshDeck("No commander");
    const r2 = search(db, "t:creature", { deckId: id2 }).map((x) => x.name);
    assert.ok(r2.includes("Llanowar Elves"));
  });
});
