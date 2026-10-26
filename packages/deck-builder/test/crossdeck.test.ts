import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { OWNER } from "./actors.ts";
import { listActivity, recordActivity } from "../src/activity.ts";
import { getBrief, setEngine, updateBrief } from "../src/deck/brief.ts";
import { checkDeck, isDoctorPair } from "../src/deck/checks.ts";
import { compareDecks, forkDeck } from "../src/deck/fork.ts";
import { cardHistory, listLog } from "../src/deck/log.ts";
import { createProposal, pendingItemIds, rejectItems } from "../src/deck/proposals.ts";
import {
  createSlot,
  createTag,
  deleteDeck,
  getDeck,
  organizeCard,
  setDeckCategory,
  setDeckParent,
  shortlistCard,
} from "../src/deck/service.ts";
import { getFocus, setFocus } from "../src/focus.ts";
import { playerStats } from "../src/stats.ts";
import { AGENT, add, cut, newDeck, testDb } from "./helpers.ts";

describe("forks (spec §5.3)", () => {
  const db = testDb();
  const parent = newDeck(db, "paper", "Parent");
  const ramp = createSlot(db, OWNER, parent, "Ramp", 1, 2);
  const tag = createTag(db, OWNER, parent, "rock");
  add(db, parent, "id-teferi", { role: "commander" });
  add(db, parent, "id-solring", { slotId: ramp });
  organizeCard(db, OWNER, parent, "id-solring", { tagIds: [tag] });
  shortlistCard(db, OWNER, parent, "id-counterspell", "maybe later");
  setDeckCategory(db, OWNER, parent, "paper");
  updateBrief(db, OWNER, parent, { thesis: "Artifacts." });
  setEngine(db, OWNER, parent, "Mana", "", [{ oracle_id: "id-solring" }]);
  const fork = forkDeck(db, AGENT, parent, "Fork");

  test("a fork is Theorycrafted and records its parent and copies list, organization and brief", () => {
    const view = getDeck(db, OWNER, fork);
    assert.ok(!("status" in view.deck));
    assert.equal(view.deck.category, "theorycrafted");
    assert.equal(getDeck(db, OWNER, parent).deck.category, "paper");
    assert.deepEqual(view.deck.parent, { id: parent, name: "Parent" });
    assert.equal(view.deck.color_identity, "U");
    assert.deepEqual(view.slots.map((s) => [s.name, s.target_min, s.target_max]), [["Ramp", 1, 2]]);
    const ring = view.cards.find((c) => c.oracle_id === "id-solring")!;
    assert.equal(ring.slot_id, view.slots[0].id);
    assert.deepEqual(ring.tag_ids, [view.tags[0].id]);
    assert.notEqual(view.slots[0].id, ramp);
    assert.equal(view.cards.find((c) => c.oracle_id === "id-counterspell")!.maybe_note, "maybe later");
    assert.equal(getBrief(db, OWNER, fork).thesis, "Artifacts.");
    assert.equal(getBrief(db, OWNER, fork).engines[0].pieces[0].in_deck, true);
    assert.match(listLog(db, OWNER, { deckId: fork })[0].summary, /Forked from "Parent"/);
  });

  test("the two diverge, and compare defaults to the parent", () => {
    add(db, fork, "id-forest", { quantity: 2 });
    cut(db, fork, "id-solring");
    add(db, parent, "id-bog");
    const diff = compareDecks(db, OWNER, fork);
    assert.equal(diff.b_is_parent, true);
    assert.deepEqual(diff.only_in_a.map((c) => c.name), ["Forest"]);
    assert.deepEqual(diff.only_in_b.map((c) => c.name).sort(), ["Bojuka Bog", "Sol Ring"]);
    assert.equal(diff.shared_count, 1);
    assert.throws(() => compareDecks(db, OWNER, parent), /not a fork/);
    assert.throws(() => compareDecks(db, OWNER, fork, fork), /different deck/);
  });
  // The case that started v2: a reworked copy whose agent knew nothing of the original.
  test("a fork inherits the decisions made in the deck it came from, labelled as such", () => {
    const original = newDeck(db, "paper", "Ruled original");
    add(db, original, "id-teferi", { role: "commander" });
    const pid = createProposal(db, AGENT, original, {
      items: [{ action: "add", oracle_id: "id-counterspell", reason: "Interaction." }],
    });
    rejectItems(db, OWNER, pendingItemIds(db, OWNER, pid), "No counterspells.");
    const rework = forkDeck(db, AGENT, original, "Ruled rework");
    const [inherited] = cardHistory(db, OWNER, rework, "id-counterspell");
    assert.deepEqual(
      [inherited.kind, inherited.decision_reason, inherited.from_deck],
      ["reject", "No counterspells.", "Ruled original"],
    );
    assert.equal(cardHistory(db, OWNER, original, "id-counterspell")[0].from_deck, null);
    // It does not flow the other way: the original does not see the fork's history.
    add(db, rework, "id-solring");
    assert.equal(cardHistory(db, OWNER, original, "id-solring").length, 0);
  });
  test("the owner can link a hand-made copy to its original, but not in a circle", () => {
    const original = newDeck(db, "paper", "Original");
    const copy = newDeck(db, "paper", "Hand copy");
    add(db, original, "id-solring");
    add(db, copy, "id-forest");
    assert.throws(() => setDeckParent(db, AGENT, copy, original), /Only the owner/);
    setDeckParent(db, OWNER, copy, original);
    assert.equal(compareDecks(db, OWNER, copy).b.name, "Original");
    assert.throws(() => setDeckParent(db, OWNER, original, copy), /descended from "Original"/);
    assert.throws(() => setDeckParent(db, OWNER, copy, copy), /descended from/);
    setDeckParent(db, OWNER, copy, null);
    assert.equal(getDeck(db, OWNER, copy).deck.parent, null);
  });
  test("deleting the parent leaves the fork standing with no parent", () => {
    const p = newDeck(db, "paper");
    const f = forkDeck(db, OWNER, p, "Orphan");
    deleteDeck(db, OWNER, p);
    assert.equal(getDeck(db, OWNER, f).deck.parent, null);
  });
});

describe("checks (spec §9)", () => {
  const db = testDb();
  const keys = (id: number) => checkDeck(db, OWNER, id).map((f) => f.key);
  test("an empty deck reports its count and missing commander", () => {
    assert.deepEqual(keys(newDeck(db)), ["card_count", "no_commander"]);
  });
  test("legality, identity, copies, eligibility, pairing, slots and companion", () => {
    const id = newDeck(db);
    const slot = createSlot(db, OWNER, id, "Ramp", 3, null);
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
    shortlistCard(db, OWNER, id, "id-golos");
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
  test("computed across Paper decks, counting a fork and its parent as one family", () => {
    const db = testDb();
    const a = newDeck(db, "paper", "Atraxa deck");
    add(db, a, "id-atraxa", { role: "commander" });
    add(db, a, "id-solring");
    add(db, a, "id-forest", { quantity: 2 });
    const b = newDeck(db, "paper", "Teferi deck");
    add(db, b, "id-teferi", { role: "commander" });
    add(db, b, "id-solring");
    add(db, b, "id-counterspell");
    createSlot(db, OWNER, b, "Interaction");
    const fork = forkDeck(db, OWNER, b, "Teferi fork");
    const empty = newDeck(db, "theorycrafted", "Empty");
    for (const id of [a, b, fork, empty]) setDeckCategory(db, OWNER, id, "paper");

    const stats = playerStats(db, OWNER);
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
  test("Theorycrafted and Archived decks contribute to none of the statistics", () => {
    const db = testDb();
    const paper = newDeck(db, "theorycrafted", "Paper stats");
    add(db, paper, "id-teferi", { role: "commander" });
    add(db, paper, "id-solring");
    createSlot(db, OWNER, paper, "Ramp");
    setDeckCategory(db, OWNER, paper, "paper");
    const baseline = playerStats(db, OWNER);
    for (const category of ["theorycrafted", "archived"] as const) {
      const excluded = newDeck(db, "paper", `${category} stats`);
      add(db, excluded, "id-atraxa", { role: "commander" });
      add(db, excluded, "id-solring");
      add(db, excluded, "id-forest", { quantity: 10 });
      createSlot(db, OWNER, excluded, "Excluded slot");
      setDeckCategory(db, OWNER, excluded, category);
      assert.deepEqual(playerStats(db, OWNER), baseline);
      setDeckCategory(db, OWNER, excluded, "paper");
      assert.equal(playerStats(db, OWNER).deck_count, 2);
      assert.equal(playerStats(db, OWNER).reused_cards.length, 1);
      setDeckCategory(db, OWNER, excluded, category);
      assert.deepEqual(playerStats(db, OWNER), baseline);
    }
    setDeckCategory(db, OWNER, paper, "archived");
    const empty = playerStats(testDb(), OWNER);
    assert.deepEqual(playerStats(db, OWNER), empty);
  });
  test("Paper forks share a family through excluded ancestors", () => {
    const db = testDb();
    const root = newDeck(db, "paper", "Legacy root");
    add(db, root, "id-solring");
    setDeckCategory(db, OWNER, root, "archived");
    const middle = forkDeck(db, OWNER, root, "Theorycrafted middle");
    const a = forkDeck(db, OWNER, middle, "Paper descendant A");
    const b = forkDeck(db, OWNER, root, "Paper descendant B");
    for (const id of [a, b]) setDeckCategory(db, OWNER, id, "paper");
    const stats = playerStats(db, OWNER);
    assert.equal(stats.deck_count, 2);
    assert.equal(stats.families, 1);
    assert.deepEqual(stats.decks.map((d) => d.id), [a, b]);
    assert.deepEqual(stats.reused_cards, []);
  });
});

describe("focus and activity", () => {
  const db = testDb();
  test("focus names the deck on screen and the selected cards", () => {
    assert.equal(getFocus(db, OWNER), null);
    const id = newDeck(db, "paper", "On screen");
    setFocus(db, OWNER, id, ["id-solring", "id-nonexistent"]);
    const focus = getFocus(db, OWNER)!;
    assert.deepEqual([focus.deck_id, focus.deck_name, focus.selected], [id, "On screen", ["Sol Ring"]]);
    assert.equal(focus.age_minutes, 0);
    deleteDeck(db, OWNER, id);
    assert.equal(getFocus(db, OWNER), null);
    setFocus(db, OWNER, null);
    assert.equal(getFocus(db, OWNER), null);
  });
  test("only an agent's writes are activity", () => {
    recordActivity(db, OWNER, "remember", null, "owner wrote");
    recordActivity(db, AGENT, "remember", null, "Remembered: prefers long games");
    const feed = listActivity(db, OWNER);
    assert.equal(feed.length, 1);
    assert.deepEqual([feed[0].actor, feed[0].action], ["test-agent", "remember"]);
  });
});
