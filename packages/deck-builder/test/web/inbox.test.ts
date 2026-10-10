import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { pendingItems, waitingCount } from "../../web/src/domain/inbox.ts";
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

  test("an empty inbox waits on nothing, and unreviewed rulings are not decisions", () => {
    assert.equal(waitingCount({ proposals: [], document_edits: [], unreviewed: 4 }), 0);
  });

  test("a document edit that is no longer pending does not wait", () => {
    assert.equal(
      waitingCount({ proposals: [], document_edits: [documentEdit({ status: "accepted" })], unreviewed: 0 }),
      0,
    );
  });
});
