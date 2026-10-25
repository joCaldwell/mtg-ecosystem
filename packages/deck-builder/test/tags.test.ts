import { test } from "node:test";
import assert from "node:assert/strict";
import { OWNER } from "./actors.ts";
import { forkDeck } from "../src/deck/fork.ts";
import { undo } from "../src/deck/membership.ts";
import { createTag, deleteDeck, deleteTag, getDeck, organizeCard, shortlistCard } from "../src/deck/service.ts";
import { add, cut, newDeck, testDb } from "./helpers.ts";

test("tag sets survive parking, promotion and forks; cuts and deletions clean up links", () => {
  const db = testDb();
  try {
    const deck = newDeck(db);
    const first = createTag(db, OWNER, deck, "Engine");
    const second = createTag(db, OWNER, deck, "Need to get");
    add(db, deck, "id-solring");
    organizeCard(db, OWNER, deck, "id-solring", { tagIds: [first, second] });
    cut(db, deck, "id-solring", { park: true });
    assert.deepEqual(getDeck(db, OWNER, deck).cards[0].tag_ids.sort(), [first, second]);
    add(db, deck, "id-solring");
    assert.deepEqual(getDeck(db, OWNER, deck).cards[0].tag_ids.sort(), [first, second]);

    shortlistCard(db, OWNER, deck, "id-counterspell");
    organizeCard(db, OWNER, deck, "id-counterspell", { tagIds: [second] });
    const fork = forkDeck(db, OWNER, deck, "Tag lifecycle fork");
    const copy = getDeck(db, OWNER, fork);
    const ids = new Map(copy.tags.map((t) => [t.name, t.id]));
    assert.ok(copy.tags.every((t) => t.id !== first && t.id !== second));
    assert.deepEqual(copy.cards.find((c) => c.oracle_id === "id-solring")!.tag_ids.sort(), [ids.get("Engine")!, ids.get("Need to get")!].sort());
    assert.deepEqual(copy.cards.find((c) => c.oracle_id === "id-counterspell")!.tag_ids, [ids.get("Need to get")]);

    const entry = cut(db, deck, "id-solring")!;
    assert.equal(db.prepare("SELECT 1 FROM deck_card_tags WHERE deck_id = ? AND oracle_id = ?").get(deck, "id-solring"), undefined);
    deleteTag(db, OWNER, deck, first);
    undo(db, OWNER, entry);
    assert.deepEqual(getDeck(db, OWNER, deck).cards.find((c) => c.oracle_id === "id-solring")!.tag_ids, [second], "undo does not resurrect a deleted tag");
    deleteDeck(db, OWNER, deck);
    assert.equal(db.prepare("SELECT 1 FROM deck_card_tags WHERE deck_id = ?").get(deck), undefined);
    assert.equal(db.prepare("SELECT 1 FROM tags WHERE deck_id = ?").get(deck), undefined);
    assert.equal(getDeck(db, OWNER, fork).cards.length, 2, "the fork's vocabulary and assignments remain independent");
    assert.deepEqual(db.prepare("PRAGMA foreign_key_check").all(), []);
  } finally {
    db.close();
  }
});
