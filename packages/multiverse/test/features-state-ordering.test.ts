import { test } from "node:test";
import assert from "node:assert/strict";
import { acceptsDeckState } from "../web/src/features/deck/stateOrdering.ts";

const state = (id: number, revision: number, edit_version: number) => ({ deck: { id, revision, edit_version } });

test("only the requested deck may enter an empty or populated store", () => {
  assert.equal(acceptsDeckState(7, null, state(7, 0, 0)), true);
  assert.equal(acceptsDeckState(7, null, state(8, 0, 0)), false);
  assert.equal(acceptsDeckState(7, state(7, 3, 5), state(8, 90, 90)), false);
});

test("delayed membership or organization responses cannot roll back either version", () => {
  const current = state(7, 4, 8);
  assert.equal(acceptsDeckState(7, current, state(7, 3, 7)), false);
  assert.equal(acceptsDeckState(7, current, state(7, 3, 9)), false);
  assert.equal(acceptsDeckState(7, current, state(7, 5, 7)), false);
  assert.equal(acceptsDeckState(7, current, state(7, 5, 9)), true);
  assert.equal(acceptsDeckState(7, current, state(7, 4, 9)), true);
});

test("equal-version refreshes remain eligible for proposal, price and memory updates", () => {
  const current = state(7, 4, 8);
  const incoming = { ...state(7, 4, 8), proposals: ["new server metadata"] };
  assert.equal(acceptsDeckState(7, current, incoming), true);
});
