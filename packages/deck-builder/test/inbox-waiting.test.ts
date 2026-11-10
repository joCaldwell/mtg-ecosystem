import { test } from "node:test";
import assert from "node:assert/strict";
import type { Inbox } from "../web/src/api/client.ts";
import { deckWaitingState, waitingByDeck, waitingCount } from "../web/src/domain/inbox.ts";

test("deck badges count the Inbox's proposals and edit groups, with a flag only before it loads", () => {
  const inbox = {
    proposals: [
      { deck_id: 1, status: "open", disposition: "current", items: [{ status: "pending" }, { status: "pending" }] },
      { deck_id: 1, status: "open", disposition: "current", items: [{ status: "accepted" }] },
    ],
    document_edits: [
      { id: 1, group_id: 10, deck_id: 1, status: "pending" },
      { id: 2, group_id: 10, deck_id: 1, status: "pending" },
      { id: 3, group_id: 11, deck_id: 2, status: "pending" },
    ],
  } as unknown as Inbox;
  const summary = { id: 1, pending_items: 2, pending_document_edits: 2 };
  const counts = waitingByDeck(inbox);
  assert.equal(waitingCount(inbox), 3);
  assert.deepEqual(deckWaitingState(summary, counts), { count: 2, pending: true });
  assert.deepEqual(deckWaitingState(summary, null), { count: null, pending: true });
  assert.deepEqual(deckWaitingState(summary, new Map()), { count: 0, pending: false });
  assert.deepEqual(deckWaitingState({ ...summary, pending_items: 0, pending_document_edits: 0 }, null), { count: null, pending: false });
});
