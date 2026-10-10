import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { OWNER } from "../src/actor.ts";
import { listActivity, recordActivity } from "../src/activity.ts";
import { getBrief, setEngine, updateBrief } from "../src/deck/brief.ts";
import { checkDeck, isDoctorPair } from "../src/deck/checks.ts";
import { compareDecks, forkDeck } from "../src/deck/fork.ts";
import { cardHistory, listLog } from "../src/deck/log.ts";
import { paperStatus } from "../src/deck/paper.ts";
import { createProposal, pendingItemIds, rejectItems } from "../src/deck/proposals.ts";
import {
  createSlot,
  createTag,
  deleteDeck,
  getDeck,
  organizeCard,
  setAllInPaper,
  setDeckParent,
  setInPaper,
  shortlistCard,
} from "../src/deck/service.ts";
import { getFocus, setFocus } from "../src/focus.ts";
import { playerStats } from "../src/stats.ts";
import { AGENT, add, cut, newDeck, testDb } from "./helpers.ts";

describe("forks (spec §5.3)", () => {
  const db = testDb();
  const parent = newDeck(db, "active", "Parent");
  const ramp = createSlot(db, parent, "Ramp", 1, 2);
  const tag = createTag(db, parent, "rock");
  add(db, parent, "id-teferi", { role: "commander" });
  add(db, parent, "id-solring", { slotId: ramp });
  organizeCard(db, parent, "id-solring", { tagIds: [tag] });
  shortlistCard(db, parent, "id-counterspell", "maybe later");
  setAllInPaper(db, OWNER, parent, true);
  updateBrief(db, parent, { thesis: "Artifacts." });
  setEngine(db, parent, "Mana", "", [{ oracle_id: "id-solring" }]);
  const fork = forkDeck(db, AGENT, parent, "Fork");

  test("a fork is a draft that records its parent and copies list, organization and brief", () => {
    const view = getDeck(db, fork);
    assert.equal(view.deck.status, "draft");
    assert.deepEqual(view.deck.parent, { id: parent, name: "Parent" });
    assert.equal(view.deck.color_identity, "U");
    assert.deepEqual(view.slots.map((s) => [s.name, s.target_min, s.target_max]), [["Ramp", 1, 2]]);
    const ring = view.cards.find((c) => c.oracle_id === "id-solring")!;
    assert.equal(ring.slot_id, view.slots[0].id);
    assert.deepEqual(ring.tag_ids, [view.tags[0].id]);
    assert.notEqual(view.slots[0].id, ramp);
    assert.equal(view.cards.find((c) => c.oracle_id === "id-counterspell")!.maybe_note, "maybe later");
    assert.equal(getBrief(db, fork).thesis, "Artifacts.");
    assert.equal(getBrief(db, fork).engines[0].pieces[0].in_deck, true);
    assert.match(listLog(db, { deckId: fork })[0].summary, /Forked from "Parent"/);
  });
  test("the paper flag is not copied: the physical cards are in the parent", () => {
    assert.ok(getDeck(db, fork).cards.every((c) => c.in_paper === 0));
  });
  test("the two diverge, and compare defaults to the parent", () => {
    add(db, fork, "id-forest", { quantity: 2 });
    cut(db, fork, "id-solring");
    add(db, parent, "id-bog");
    const diff = compareDecks(db, fork);
    assert.equal(diff.b_is_parent, true);
    assert.deepEqual(diff.only_in_a.map((c) => c.name), ["Forest"]);
    assert.deepEqual(diff.only_in_b.map((c) => c.name).sort(), ["Bojuka Bog", "Sol Ring"]);
    assert.equal(diff.shared_count, 1);
    assert.throws(() => compareDecks(db, parent), /not a fork/);
    assert.throws(() => compareDecks(db, fork, fork), /different deck/);
  });
  // The case that started v2: a reworked copy whose agent knew nothing of the original.
  test("a fork inherits the rulings made in the deck it came from, labelled as such", () => {
    const original = newDeck(db, "active", "Ruled original");
    add(db, original, "id-teferi", { role: "commander" });
    const pid = createProposal(db, AGENT, original, {
      items: [{ action: "add", oracle_id: "id-counterspell", reason: "Interaction." }],
    });
    rejectItems(db, OWNER, pendingItemIds(db, pid), "No counterspells.");
    const rework = forkDeck(db, AGENT, original, "Ruled rework");
    const [inherited] = cardHistory(db, rework, "id-counterspell");
    assert.deepEqual(
      [inherited.kind, inherited.ruling_reason, inherited.from_deck],
      ["reject", "No counterspells.", "Ruled original"],
    );
    assert.equal(cardHistory(db, original, "id-counterspell")[0].from_deck, null);
    // It does not flow the other way: the original does not see the fork's history.
    add(db, rework, "id-solring");
    assert.equal(cardHistory(db, original, "id-solring").length, 0);
  });
  test("the owner can link a hand-made copy to its original, but not in a circle", () => {
    const original = newDeck(db, "active", "Original");
    const copy = newDeck(db, "active", "Hand copy");
    add(db, original, "id-solring");
    add(db, copy, "id-forest");
    assert.throws(() => setDeckParent(db, AGENT, copy, original), /Only the owner/);
    setDeckParent(db, OWNER, copy, original);
    assert.equal(compareDecks(db, copy).b.name, "Original");
    assert.throws(() => setDeckParent(db, OWNER, original, copy), /descended from "Original"/);
    assert.throws(() => setDeckParent(db, OWNER, copy, copy), /descended from/);
    setDeckParent(db, OWNER, copy, null);
    assert.equal(getDeck(db, copy).deck.parent, null);
  });
  test("deleting the parent leaves the fork standing with no parent", () => {
    const p = newDeck(db, "active");
    const f = forkDeck(db, OWNER, p, "Orphan");
    deleteDeck(db, OWNER, p);
    assert.equal(getDeck(db, f).deck.parent, null);
  });
});

describe("paper status (spec §5.5)", () => {
  test("missing cards split into a pull list and a buy list", () => {
    const db = testDb();
    const golems = newDeck(db, "active", "Golems");
    const green = newDeck(db, "active", "Green");
    add(db, golems, "id-solring");
    add(db, golems, "id-forest", { quantity: 3 });
    setAllInPaper(db, OWNER, golems, true);
    add(db, green, "id-solring");
    add(db, green, "id-llanowar");
    add(db, green, "id-seedborn");
    setInPaper(db, OWNER, green, "id-seedborn", true);
    shortlistCard(db, green, "id-forest");

    const status = paperStatus(db, green);
    assert.deepEqual([status.total_cards, status.in_paper_cards], [3, 1]);
    assert.deepEqual(status.pull, [
      { name: "Sol Ring", type_line: "Artifact", quantity: 1, held_by: [{ deck_id: golems, deck_name: "Golems" }] },
    ]);
    assert.deepEqual(status.buy, [{ name: "Llanowar Elves", type_line: "Creature — Elf Druid", quantity: 1 }]);
  });
});

describe("checks (spec §9)", () => {
  const db = testDb();
  const keys = (id: number) => checkDeck(db, id).map((f) => f.key);
  test("an empty deck reports its count and missing commander", () => {
    assert.deepEqual(keys(newDeck(db)), ["card_count", "no_commander"]);
  });
  test("legality, identity, copies, eligibility, pairing, slots and companion", () => {
    const id = newDeck(db);
    const slot = createSlot(db, id, "Ramp", 3, null);
    add(db, id, "id-teferi", { role: "commander" });
    add(db, id, "id-llanowar", { role: "commander" });
    add(db, id, "id-golos");
    add(db, id, "id-solring", { quantity: 2, slotId: slot });
    add(db, id, "id-goyf", { role: "companion" });
    const found = keys(id);
    for (const key of [
      "card_count",
      "commander_ineligible:id-llanowar",
      "commander_pair",
      "identity:id-golos",
      "banned:id-golos",
      "singleton:id-solring",
      `slot_under:${slot}`,
      "companion_unchecked:id-goyf",
    ])
      assert.ok(found.includes(key), `${key} in ${found.join(", ")}`);
  });
  test("the maybe list is outside every check", () => {
    const id = newDeck(db);
    add(db, id, "id-teferi", { role: "commander" });
    shortlistCard(db, id, "id-golos");
    assert.ok(!keys(id).some((k) => k.includes("id-golos")));
  });
  test("Doctor's companion pairs are recognized from type and text", () => {
    const doctor = { type_line: "Legendary Creature — Time Lord Doctor", oracle_text: "" };
    const companion = { type_line: "Legendary Creature — Human", oracle_text: "Doctor's companion (You can have two commanders if the other is the Doctor.)" };
    assert.equal(isDoctorPair([doctor, companion]), true);
    assert.equal(isDoctorPair([companion, companion]), false);
  });
});

describe("player statistics (spec §3)", () => {
  test("computed across decks, counting a fork and its parent as one family", () => {
    const db = testDb();
    const a = newDeck(db, "active", "Atraxa deck");
    add(db, a, "id-atraxa", { role: "commander" });
    add(db, a, "id-solring");
    add(db, a, "id-forest", { quantity: 2 });
    const b = newDeck(db, "active", "Teferi deck");
    add(db, b, "id-teferi", { role: "commander" });
    add(db, b, "id-solring");
    add(db, b, "id-counterspell");
    createSlot(db, b, "Interaction");
    const fork = forkDeck(db, OWNER, b, "Teferi fork");
    newDeck(db, "draft", "Empty");

    const stats = playerStats(db);
    assert.equal(stats.deck_count, 4);
    assert.equal(stats.families, 2);
    assert.deepEqual(stats.colors, { W: 1, U: 3, B: 1, R: 0, G: 1 });
    assert.deepEqual(stats.identities, [
      { identity: "U", decks: 2 },
      { identity: "WUBG", decks: 1 },
    ]);
    // Sol Ring is in three decks but two families; Counterspell is in one family.
    assert.deepEqual(stats.reused_cards, [{ name: "Sol Ring", type_line: "Artifact", decks: 2 }]);
    assert.ok(stats.commander_types.some((t) => t.type === "Teferi" && t.decks === 2));
    assert.equal(stats.decks.find((d) => d.id === fork)!.forked_from, "Teferi deck");
    assert.deepEqual(stats.slots, [{ name: "Interaction", decks: 2, average_count: 0 }]);
    assert.equal(stats.averages.lands, 0.7);
  });
});

describe("focus and activity", () => {
  const db = testDb();
  test("focus names the deck on screen and the selected cards", () => {
    assert.equal(getFocus(db), null);
    const id = newDeck(db, "active", "On screen");
    setFocus(db, id, ["id-solring", "id-nonexistent"]);
    const focus = getFocus(db)!;
    assert.deepEqual([focus.deck_id, focus.deck_name, focus.selected], [id, "On screen", ["Sol Ring"]]);
    assert.equal(focus.age_minutes, 0);
    deleteDeck(db, OWNER, id);
    assert.equal(getFocus(db), null);
    setFocus(db, null);
    assert.equal(getFocus(db), null);
  });
  test("only an agent's writes are activity", () => {
    recordActivity(db, OWNER, "remember", null, "owner wrote");
    recordActivity(db, AGENT, "remember", null, "Remembered: prefers long games");
    const feed = listActivity(db);
    assert.equal(feed.length, 1);
    assert.deepEqual([feed[0].actor, feed[0].action], ["test-agent", "remember"]);
  });
});
