import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { PermissionError } from "../src/actor.ts";
import { OWNER } from "./actors.ts";
import { forkDeck } from "../src/deck/fork.ts";
import {
  getBudgets,
  getMemory,
  listMemories,
  loadScope,
  memoriesForCards,
  remember,
  reviseMemory,
  setBudgets,
} from "../src/memory.ts";
import { AGENT, add, newDeck, testDb } from "./helpers.ts";

describe("writing memories (spec §7.3)", () => {
  const db = testDb();
  test("an agent writes without approval; what it concludes is a hypothesis by default", () => {
    const { memory } = remember(db, AGENT, { text: "Prefers games that go long.", scope: "global" });
    assert.deepEqual(
      [memory.author, memory.basis, memory.owner_touched, memory.status],
      ["test-agent", "inferred", false, "active"],
    );
    const stated = remember(db, AGENT, {
      text: "Playgroup bans infinite combos.",
      scope: "global",
      basis: "stated",
    }).memory;
    assert.equal(stated.basis, "stated");
    assert.equal(remember(db, OWNER, { text: "I own no dual lands.", scope: "global" }).memory.basis, "stated");
  });
  test("a deck memory needs its deck and carries the revision it was written at", () => {
    const id = newDeck(db);
    add(db, id, "id-solring");
    assert.throws(() => remember(db, AGENT, { text: "x", scope: "deck" }), /needs the deck/);
    const { memory } = remember(db, AGENT, {
      text: "Never found the engine before turn nine in five goldfishes.",
      scope: "deck",
      deck_id: id,
    });
    assert.equal(memory.deck_revision, 1);
  });
  test("mentioning a card links it; invented cards and essays are refused", () => {
    const { memory } = remember(db, AGENT, {
      text: "Finds [[Sol Ring]] boring and cuts it on sight.",
      scope: "global",
      cards: ["id-llanowar"],
    });
    assert.deepEqual(memory.cards.map((c) => c.name), ["Llanowar Elves", "Sol Ring"]);
    assert.throws(
      () => remember(db, AGENT, { text: "Loves [[Seedborn Sage]].", scope: "global" }),
      /is not a card name/,
    );
    assert.throws(
      () => remember(db, AGENT, { text: "x".repeat(1300), scope: "global" }),
      /one statement and its reason/,
    );
  });
  test("saving returns what resembles it, and an exact repeat is refused", () => {
    const fresh = testDb();
    const first = remember(fresh, AGENT, {
      text: "Dislikes counterspells in casual pods.",
      scope: "global",
    }).memory;
    const second = remember(fresh, AGENT, {
      text: "Has rejected every counterspell proposed so far.",
      scope: "global",
    });
    assert.deepEqual(second.similar.map((m) => m.id), [first.id]);
    assert.throws(
      () => remember(fresh, AGENT, { text: "dislikes counterspells in casual pods", scope: "global" }),
      new RegExp(`Memory #${first.id} already says exactly this`),
    );
  });
});

describe("revising (spec §7.3)", () => {
  const db = testDb();
  test("superseding keeps the old row, marked and linked; retiring hides without deleting", () => {
    const old = remember(db, AGENT, { text: "Budget is fifty dollars a deck.", scope: "global" }).memory;
    const replacement = reviseMemory(db, AGENT, old.id, {
      action: "supersede",
      text: "Budget is one hundred dollars a deck.",
    });
    const kept = getMemory(db, OWNER, old.id);
    assert.deepEqual([kept.status, kept.superseded_by], ["superseded", replacement.id]);
    assert.deepEqual(listMemories(db, OWNER, { scope: "global" }).map((m) => m.id), [replacement.id]);

    reviseMemory(db, AGENT, replacement.id, { action: "retire" });
    assert.equal(listMemories(db, OWNER, { scope: "global" }).length, 0);
    assert.equal(listMemories(db, OWNER, { scope: "global", status: "any" }).length, 2);
    assert.equal(reviseMemory(db, OWNER, replacement.id, { action: "restore" }).status, "active");
  });
  test("superseding can promote a deck memory to global", () => {
    const id = newDeck(db);
    const local = remember(db, AGENT, { text: "Cut the tutors here.", scope: "deck", deck_id: id }).memory;
    const promoted = reviseMemory(db, AGENT, local.id, {
      action: "supersede",
      text: "Cuts tutors from every deck: wants games to vary.",
      scope: "global",
    });
    assert.deepEqual([promoted.scope, promoted.deck_id], ["global", null]);
  });
  test("the owner outranks the agent", () => {
    const mine = remember(db, OWNER, { text: "No stax, ever.", scope: "global" }).memory;
    for (const revision of [
      { action: "update", text: "Some stax." },
      { action: "supersede", text: "Stax is fine." },
      { action: "retire" },
    ] as const)
      assert.throws(() => reviseMemory(db, AGENT, mine.id, revision), PermissionError);

    // An agent memory the owner edits becomes the owner's too.
    const theirs = remember(db, AGENT, { text: "Seems to like tribal decks.", scope: "global" }).memory;
    reviseMemory(db, OWNER, theirs.id, { action: "update", text: "Likes tribal decks, especially elves." });
    assert.equal(getMemory(db, OWNER, theirs.id).owner_touched, true);
    assert.throws(() => reviseMemory(db, AGENT, theirs.id, { action: "retire" }), /cannot be changed by an agent/);
  });
  test("confirming turns a hypothesis into a stated fact without changing who may edit it", () => {
    const m = remember(db, AGENT, { text: "Probably avoids land destruction.", scope: "global" }).memory;
    const confirmed = reviseMemory(db, AGENT, m.id, { action: "confirm" });
    assert.deepEqual([confirmed.basis, confirmed.owner_touched], ["stated", false]);
    reviseMemory(db, AGENT, m.id, { action: "update", text: "Avoids land destruction." });
  });
});

describe("surfacing (spec §7.2)", () => {
  const db = testDb();
  const a = newDeck(db);
  const b = newDeck(db);
  remember(db, AGENT, { text: "[[Sol Ring]] is banned in the playgroup.", scope: "global", basis: "stated" });
  remember(db, AGENT, { text: "[[Sol Ring]] was dead in three of five opens here.", scope: "deck", deck_id: a });

  test("a global card memory follows the card everywhere; a deck one stays in its deck", () => {
    assert.equal(memoriesForCards(db, OWNER, ["id-solring"], a).get("id-solring")!.length, 2);
    assert.equal(memoriesForCards(db, OWNER, ["id-solring"], b).get("id-solring")!.length, 1);
    assert.equal(memoriesForCards(db, OWNER, ["id-solring"]).get("id-solring")!.length, 1);
    assert.equal(memoriesForCards(db, OWNER, ["id-forest"], a).size, 0);
  });
  test("search is full-text, filterable by scope, deck and card", () => {
    assert.equal(listMemories(db, OWNER, { query: "playgroup banned" }).length, 1);
    assert.equal(listMemories(db, OWNER, { query: "opens", scope: "global" }).length, 0);
    assert.equal(listMemories(db, OWNER, { cardId: "id-solring" }).length, 2);
    assert.equal(listMemories(db, OWNER, { deckId: a }).length, 1);
    assert.deepEqual(listMemories(db, OWNER, { query: "the and" }), []);
  });
});

describe("budgets (spec §7.4)", () => {
  test("a scope loads the newest that fit and says how many it left out", () => {
    const db = testDb();
    setBudgets(db, OWNER, { global: 100 });
    assert.deepEqual(getBudgets(db, OWNER), { global: 100, deck: 1000 });
    for (let i = 0; i < 8; i++)
      remember(db, AGENT, { text: `Preference number ${i}: ${"detail ".repeat(12)}`, scope: "global" });
    const loaded = loadScope(db, OWNER, "global");
    assert.ok(loaded.memories.length > 0 && loaded.memories.length < 8);
    assert.equal(loaded.omitted, 8 - loaded.memories.length);
    assert.equal(loaded.over, true);
    assert.ok(loaded.used <= 100);
    // Newest first.
    assert.match(loaded.memories[0].text, /number 7/);
    assert.throws(() => setBudgets(db, OWNER, { deck: 5 }), /between 100 and 20000/);
  });
  test("a scope within budget is not flagged", () => {
    const db = testDb();
    remember(db, AGENT, { text: "Short.", scope: "global" });
    assert.deepEqual(
      (({ omitted, over }) => ({ omitted, over }))(loadScope(db, OWNER, "global")),
      { omitted: 0, over: false },
    );
  });
});

describe("forks copy deck memories (spec §5.3)", () => {
  test("the copies diverge from the originals", () => {
    const db = testDb();
    const parent = newDeck(db);
    add(db, parent, "id-solring");
    const original = remember(db, AGENT, {
      text: "[[Sol Ring]] stays: the curve needs it.",
      scope: "deck",
      deck_id: parent,
    }).memory;
    const fork = forkDeck(db, AGENT, parent, "Fork");
    const [copy] = listMemories(db, OWNER, { deckId: fork });
    assert.notEqual(copy.id, original.id);
    assert.equal(copy.text, original.text);
    assert.deepEqual(copy.cards.map((c) => c.name), ["Sol Ring"]);
    reviseMemory(db, AGENT, copy.id, { action: "retire" });
    assert.equal(listMemories(db, OWNER, { deckId: parent }).length, 1);
    assert.equal(listMemories(db, OWNER, { deckId: fork }).length, 0);
  });
});
