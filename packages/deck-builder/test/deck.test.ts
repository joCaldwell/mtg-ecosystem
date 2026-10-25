import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { search } from "../src/search/index.ts";
import { ServiceError } from "../src/errors.ts";
import { OWNER, PermissionError } from "../src/actor.ts";
import { applyChanges, setQuantity } from "../src/deck/membership.ts";
import {
  copyLimit,
  createDeck,
  createSlot,
  createTag,
  defineSlots,
  deleteDeck,
  deleteSlot,
  deleteTag,
  fileCards,
  getDeck,
  listDecks,
  organizeCard,
  renameDeck,
  renameTag,
  setDeckCategory,
  shortlistCard,
  similarTag,
  unshortlistCard,
  updateSlot,
  validateVocabName,
} from "../src/deck/service.ts";
import { AGENT, add, cut, newDeck, testDb } from "./helpers.ts";

const db = testDb();

describe("deck lifecycle", () => {
  test("create, list, rename, delete", () => {
    const id = createDeck(db, OWNER, "Test CRUD");
    assert.ok(listDecks(db).some((d) => d.id === id && d.name === "Test CRUD"));
    renameDeck(db, OWNER, id, "Renamed");
    assert.equal(getDeck(db, id).deck.name, "Renamed");
    deleteDeck(db, OWNER, id);
    assert.throws(() => getDeck(db, id), ServiceError);
  });
  test("duplicate names rejected", () => {
    createDeck(db, OWNER, "Dupe");
    assert.throws(() => createDeck(db, OWNER, "dupe"), /already exists/);
  });
  // Spec §5.2: a new deck is a draft; only the owner changes that.
  test("new decks are Theorycrafted, and only the owner changes lifecycle", () => {
    const id = createDeck(db, OWNER, "Status");
    assert.equal(getDeck(db, id).deck.category, "theorycrafted");
    assert.throws(() => setDeckCategory(db, AGENT, id, "paper"), PermissionError);
    setDeckCategory(db, OWNER, id, "paper");
    assert.equal(getDeck(db, id).deck.category, "paper");
  });
  test("the roster carries commanders, counts and pending work", () => {
    const id = newDeck(db, "paper", "Roster");
    add(db, id, "id-atraxa", { role: "commander" });
    add(db, id, "id-solring");
    const row = listDecks(db).find((d) => d.id === id)!;
    assert.deepEqual(row.commanders, ["Atraxa, Praetors' Voice"]);
    assert.equal(row.card_count, 2);
    assert.equal(row.color_identity, "WUBG");
    assert.equal(row.pending_items, 0);
  });
  test("only the owner changes lifecycle, preserving membership and revisions", () => {
    const id = newDeck(db, "paper", "Classification");
    add(db, id, "id-solring");
    const before = getDeck(db, id);
    assert.equal(before.deck.category, "paper");
    for (const category of ["paper", "legacy", "theorycrafted"] as const) {
      assert.throws(() => setDeckCategory(db, AGENT, id, category), PermissionError);
      setDeckCategory(db, OWNER, id, category);
      const after = getDeck(db, id);
      assert.equal(after.deck.category, category);
      assert.equal(listDecks(db).find((d) => d.id === id)!.category, category);
      assert.ok(!("status" in after.deck));
      assert.equal(after.deck.revision, before.deck.revision);
      assert.deepEqual(after.cards, before.cards);
      if (category !== "theorycrafted") assert.throws(() => applyChanges(db, AGENT, id, [{ action: "cut", oracle_id: "id-solring" }], { kind: "edit" }), PermissionError);
    }
    assert.throws(() => setDeckCategory(db, OWNER, id, "invalid" as any), ServiceError);
    assert.equal(getDeck(db, id).deck.category, "theorycrafted");
    assert.throws(() => setDeckCategory(db, OWNER, -1, "paper"), ServiceError);
  });
});

describe("commanders and color identity", () => {
  test("commander sets deck identity; violations surface, never block", () => {
    const id = newDeck(db);
    add(db, id, "id-atraxa", { role: "commander" });
    assert.equal(getDeck(db, id).deck.color_identity, "WUBG");

    add(db, id, "id-ball"); // red card in a WUBG deck
    assert.deepEqual(
      getDeck(db, id).computed.identity_violations.map((v) => v.name),
      ["Ball Lightning"],
    );

    // removing the commander clears identity and therefore violations
    cut(db, id, "id-atraxa");
    assert.equal(getDeck(db, id).computed.identity_violations.length, 0);
    assert.equal(getDeck(db, id).deck.ci_mask, null);
  });
  test("at most two command-zone cards, one companion", () => {
    const id = newDeck(db);
    add(db, id, "id-atraxa", { role: "commander" });
    add(db, id, "id-kenrith", { role: "commander" });
    assert.throws(() => add(db, id, "id-teferi", { role: "commander" }), /at most two/);
    add(db, id, "id-goyf", { role: "companion" });
    add(db, id, "id-teferi");
    assert.throws(
      () =>
        applyChanges(db, OWNER, id, [{ action: "role", oracle_id: "id-teferi", role: "companion" }], {
          kind: "edit",
        }),
      /at most one companion/,
    );
  });
  test("a role change recomputes identity and preserves its purpose", () => {
    const id = newDeck(db);
    const slot = createSlot(db, id, "Card Advantage");
    add(db, id, "id-teferi", { slotId: slot });
    applyChanges(db, OWNER, id, [{ action: "role", oracle_id: "id-teferi", role: "commander" }], {
      kind: "edit",
    });
    const view = getDeck(db, id);
    assert.equal(view.deck.color_identity, "U");
    assert.equal(view.cards[0].slot_id, slot);
  });
});

describe("slots", () => {
  test("targets, deltas, and statuses", () => {
    const id = newDeck(db);
    const ramp = createSlot(db, id, "Ramp", 2, 3);
    createSlot(db, id, "Card Advantage", 2, null);
    add(db, id, "id-llanowar", { slotId: ramp });
    add(db, id, "id-solring", { slotId: ramp });
    add(db, id, "id-seedborn");

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
    const id = newDeck(db);
    assert.throws(() => createSlot(db, id, "land"), /reserved/);
    assert.throws(() => createSlot(db, id, "Or"), /reserved/);
    assert.throws(() => validateVocabName("tag"), /reserved/);
    createSlot(db, id, "Interaction");
    assert.throws(() => createSlot(db, id, "interaction"), /already exists/);
    assert.throws(() => createSlot(db, id, "Wipes", 5, 2), /min cannot exceed max/);
  });
  // Slots categorise by role, card type is the decklist's other grouping, and
  // a slot named "lands" would make the deck's land count ambiguous.
  test("card types are rejected in the plural too, and only as whole names", () => {
    const id = newDeck(db);
    for (const name of ["lands", "Creatures", "artifacts", "sorceries", "Planeswalkers"])
      assert.throws(() => createSlot(db, id, name), /card type/, name);
    createSlot(db, id, "creature removal");
    createSlot(db, id, "land destruction");
    assert.deepEqual(
      getDeck(db, id).slots.map((s) => s.name),
      ["creature removal", "land destruction"],
    );
  });
  test("renaming a slot to a card type is rejected", () => {
    const id = newDeck(db);
    const s = createSlot(db, id, "mana base", 36, 38);
    assert.throws(() => updateSlot(db, id, s, { name: "lands" }), /card type/);
    assert.equal(getDeck(db, id).slots[0].name, "mana base");
  });
  test("deleting a slot unslots its cards", () => {
    const id = newDeck(db);
    const s = createSlot(db, id, "Removal");
    add(db, id, "id-counterspell", { slotId: s });
    deleteSlot(db, id, s);
    const { cards, computed } = getDeck(db, id);
    assert.equal(cards[0].slot_id, null);
    assert.equal(computed.unslotted_count, 1);
  });
  test("defineSlots creates, retargets, renames and removes in one call, or none of it", () => {
    const id = newDeck(db);
    defineSlots(db, id, [
      { name: "Ramp", target_min: 8, target_max: 10 },
      { name: "Interaction", target_min: 10 },
    ]);
    defineSlots(db, id, [
      { name: "ramp", target_min: 9 },
      { name: "Interaction", rename_to: "Answers" },
    ]);
    assert.deepEqual(
      getDeck(db, id).slots.map((s) => [s.name, s.target_min, s.target_max]),
      [["Ramp", 9, 10], ["Answers", 10, null]],
    );
    // One bad entry and the whole call is refused.
    assert.throws(
      () => defineSlots(db, id, [{ name: "Draw" }, { name: "lands" }]),
      /card type/,
    );
    assert.equal(getDeck(db, id).slots.length, 2);
    defineSlots(db, id, [{ name: "Answers", remove: true }]);
    assert.deepEqual(getDeck(db, id).slots.map((s) => s.name), ["Ramp"]);
    assert.throws(() => defineSlots(db, id, [{ name: "Nope", remove: true }]), /no slot named/);
  });
});

describe("tags", () => {
  test("vocabulary with reserved names; cascade on delete", () => {
    const id = newDeck(db);
    const t = createTag(db, id, "wincon");
    assert.throws(() => createTag(db, id, "creature"), /reserved/);
    add(db, id, "id-atraxa");
    organizeCard(db, id, "id-atraxa", { tagIds: [t] });
    assert.deepEqual(getDeck(db, id).cards[0].tag_ids, [t]);
    deleteTag(db, id, t);
    assert.deepEqual(getDeck(db, id).cards[0].tag_ids, []);
  });
  // Spec §5.4: without this the vocabulary grows three spellings of one idea.
  test("a tag that closely matches an existing one is refused, naming it", () => {
    const id = newDeck(db);
    createTag(db, id, "Card Draw");
    for (const dupe of ["card draw", "card-draw", "Card Draws", "draw card"])
      assert.throws(() => createTag(db, id, dupe), /already has the tag 'Card Draw'/, dupe);
    // A different idea that merely shares a word is a judgement call, and allowed.
    createTag(db, id, "draw");
    assert.equal(similarTag(["ramp"], "mana ramp"), null);
  });
  test("fileCards files by name in bulk, creating tags, all or nothing", () => {
    const id = newDeck(db);
    createSlot(db, id, "Ramp");
    add(db, id, "id-llanowar");
    add(db, id, "id-solring");
    fileCards(db, id, [
      { oracle_id: "id-llanowar", slot: "ramp", add_tags: ["dork"] },
      { oracle_id: "id-solring", slot: "Ramp", add_tags: ["rock", "dork"] },
    ]);
    const view = getDeck(db, id);
    assert.ok(view.cards.every((c) => c.slot_id === view.slots[0].id));
    assert.deepEqual(view.tags.map((t) => t.name), ["dork", "rock"]);

    const revision = view.deck.revision;
    assert.throws(
      () =>
        fileCards(db, id, [
          { oracle_id: "id-llanowar", slot: null },
          { oracle_id: "id-solring", slot: "Nowhere" },
        ]),
      /no slot named 'Nowhere'.*Its slots are: Ramp/,
    );
    // The first entry did not land either, and filing never bumps the revision.
    assert.ok(getDeck(db, id).cards.every((c) => c.slot_id != null));
    assert.equal(getDeck(db, id).deck.revision, revision);

    fileCards(db, id, [{ oracle_id: "id-solring", remove_tags: ["dork"] }]);
    const ring = getDeck(db, id).cards.find((c) => c.oracle_id === "id-solring")!;
    assert.equal(ring.tag_ids.length, 1);
  });
  test("rename enforces the creation vocabulary rule and preserves assignments", () => {
    const id = newDeck(db);
    createTag(db, id, "Card Draw");
    const tag = createTag(db, id, "Engine");
    add(db, id, "id-solring");
    organizeCard(db, id, "id-solring", { tagIds: [tag] });
    for (const name of ["card-draw", "draw card", "Card Draws"])
      assert.throws(() => renameTag(db, id, tag, name), /already has the tag 'Card Draw'/);
    renameTag(db, id, tag, "Engines");
    assert.deepEqual(getDeck(db, id).cards[0].tag_ids, [tag]);
    assert.equal(getDeck(db, id).tags.find((t) => t.id === tag)!.name, "Engines");
  });
  test("filing is atomic, scoped to a deck, and replaces a deduplicated tag set", () => {
    const id = newDeck(db);
    const other = newDeck(db);
    const first = createTag(db, id, "Engine");
    const second = createTag(db, id, "Need to get");
    const foreign = createTag(db, other, "Engine");
    const slot = createSlot(db, id, "Ramp");
    add(db, id, "id-solring");
    organizeCard(db, id, "id-solring", { tagIds: [first, first, second] });
    const before = getDeck(db, id);
    assert.equal(before.cards[0].tag_ids.length, 2);
    assert.throws(() => organizeCard(db, id, "id-solring", { slotId: slot, tagIds: [foreign] }), /not found in deck/);
    for (const invalid of [null, "1", [0], [-1], [1.5], ["1"]])
      assert.throws(() => organizeCard(db, id, "id-solring", { tagIds: invalid as any }), /array of positive integers/);
    assert.deepEqual(getDeck(db, id), before, "a failed tag update must not change the slot");
    assert.throws(() => db.prepare("INSERT INTO deck_card_tags VALUES (?, ?, ?)").run(id, "id-solring", foreign), /FOREIGN KEY/);
    assert.throws(() => db.prepare("INSERT INTO deck_card_tags VALUES (?, ?, ?)").run(id, "id-atraxa", first), /FOREIGN KEY/);
    organizeCard(db, id, "id-solring", { tagIds: [second] });
    assert.deepEqual(getDeck(db, id).cards[0].tag_ids, [second]);
    organizeCard(db, id, "id-solring", { tagIds: [] });
    assert.deepEqual(getDeck(db, id).cards[0].tag_ids, []);
    assert.equal(getDeck(db, id).deck.revision, before.deck.revision);
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
    const id = newDeck(db);
    add(db, id, "id-forest", { quantity: 5 });
    add(db, id, "id-solring");
    add(db, id, "id-solring");
    add(db, id, "id-dwarves", { quantity: 8 });

    const { computed, cards } = getDeck(db, id);
    assert.equal(cards.find((c) => c.oracle_id === "id-forest")!.quantity, 5);
    assert.equal(computed.card_count, 15);
    const violations = Object.fromEntries(
      computed.singleton_violations.map((v) => [v.name, v]),
    );
    assert.ok(!("Forest" in violations));
    assert.equal(violations["Sol Ring"].limit, 1);
    assert.equal(violations["Seven Dwarves"].limit, 7);

    setQuantity(db, OWNER, id, "id-dwarves", 7);
    assert.ok(
      !getDeck(db, id).computed.singleton_violations.some((v) => v.name === "Seven Dwarves"),
    );
  });
});

describe("computed state", () => {
  test("counts, curve, pips, lands; companion excluded from the 100", () => {
    const id = newDeck(db);
    add(db, id, "id-atraxa", { role: "commander" }); // {G}{W}{U}{B} cmc 4
    add(db, id, "id-counterspell"); // {U}{U} cmc 2
    add(db, id, "id-forest");
    add(db, id, "id-goyf", { role: "companion" });

    const { computed } = getDeck(db, id);
    assert.equal(computed.card_count, 3);
    assert.equal(computed.delta_to_100, -97);
    assert.equal(computed.land_count, 1);
    assert.equal(computed.average_mana_value, 3);
    assert.equal(computed.curve["2"], 1);
    assert.equal(computed.curve["4"], 1);
    assert.equal(computed.pips.U, 3);
    assert.equal(computed.pips.G, 1);
  });
  test("average mana value weights copies and excludes lands and the maybe list", () => {
    const id = newDeck(db);
    assert.equal(getDeck(db, id).computed.average_mana_value, null);
    add(db, id, "id-forest");
    assert.equal(getDeck(db, id).computed.average_mana_value, null);
    add(db, id, "id-llanowar", { quantity: 3 });
    add(db, id, "id-seedborn");
    shortlistCard(db, id, "id-teferi");
    assert.equal(getDeck(db, id).computed.average_mana_value, 2);
  });
});

describe("the maybe list", () => {
  test("shortlisting is organization: no revision, outside every count", () => {
    const id = newDeck(db);
    add(db, id, "id-solring");
    const revision = getDeck(db, id).deck.revision;
    shortlistCard(db, id, "id-ball", "possible finisher");
    const view = getDeck(db, id);
    assert.equal(view.deck.revision, revision);
    assert.equal(view.computed.card_count, 1);
    assert.equal(view.computed.maybe_count, 1);
    assert.equal(view.cards.find((c) => c.oracle_id === "id-ball")!.maybe_note, "possible finisher");
    assert.throws(() => shortlistCard(db, id, "id-solring"), /already in the deck/);
    unshortlistCard(db, id, "id-ball");
    assert.equal(getDeck(db, id).computed.maybe_count, 0);
    assert.throws(() => unshortlistCard(db, id, "id-solring"), /membership change/);
  });
  test("promoting keeps the slot; parking is a cut that keeps the card around", () => {
    const id = newDeck(db);
    const slot = createSlot(db, id, "Ramp");
    shortlistCard(db, id, "id-llanowar");
    organizeCard(db, id, "id-llanowar", { slotId: slot });
    add(db, id, "id-llanowar");
    let card = getDeck(db, id).cards[0];
    assert.equal(card.maybe, 0);
    assert.equal(card.slot_id, slot);
    assert.equal(getDeck(db, id).computed.card_count, 1);

    cut(db, id, "id-llanowar", { park: true });
    card = getDeck(db, id).cards[0];
    assert.equal(card.maybe, 1);
    assert.equal(card.slot_id, slot);
    assert.equal(getDeck(db, id).computed.card_count, 0);
  });
});

describe("deck-context search (slot:/tag:, identity pre-filter)", () => {
  test("slot:, slot:none, and tag: filters", () => {
    const id = newDeck(db);
    const ramp = createSlot(db, id, "Ramp");
    const t = createTag(db, id, "combo");
    add(db, id, "id-llanowar", { slotId: ramp });
    add(db, id, "id-seedborn");
    organizeCard(db, id, "id-seedborn", { tagIds: [t] });

    assert.deepEqual(search(db, "slot:ramp", { deckId: id }).map((r) => r.name), ["Llanowar Elves"]);
    assert.deepEqual(search(db, "slot:none", { deckId: id }).map((r) => r.name), ["Seedborn Muse"]);
    assert.deepEqual(search(db, "tag:combo", { deckId: id }).map((r) => r.name), ["Seedborn Muse"]);
  });
  test("deckId applies the deck's identity as pre-filter (spec §4.2)", () => {
    const id = newDeck(db);
    add(db, id, "id-teferi", { role: "commander" }); // mono-U
    const r = search(db, "t:creature or t:instant", { deckId: id }).map((x) => x.name);
    assert.ok(r.includes("Counterspell"));
    assert.ok(!r.includes("Llanowar Elves"));
    // no commanders -> unfiltered
    const id2 = newDeck(db);
    const r2 = search(db, "t:creature", { deckId: id2 }).map((x) => x.name);
    assert.ok(r2.includes("Llanowar Elves"));
  });
});
