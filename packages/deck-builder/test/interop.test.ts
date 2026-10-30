import { test, describe } from "node:test";
import assert from "node:assert/strict";
import { OWNER } from "./actors.ts";
import { createDeck, getDeck } from "../src/deck/service.ts";
import {
  applyImport,
  parseDeckList,
} from "../src/deck/interop.ts";
import { testDb } from "./helpers.ts";

const db = testDb();


describe("Archidekt list parsing (verified format superset)", () => {
  test("accepts the full round-trip template: 1x Name (code) *F* [Category] ^Label,#hex^", () => {
    const { entries } = parseDeckList("1x Sol Ring (cmm) 405 *F* [Ramp,Artifact] ^Buy,#000000^");
    assert.equal(entries.length, 1);
    assert.equal(entries[0].quantity, 1);
    assert.equal(entries[0].name, "Sol Ring");
    assert.deepEqual(entries[0].categories, ["Ramp", "Artifact"]);
  });
});


describe("import applies in full and at once (spec §12)", () => {

  test("Archidekt's sectioned export sets the commander and keeps the maybeboard out of the deck", () => {
    const id = createDeck(db, OWNER, "Import sections");
    const text = [
      "Commander",
      "1 Teferi, Temporal Archmage",
      "",
      "Mainboard",
      "1 Sol Ring",
      "5 Forest",
      "",
      "Maybeboard",
      "1 Ball Lightning",
      "1 Tarmogoyf",
    ].join("\n");
    applyImport(db, OWNER, id, text);
    const view = getDeck(db, OWNER, id);
    assert.equal(view.cards.find((c) => c.name === "Teferi, Temporal Archmage")!.role, "commander");
    assert.equal(view.deck.color_identity, "U");
    assert.equal(view.computed.card_count, 7);
    assert.deepEqual(
      view.cards.filter((c) => c.maybe).map((c) => c.name).sort(),
      ["Ball Lightning", "Tarmogoyf"],
    );
  });
});
