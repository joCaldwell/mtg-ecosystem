import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { pendingItems, waitingByDeck, waitingCount } from "../../web/src/domain/inbox.ts";
import { documentEdit, item, proposal } from "./fixtures.ts";

describe("inbox", () => {
  test("pendingItems keeps only the unruled", () => {
    const p = proposal([item({ name: "Test Card A" }), item({ name: "Test Card B", status: "accepted" })]);
    assert.deepEqual(pendingItems(p).map((i) => i.card_name), ["Test Card A"]);
  });

  test("waitingCount counts proposals with a pending item, plus document edits", () => {
    const open = proposal([item({ status: "accepted" }), item({ name: "Test Card B" })]);
    const finished = proposal([item({ name: "Test Card C", status: "rejected" })]);
    assert.equal(
      waitingCount({ proposals: [open, finished], document_edits: [documentEdit(), documentEdit()], unreviewed: 5 }),
      3,
    );
  });

  test("an empty inbox waits on nothing, and unreviewed decisions are not decisions", () => {
    assert.equal(waitingCount({ proposals: [], document_edits: [], unreviewed: 4 }), 0);
  });

  test("a document edit that is no longer pending does not wait", () => {
    assert.equal(
      waitingCount({ proposals: [], document_edits: [documentEdit({ status: "accepted" })], unreviewed: 0 }),
      0,
    );
  });
});

test("a related document group is one inbox decision; superseded cards aren't pending work", () => {
  const group = [documentEdit({ id: 101, group_id: 101 }), documentEdit({ id: 102, group_id: 101 })];
  assert.equal(waitingCount({ proposals: [], document_edits: group, unreviewed: 0 }), 1);
  const old = proposal([item()], { status: "resolved", disposition: "superseded", is_current: false });
  assert.deepEqual(pendingItems(old), []);
});

test("waitingByDeck splits the same count by deck and leaves profile edits out", () => {
  const a = proposal([item()], { deck_id: 1 });
  const b = proposal([item()], { deck_id: 1 });
  const done = proposal([item({ status: "accepted" })], { deck_id: 2 });
  const edits = [documentEdit({ id: 7, group_id: 7, deck_id: 2 }), documentEdit({ id: 8, group_id: 7, deck_id: 2 }), documentEdit({ id: 9, deck_id: null })];
  const counts = waitingByDeck({ proposals: [a, b, done], document_edits: edits, unreviewed: 0 });
  assert.deepEqual([...counts].sort(), [[1, 2], [2, 1]]);
});
