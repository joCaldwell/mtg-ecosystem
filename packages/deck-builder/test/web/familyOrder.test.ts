import { test } from "node:test";
import assert from "node:assert/strict";
import type { DeckSummary } from "../../web/src/api/client.ts";
import { familyOrder } from "../../web/src/domain/familyOrder.ts";

test("family order preserves grandchildren, missing parents and corrupt cycles without duplicates", () => {
  const decks = [{ id: 3, parent_id: 2 }, { id: 1, parent_id: null }, { id: 2, parent_id: 1 }, { id: 4, parent_id: 99 }, { id: 5, parent_id: 6 }, { id: 6, parent_id: 5 }] as DeckSummary[];
  const rows = familyOrder(decks);
  assert.deepEqual(rows.map(r => r.deck.id), [1, 2, 3, 4, 5, 6]);
  assert.deepEqual(rows.map(r => r.fork), [false, true, true, false, false, true]);
});
