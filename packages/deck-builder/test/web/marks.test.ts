import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { cardMarks, paperMarksUseful } from "../../web/src/domain/marks.ts";
import { card, deckState, logEntry, memory } from "./fixtures.ts";

const A = "oid-Test Card A";
const B = "oid-Test Card B";
const logCard = (oracle_id: string) => ({
  oracle_id, card_name: oracle_id, action: "add" as const, quantity: 1, item_id: null, reason: "",
});

describe("cardMarks, memories", () => {
  test("global memories and this deck's count", () => {
    const state = deckState();
    const marks = cardMarks(state, [
      memory({ scope: "global", cards: [A] }),
      memory({ scope: "deck", deck_id: state.deck.id, cards: [A, B] }),
    ]);
    assert.deepEqual(marks.get(A), { memories: 2, rulings: 0 });
    assert.deepEqual(marks.get(B), { memories: 1, rulings: 0 });
  });

  test("superseded and retired memories do not count", () => {
    const marks = cardMarks(deckState(), [
      memory({ status: "superseded", cards: [A] }),
      memory({ status: "retired", cards: [A] }),
    ]);
    assert.equal(marks.size, 0);
  });

  test("another deck's memories do not count", () => {
    const state = deckState();
    const marks = cardMarks(state, [memory({ scope: "deck", deck_id: state.deck.id + 1, cards: [A] })]);
    assert.equal(marks.size, 0);
  });

  test("a memory linked to no card marks nothing", () => {
    assert.equal(cardMarks(deckState(), [memory({ cards: [] })]).size, 0);
  });
});

describe("cardMarks, rulings", () => {
  test("accept and reject entries count, one per entry", () => {
    const state = deckState({
      log: [
        logEntry({ kind: "accept", cards: [logCard(A), logCard(A)] }),
        logEntry({ kind: "reject", cards: [logCard(A), logCard(B)] }),
      ],
    });
    const marks = cardMarks(state, []);
    assert.deepEqual(marks.get(A), { memories: 0, rulings: 2 });
    assert.deepEqual(marks.get(B), { memories: 0, rulings: 1 });
  });

  test("undone entries and other kinds of entry do not count", () => {
    const state = deckState({
      log: [
        logEntry({ kind: "accept", undone_by: 9, cards: [logCard(A)] }),
        logEntry({ kind: "edit", cards: [logCard(A)] }),
        logEntry({ kind: "import", cards: [logCard(A)] }),
        logEntry({ kind: "undo", cards: [logCard(A)] }),
      ],
    });
    assert.equal(cardMarks(state, []).size, 0);
  });

  test("memories and rulings combine on one card", () => {
    const state = deckState({ log: [logEntry({ kind: "reject", cards: [logCard(A)] })] });
    const marks = cardMarks(state, [memory({ cards: [A] })]);
    assert.deepEqual(marks.get(A), { memories: 1, rulings: 1 });
  });
});

describe("paperMarksUseful", () => {
  test("only once at least half the deck is in paper; the maybe list does not count", () => {
    const deck = (inPaper: number, missing: number) =>
      deckState({
        cards: [
          ...Array.from({ length: inPaper }, (_, i) => card({ name: `Test Card P${i}`, in_paper: 1 })),
          ...Array.from({ length: missing }, (_, i) => card({ name: `Test Card M${i}`, in_paper: 0 })),
          card({ name: "Test Card Maybe", maybe: 1, in_paper: 0 }),
        ],
      });
    assert.equal(paperMarksUseful(deck(3, 1)), true);
    assert.equal(paperMarksUseful(deck(2, 2)), true);
    assert.equal(paperMarksUseful(deck(1, 3)), false);
    assert.equal(paperMarksUseful(deckState({ cards: [] })), false);
  });
});
