import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { OWNER, PermissionError } from "../src/actor.ts";
import { applyChanges, undo } from "../src/deck/membership.ts";
import {
  cardHistory,
  getLogEntry,
  listLog,
  markReviewed,
  unreviewedCount,
} from "../src/deck/log.ts";
import { createSlot, createTag, getDeck, organizeCard, setDeckStatus, setInPaper } from "../src/deck/service.ts";
import { AGENT, add, cut, newDeck, testDb } from "./helpers.ts";

const db = testDb();
const names = (id: number) => getDeck(db, id).cards.filter((c) => !c.maybe).map((c) => c.name);

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
    assert.equal(getDeck(db, id).deck.revision, 1);
    const entry = getLogEntry(db, logId)!;
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
    assert.equal(getDeck(db, id).deck.revision, 1);
  });
  test("a change that changes nothing writes nothing", () => {
    const id = newDeck(db);
    add(db, id, "id-solring");
    const before = listLog(db, { deckId: id }).length;
    assert.equal(
      applyChanges(db, OWNER, id, [{ action: "role", oracle_id: "id-solring", role: "card" }], {
        kind: "edit",
      }),
      null,
    );
    assert.equal(listLog(db, { deckId: id }).length, before);
    assert.equal(getDeck(db, id).deck.revision, 1);
  });
});

describe("draft and active (spec §5.2)", () => {
  test("an agent edits a draft directly and is logged as its author", () => {
    const id = newDeck(db, "draft");
    const logId = applyChanges(db, AGENT, id, [{ action: "add", oracle_id: "id-solring" }], {
      kind: "edit",
    })!;
    assert.deepEqual(names(id), ["Sol Ring"]);
    assert.equal(getLogEntry(db, logId)!.author, "test-agent");
  });
  test("an agent cannot change an active deck's cards, and is told what to do instead", () => {
    const id = newDeck(db, "active");
    assert.throws(
      () => applyChanges(db, AGENT, id, [{ action: "add", oracle_id: "id-solring" }], { kind: "edit" }),
      (e: Error) => e instanceof PermissionError && /propose_changes/.test(e.message),
    );
    assert.deepEqual(names(id), []);
  });
  test("returning a deck to draft reopens it to an agent; an agent can never rule or undo", () => {
    const id = newDeck(db, "active");
    const logId = add(db, id, "id-solring")!;
    setDeckStatus(db, OWNER, id, "draft");
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
  test("undoing an add removes it; undoing a cut restores slot, tags and paper status", () => {
    const id = newDeck(db);
    const slot = createSlot(db, id, "Ramp");
    const tag = createTag(db, id, "rock");
    const addId = add(db, id, "id-solring", { slotId: slot })!;
    organizeCard(db, id, "id-solring", { tagIds: [tag] });
    setInPaper(db, OWNER, id, "id-solring", true);

    const cutId = cut(db, id, "id-solring")!;
    assert.deepEqual(names(id), []);
    const result = undo(db, OWNER, cutId);
    assert.deepEqual(result.skipped, []);
    const card = getDeck(db, id).cards[0];
    assert.deepEqual([card.slot_id, card.tag_ids, card.in_paper], [slot, [tag], 1]);
    assert.equal(getLogEntry(db, cutId)!.undone_by, result.log_id);
    assert.equal(getLogEntry(db, result.log_id)!.kind, "undo");

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
    assert.equal(getDeck(db, id).deck.color_identity, "WUBG");
    undo(db, OWNER, batch);
    const view = getDeck(db, id);
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
    assert.equal(getDeck(db, id).cards[0].maybe, 1);
    undo(db, OWNER, parked);
    assert.equal(getDeck(db, id).cards[0].maybe, 0);
  });
  test("a rejection has nothing to undo", () => {
    const id = newDeck(db);
    add(db, id, "id-solring");
    const entry = listLog(db, { deckId: id })[0];
    db.prepare("UPDATE log SET kind = 'reject' WHERE id = ?").run(entry.id);
    assert.throws(() => undo(db, OWNER, entry.id), /nothing to undo/);
  });
});

describe("prior rulings follow the card (spec §8.3)", () => {
  test("history is per card and per deck, newest first, and marks what was undone", () => {
    const a = newDeck(db);
    const b = newDeck(db);
    const addId = add(db, a, "id-solring")!;
    undo(db, OWNER, addId);
    add(db, b, "id-solring");

    const history = cardHistory(db, a, "id-solring");
    assert.deepEqual(history.map((h) => [h.kind, h.action, h.undone]), [
      ["undo", "cut", false],
      ["edit", "add", true],
    ]);
    assert.equal(cardHistory(db, b, "id-solring").length, 1);
    assert.deepEqual(cardHistory(db, a, "id-forest"), []);
  });
});

describe("the review queue (spec §8.4)", () => {
  test("the owner's changes to an active deck wait for an agent; draft assembly and agent edits do not", () => {
    const fresh = testDb();
    const active = newDeck(fresh, "active");
    const draft = newDeck(fresh, "draft");
    add(fresh, draft, "id-solring");
    applyChanges(fresh, AGENT, draft, [{ action: "add", oracle_id: "id-forest" }], { kind: "edit" });
    assert.equal(unreviewedCount(fresh), 0);

    const first = add(fresh, active, "id-solring")!;
    const second = add(fresh, active, "id-forest")!;
    assert.equal(unreviewedCount(fresh), 2);
    assert.deepEqual(
      listLog(fresh, { unreviewedOnly: true }).map((e) => e.id),
      [second, first],
    );
    assert.equal(markReviewed(fresh, { ids: [first] }), 1);
    assert.equal(unreviewedCount(fresh), 1);
    assert.equal(markReviewed(fresh, { through: second }), 1);
    assert.equal(unreviewedCount(fresh), 0);
    assert.throws(() => markReviewed(fresh, {}), /ids of the rulings/);
  });
});
