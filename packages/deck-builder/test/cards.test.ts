import { test, describe } from "node:test";
import assert from "node:assert/strict";
import {
  CardResolutionError,
  lintProse,
  resolveCard,
  resolveCards,
  suggestNames,
} from "../src/cards.ts";
import { testDb } from "./helpers.ts";

const db = testDb();

describe("exact resolution (spec §4.3)", () => {
  test("a name resolves exactly, case-insensitively, and never fuzzily", () => {
    assert.equal(resolveCard(db, "Sol Ring").oracle_id, "id-solring");
    assert.equal(resolveCard(db, "  sol ring ").oracle_id, "id-solring");
    assert.throws(() => resolveCard(db, "Sol Rin"), CardResolutionError);
    assert.throws(() => resolveCard(db, "Seedborn Sage"), CardResolutionError);
  });
  test("an oracle_id is accepted wherever a name is", () => {
    assert.equal(resolveCard(db, "id-atraxa").name, "Atraxa, Praetors' Voice");
  });
  test("a face name resolves to its card", () => {
    assert.equal(resolveCard(db, "Witch Enchanter").oracle_id, "id-witch");
    assert.equal(resolveCard(db, "Witch-Blessed Meadow").oracle_id, "id-witch");
  });
  // "Rampant Growth" is both a card and the back face of another card. The
  // card actually called that wins; the other is reachable by its own name.
  test("a full-name match wins over a face-name match", () => {
    assert.equal(resolveCard(db, "Rampant Growth").oracle_id, "id-rampant");
    assert.equal(resolveCard(db, "Studious First-Year").oracle_id, "id-firstyear");
    assert.equal(
      resolveCard(db, "Studious First-Year // Rampant Growth").oracle_id,
      "id-firstyear",
    );
  });
  test("one bad name fails the whole batch, naming every failure with suggestions", () => {
    try {
      resolveCards(db, ["Sol Ring", "Sol Ringz", "Counterspel"]);
      assert.fail("should have thrown");
    } catch (e) {
      assert.ok(e instanceof CardResolutionError);
      assert.equal(e.failures.length, 2);
      assert.match(e.message, /nothing was changed/);
      assert.match(e.message, /"Sol Ringz" is not a card name\. Did you mean: Sol Ring/);
      assert.match(e.message, /Counterspell/);
    }
  });
});

describe("suggestions", () => {
  // The old ranking offered Soldier cards for "Sol Ringz"; closeness to what
  // was typed has to come first.
  test("the closest spelling is offered first", () => {
    assert.equal(suggestNames(db, "Sol Ringz")[0], "Sol Ring");
    assert.equal(suggestNames(db, "Llanowar Elfs")[0], "Llanowar Elves");
    assert.equal(suggestNames(db, "Atraxa Praetors Voice")[0], "Atraxa, Praetors' Voice");
    assert.equal(suggestNames(db, "Witch Enchantr")[0], "Witch Enchanter // Witch-Blessed Meadow");
  });
  test("nothing remotely similar yields no suggestions", () => {
    assert.deepEqual(suggestNames(db, "Qqqzzzxx"), []);
  });
});

describe("prose linting (spec §10.5)", () => {
  test("every [[Card Name]] must be a real card", () => {
    const cards = lintProse(db, "Swap [[Llanowar Elves]] for [[Sol Ring]]; keep [[sol ring]] too.");
    assert.deepEqual(cards.map((c) => c.name).sort(), ["Llanowar Elves", "Sol Ring"]);
    assert.throws(() => lintProse(db, "Add [[Signet of the Adeptus]]"), /is not a card name/);
  });
  test("prose with no references passes, and a shared name is not a failure", () => {
    assert.deepEqual(lintProse(db, "Too slow for this list."), []);
    assert.equal(lintProse(db, "[[Rampant Growth]] is the ramp spell here.")[0].oracle_id, "id-rampant");
  });
});
