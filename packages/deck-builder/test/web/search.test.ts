import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { cardMarks } from "../../web/src/domain/marks.ts";
import { annotateResults } from "../../web/src/domain/search.ts";
import { card, deckState, item, logEntry, memory, proposal, searchResult } from "./fixtures.ts";

const logCard = (name: string) => ({
  oracle_id: `oid-${name}`, card_name: name, action: "add" as const, quantity: 1, item_id: null, reason: "",
});

describe("annotateResults", () => {
  test("in the deck, on the maybe list, or neither", () => {
    const state = deckState({
      cards: [card({ name: "Test Card A" }), card({ name: "Test Card B", maybe: 1 })],
    });
    const [a, b, c] = annotateResults(
      [searchResult({ name: "Test Card A" }), searchResult({ name: "Test Card B" }), searchResult({ name: "Test Card C" })],
      state,
      new Map(),
    );
    assert.deepEqual([a.inDeck, a.onMaybe], [true, false]);
    assert.deepEqual([b.inDeck, b.onMaybe], [false, true]);
    assert.deepEqual([c.inDeck, c.onMaybe], [false, false]);
  });

  test("pending proposal items mark the card; resolved ones do not", () => {
    const state = deckState({
      proposals: [
        proposal([
          item({ action: "add", name: "Test Card A" }),
          item({ action: "cut", name: "Test Card B" }),
          item({ action: "add", name: "Test Card C", status: "rejected" }),
        ]),
      ],
    });
    const out = annotateResults(
      ["Test Card A", "Test Card B", "Test Card C", "Test Card D"].map((name) => searchResult({ name })),
      state,
      new Map(),
    );
    assert.deepEqual(out.map((r) => r.proposed), ["add", "cut", null, null]);
  });

  test("the last rejection is the newest reject entry naming the card, with the owner's reason", () => {
    const state = deckState({
      // Newest first, as the server sends it.
      log: [
        logEntry({ kind: "accept", revision: 9, ruling_reason: "ignored", cards: [logCard("Test Card A")] }),
        logEntry({ kind: "reject", revision: 7, ruling_reason: "too slow", cards: [logCard("Test Card A")] }),
        logEntry({ kind: "reject", revision: 3, ruling_reason: "older", cards: [logCard("Test Card A"), logCard("Test Card B")] }),
      ],
    });
    const [a, b, c] = annotateResults(
      ["Test Card A", "Test Card B", "Test Card C"].map((name) => searchResult({ name })),
      state,
      new Map(),
    );
    assert.deepEqual(a.lastRejection, { revision: 7, reason: "too slow" });
    assert.deepEqual(b.lastRejection, { revision: 3, reason: "older" });
    assert.equal(c.lastRejection, null);
  });

  test("memory counts come from the marks", () => {
    const state = deckState();
    const marks = cardMarks(state, [
      memory({ cards: ["oid-Test Card A"] }),
      memory({ cards: ["oid-Test Card A"] }),
    ]);
    const [a, b] = annotateResults(
      [searchResult({ name: "Test Card A" }), searchResult({ name: "Test Card B" })],
      state,
      marks,
    );
    assert.equal(a.memories, 2);
    assert.equal(b.memories, 0);
  });

  test("the card itself is passed through and order is kept", () => {
    const results = [searchResult({ name: "Test Card B" }), searchResult({ name: "Test Card A" })];
    const out = annotateResults(results, deckState(), new Map());
    assert.deepEqual(out.map((r) => r.card), results);
  });
});

describe("annotateResults ordering", () => {
  test("Commander-legal cards come first, each part keeping the server's order", () => {
    const out = annotateResults(
      [
        searchResult({ name: "Test Card A", commander_legality: "not_legal" }),
        searchResult({ name: "Test Card B" }),
        searchResult({ name: "Test Card C", commander_legality: "banned" }),
        searchResult({ name: "Test Card D" }),
      ],
      deckState({}),
      new Map(),
    );
    assert.deepEqual(out.map((r) => r.card.name), ["Test Card B", "Test Card D", "Test Card A", "Test Card C"]);
    assert.deepEqual(out.map((r) => r.legal), [true, true, false, false]);
  });
});
