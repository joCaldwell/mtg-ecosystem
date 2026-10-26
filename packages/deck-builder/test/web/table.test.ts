import { test } from "node:test";
import assert from "node:assert/strict";
import { faceOf, selectedAfterClick } from "../../web/src/domain/table.ts";
import { parseRoute, href } from "../../web/src/app/routes.ts";
import { FIXTURES } from "../fixtures.ts";
test("table selection toggles independent instance IDs without changing another copy", () => {
  assert.deepEqual(selectedAfterClick(["a"], "b", false), ["b"]);
  assert.deepEqual(selectedAfterClick(["a"], "b", true), ["a", "b"]);
  assert.deepEqual(selectedAfterClick(["a", "b"], "a", true), ["b"]);
});
test("table faces use verified fixture faces and the playtest route survives a reload", () => {
  const c = FIXTURES.find((c) => c.oracle_id === "id-witch")!;
  const card = { oracle_id: c.oracle_id!, name: c.name, faces: c.card_faces! };
  assert.equal(faceOf(card, 1).name, c.card_faces![1].name);
  assert.equal(faceOf(card, 9).name, c.name);
  const route = { page: "playtest" as const, deckId: 12 };
  assert.deepEqual(parseRoute(href(route)), route);
});
