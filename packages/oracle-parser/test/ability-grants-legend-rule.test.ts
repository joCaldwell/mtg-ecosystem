import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Wordings from the cached Scryfall corpus (Mirror Gallery, Mirror Box, The
// Master Multiplied, Brothers Yamazaki, Hall of Echoes). CR 704.5j.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function parse(text: string, typeLine: string, name?: string) {
  return parseOracleText(text, name, { typeLine });
}
function only(text: string, typeLine: string, name?: string): Ability {
  const parsed = parse(text, typeLine, name);
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  if (!parsed.ok) throw new Error(text);
  assert.equal(parsed.abilities.length, 1);
  return JSON.parse(JSON.stringify(parsed.abilities[0]));
}
function rejects(text: string, typeLine: string) {
  assert.equal(parse(text, typeLine).ok, false, text);
}

const fixed = (value: number) => ({ amount: "fixed" as const, value });

describe("legend rule exemptions", () => {
  it("switches the rule off entirely when no permanents are named", () => {
    assert.deepEqual(only('The "legend rule" doesn\'t apply.', "Artifact"), { kind: "static", effect: { static: "legend-rule-exemption" } });
  });

  it("exempts exactly the described permanents", () => {
    assert.deepEqual(only('The "legend rule" doesn\'t apply to permanents you control.', "Artifact"),
      { kind: "static", effect: { static: "legend-rule-exemption", what: { ref: "each", filter: { cls: "permanent", control: "you" } } } });
    assert.deepEqual(only('The "legend rule" doesn\'t apply to creature tokens you control.', "Legendary Creature — Time Lord Rogue"),
      { kind: "static", effect: { static: "legend-rule-exemption", what: { ref: "each", filter: { types: ["creature"], cls: "token", control: "you" } } } });
  });

  it("keeps a leading condition and its pronoun", () => {
    assert.deepEqual(only('If there are exactly two permanents named "Brothers Yamazaki" on the battlefield, the "legend rule" doesn\'t apply to them.', "Legendary Creature — Human Samurai"), {
      kind: "static", effect: { static: "conditional",
        condition: { condition: "count", of: { cls: "permanent", name: { name: "literal", value: "Brothers Yamazaki" }, zone: { zone: "battlefield" } },
          comparison: { op: "eq", value: fixed(2) } },
        effect: { static: "legend-rule-exemption", what: { ref: "them" } } } });
  });

  it("requires a duration for the resolving form", () => {
    assert.deepEqual(only('{5}: This land becomes a copy of target creature you control until end of turn. The "legend rule" doesn\'t apply to permanents you control this turn.', "Land"), {
      kind: "activated", costs: [{ cost: "mana", symbols: ["5"] }],
      effects: [
        { steps: [{ effect: "become", what: { ref: "this", noun: "land" },
          spec: { copyOf: { ref: "target", filter: { types: ["creature"], control: "you" }, count: fixed(1) } }, duration: { duration: "end-of-turn" } }] },
        { steps: [{ effect: "legend-rule-exemption", what: { ref: "each", filter: { cls: "permanent", control: "you" } }, duration: { duration: "this-turn" } }] },
      ] });
    rejects('Draw a card. The "legend rule" doesn\'t apply to permanents you control.', "Sorcery");
  });

  it("rejects single or targeted permanents and other quoted rules", () => {
    rejects('The "legend rule" doesn\'t apply to target creature.', "Artifact");
    rejects('The "legend rule" doesn\'t apply to this creature.', "Artifact");
    rejects('The "legend rule" doesn\'t apply to a creature you control.', "Artifact");
    rejects('The "planeswalker uniqueness rule" doesn\'t apply.', "Artifact");
    rejects('The "legend rule" applies to permanents you control.', "Artifact");
  });
});
