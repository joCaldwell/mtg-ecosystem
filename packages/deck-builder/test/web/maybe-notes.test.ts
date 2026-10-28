import { test } from "node:test";
import assert from "node:assert/strict";
import { maybeNotes } from "../../web/src/domain/maybeNotes.ts";

test("mixed import provenance counts only exact app notes without consuming custom notes or changing input order", () => {
  const cards = [
    { maybe_note: "From the pasted list's Sideboard" },
    { maybe_note: null },
    { maybe_note: "From the pasted list's Sideboard" },
    { maybe_note: "From the pasted list's Maybeboard" },
    { maybe_note: "From the pasted list's Sideboard; test this first" },
  ];
  const snapshot = structuredClone(cards);
  assert.deepEqual(maybeNotes(cards), { origins: [
    { note: "From the pasted list's Sideboard", label: "Sideboard", count: 2 },
    { note: "From the pasted list's Maybeboard", label: "Maybeboard", count: 1 },
  ], shared: null });
  assert.deepEqual(cards, snapshot);
  assert.deepEqual(maybeNotes([]), { origins: [], shared: null });
  assert.equal(maybeNotes([{ maybe_note: "Keep for later" }, { maybe_note: "Keep for later" }]).shared, "Keep for later");
  assert.equal(maybeNotes([{ maybe_note: "Keep for later" }, { maybe_note: null }]).shared, null);
});
