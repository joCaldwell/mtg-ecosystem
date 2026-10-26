import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { PermissionError } from "../src/actor.ts";
import { OWNER } from "./actors.ts";
import { applyChanges, undo } from "../src/deck/membership.ts";
import {
  cardHistory,
  getLogEntry,
  listLog,
  markReviewed,
  unreviewedCount,
} from "../src/deck/log.ts";
import { createSlot, createTag, getDeck, organizeCard, setDeckCategory } from "../src/deck/service.ts";
import { AGENT, add, cut, newDeck, testDb } from "./helpers.ts";

const db = testDb();
const names = (id: number) => getDeck(db, OWNER, id).cards.filter((c) => !c.maybe).map((c) => c.name);

describe("one act, one revision, one log entry (spec §5.1, §8.2)", () => {
  test("a batch bumps the revision once and logs every card it touched", () => {
    const id = newDeck(db);
    const logId = applyChanges(
      db,
      OWNER,
      id,
      [
        { action: "add", oracle_id: "id-atraxa", role: "commander" },
        { action: "add", oracle_id: "id-solring" },
        { action: "add", oracle_id: "id-forest", quantity: 3 },
      ],
      { kind: "edit" },
    )!;
    assert.equal(getDeck(db, OWNER, id).deck.revision, 1);
    const entry = getLogEntry(db, OWNER, logId)!;
    assert.equal(entry.revision, 1);
    assert.equal(entry.kind, "edit");
    assert.equal(entry.author, "owner");
    assert.equal(entry.summary, "+5 cards");
    assert.deepEqual(entry.cards.map((c) => [c.card_name, c.action, c.quantity]), [
      ["Atraxa, Praetors' Voice", "add", 1],
      ["Sol Ring", "add", 1],
      ["Forest", "add", 3],
    ]);
  });
  test("a failing change rolls the whole batch back", () => {
    const id = newDeck(db);
    add(db, id, "id-solring");
    assert.throws(
      () =>
        applyChanges(
          db,
          OWNER,
          id,
          [
            { action: "add", oracle_id: "id-forest" },
            { action: "cut", oracle_id: "id-ball" },
          ],
          { kind: "edit" },
        ),
      /Ball Lightning is not in the deck/,
    );
    assert.deepEqual(names(id), ["Sol Ring"]);
    assert.equal(getDeck(db, OWNER, id).deck.revision, 1);
  });
  test("a change that changes nothing writes nothing", () => {
    const id = newDeck(db);
    add(db, id, "id-solring");
    const before = listLog(db, OWNER, { deckId: id }).length;
    assert.equal(
      applyChanges(db, OWNER, id, [{ action: "role", oracle_id: "id-solring", role: "card" }], {
        kind: "edit",
      }),
      null,
    );
    assert.equal(listLog(db, OWNER, { deckId: id }).length, before);
    assert.equal(getDeck(db, OWNER, id).deck.revision, 1);
  });
});

describe("Theorycrafted and Paper (spec §5.2)", () => {
  test("an agent edits Theorycrafted cards directly and is logged as its author", () => {
    const id = newDeck(db, "theorycrafted");
    const logId = applyChanges(db, AGENT, id, [{ action: "add", oracle_id: "id-solring" }], {
      kind: "edit",
    })!;
    assert.deepEqual(names(id), ["Sol Ring"]);
    assert.equal(getLogEntry(db, OWNER, logId)!.author, "test-agent");
  });
  test("an agent cannot change a Paper deck's cards, and is told what to do instead", () => {
    const id = newDeck(db, "paper");
    assert.throws(
      () => applyChanges(db, AGENT, id, [{ action: "add", oracle_id: "id-solring" }], { kind: "edit" }),
      (e: Error) => e instanceof PermissionError && /propose_changes/.test(e.message),
    );
    assert.deepEqual(names(id), []);
  });
  test("changing a deck to Theorycrafted reopens it to an agent; an agent can never rule or undo", () => {
    const id = newDeck(db, "paper");
    const logId = add(db, id, "id-solring")!;
    setDeckCategory(db, OWNER, id, "theorycrafted");
    applyChanges(db, AGENT, id, [{ action: "add", oracle_id: "id-forest" }], { kind: "edit" });
    assert.deepEqual(names(id), ["Forest", "Sol Ring"]);
    assert.throws(() => undo(db, AGENT, logId), PermissionError);
    assert.throws(
      () => applyChanges(db, AGENT, id, [{ action: "add", oracle_id: "id-ball" }], { kind: "accept" }),
      PermissionError,
    );
  });
});

describe("undo is the reversal of a log entry (spec §8.2)", () => {
  test("undoing an add removes it; undoing a cut restores slot and tags", () => {
    const id = newDeck(db);
    const slot = createSlot(db, OWNER, id, "Ramp");
    const tag = createTag(db, OWNER, id, "rock");
    const addId = add(db, id, "id-solring", { slotId: slot })!;
    organizeCard(db, OWNER, id, "id-solring", { tagIds: [tag] });

    const cutId = cut(db, id, "id-solring")!;
    assert.deepEqual(names(id), []);
    const result = undo(db, OWNER, cutId);
    assert.deepEqual(result.skipped, []);
    const card = getDeck(db, OWNER, id).cards[0];
    assert.deepEqual([card.slot_id, card.tag_ids], [slot, [tag]]);
    assert.equal(getLogEntry(db, OWNER, cutId)!.undone_by, result.log_id);
    assert.equal(getLogEntry(db, OWNER, result.log_id)!.kind, "undo");

    assert.throws(() => undo(db, OWNER, cutId), /already been undone/);
    assert.throws(() => undo(db, OWNER, result.log_id), /cannot itself be undone/);

    undo(db, OWNER, addId);
    assert.deepEqual(names(id), []);
  });
  test("a batch is undone as a batch, and each undo is its own revision", () => {
    const id = newDeck(db);
    add(db, id, "id-teferi", { role: "commander" });
    const batch = applyChanges(
      db,
      OWNER,
      id,
      [
        { action: "cut", oracle_id: "id-teferi" },
        { action: "add", oracle_id: "id-atraxa", role: "commander" },
        { action: "add", oracle_id: "id-forest", quantity: 4 },
      ],
      { kind: "import", summary: "Imported a list" },
    )!;
    assert.equal(getDeck(db, OWNER, id).deck.color_identity, "WUBG");
    undo(db, OWNER, batch);
    const view = getDeck(db, OWNER, id);
    assert.deepEqual(names(id), ["Teferi, Temporal Archmage"]);
    assert.equal(view.cards[0].role, "commander");
    assert.equal(view.deck.color_identity, "U");
    assert.equal(view.deck.revision, 3);
  });
  test("what the deck has moved past is skipped and reported, not forced", () => {
    const id = newDeck(db);
    const batch = applyChanges(
      db,
      OWNER,
      id,
      [
        { action: "add", oracle_id: "id-solring" },
        { action: "add", oracle_id: "id-forest", quantity: 2 },
      ],
      { kind: "edit" },
    )!;
    cut(db, id, "id-solring");
    const result = undo(db, OWNER, batch);
    assert.deepEqual(result.skipped, ["Sol Ring is no longer in the deck"]);
    assert.deepEqual(names(id), []);
  });
  test("parking and promoting reverse into each other", () => {
    const id = newDeck(db);
    add(db, id, "id-solring");
    const parked = cut(db, id, "id-solring", { park: true })!;
    assert.equal(getDeck(db, OWNER, id).cards[0].maybe, 1);
    undo(db, OWNER, parked);
    assert.equal(getDeck(db, OWNER, id).cards[0].maybe, 0);
  });
  test("a rejection has nothing to undo", () => {
    const id = newDeck(db);
    add(db, id, "id-solring");
    const entry = listLog(db, OWNER, { deckId: id })[0];
    db.prepare("UPDATE log SET kind = 'reject' WHERE id = ?").run(entry.id);
    assert.throws(() => undo(db, OWNER, entry.id), /nothing to undo/);
  });
});

describe("prior decisions follow the card (spec §8.3)", () => {
  test("history is per card and per deck, newest first, and marks what was undone", () => {
    const a = newDeck(db);
    const b = newDeck(db);
    const addId = add(db, a, "id-solring")!;
    undo(db, OWNER, addId);
    add(db, b, "id-solring");

    const history = cardHistory(db, OWNER, a, "id-solring");
    assert.deepEqual(history.map((h) => [h.kind, h.action, h.undone]), [
      ["undo", "cut", false],
      ["edit", "add", true],
    ]);
    assert.equal(cardHistory(db, OWNER, b, "id-solring").length, 1);
    assert.deepEqual(cardHistory(db, OWNER, a, "id-forest"), []);
  });
});

describe("the review queue (spec §8.4)", () => {
  test("the owner's changes to a Paper deck wait for an agent; draft assembly and agent edits do not", () => {
    const fresh = testDb();
    const active = newDeck(fresh, "paper");
    const draft = newDeck(fresh, "theorycrafted");
    add(fresh, draft, "id-solring");
    applyChanges(fresh, AGENT, draft, [{ action: "add", oracle_id: "id-forest" }], { kind: "edit" });
    assert.equal(unreviewedCount(fresh, OWNER), 0);

    const first = add(fresh, active, "id-solring")!;
    const second = add(fresh, active, "id-forest")!;
    assert.equal(unreviewedCount(fresh, OWNER), 2);
    assert.deepEqual(
      listLog(fresh, OWNER, { unreviewedOnly: true }).map((e) => e.id),
      [second, first],
    );
    assert.equal(markReviewed(fresh, OWNER, { ids: [first] }), 1);
    assert.equal(unreviewedCount(fresh, OWNER), 1);
    assert.equal(markReviewed(fresh, OWNER, { through: second }), 1);
    assert.equal(unreviewedCount(fresh, OWNER), 0);
    assert.throws(() => markReviewed(fresh, OWNER, {}), /ids of the decisions/);
  });
});
