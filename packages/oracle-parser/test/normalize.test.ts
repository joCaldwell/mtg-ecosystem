import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { normalizeOracleText } from "../src/normalize.ts";

describe("normalizeOracleText", () => {
  it("preserves line boundaries", () => {
    assert.deepEqual(normalizeOracleText("Flying\nWhen ~ enters, draw a card."), [
      "Flying",
      "When ~ enters, draw a card.",
    ]);
  });

  it("strips reminder text, dropping lines that become empty", () => {
    assert.deepEqual(
      normalizeOracleText("Flying (This creature can't be blocked except by creatures with flying or reach.)"),
      ["Flying"],
    );
    assert.deepEqual(normalizeOracleText("(Reminder only.)\nHaste"), ["Haste"]);
  });

  it("replaces full and short self-references exactly", () => {
    assert.deepEqual(
      normalizeOracleText(
        "Whenever Halsin, Emerald Archdruid enters, Halsin gets +1/+1.",
        "Halsin, Emerald Archdruid",
      ),
      ["Whenever ~ enters, ~ gets +1/+1."],
    );
  });

  it("keeps possessive self-references as ~'s", () => {
    assert.deepEqual(
      normalizeOracleText("Urza's power is equal to the number of artifacts you control.", "Urza, Lord High Artificer"),
      ["~'s power is equal to the number of artifacts you control."],
    );
  });

  it("does NOT rewrite unrelated words matching a card name case-insensitively", () => {
    // A card named "Fear" must not rewrite lowercase "fear" elsewhere.
    assert.deepEqual(
      normalizeOracleText("Enchanted creature has fear.", "Fear"),
      ["Enchanted creature has fear."],
    );
  });

  it("does not rewrite name fragments inside longer words", () => {
    assert.deepEqual(normalizeOracleText("Destroy target Aurochs.", "Auro"), ["Destroy target Aurochs."]);
  });

  it("canonicalizes typographic apostrophes and minus signs", () => {
    assert.deepEqual(normalizeOracleText("−2: Target creature can’t block."), [
      "-2: Target creature can't block.",
    ]);
  });
});
