import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Calming Licid and the other Licids. CR 116.2c (a special action that ends a
// continuous effect), 611.2a (no stated duration), 303.4 (Auras).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function parse(text: string, typeLine: string) {
  return parseOracleText(text, undefined, { typeLine });
}
function only(text: string, typeLine: string): Ability {
  const parsed = parse(text, typeLine);
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  if (!parsed.ok) throw new Error(text);
  assert.equal(parsed.abilities.length, 1);
  return JSON.parse(JSON.stringify(parsed.abilities[0]));
}
function rejects(text: string, typeLine: string) {
  assert.equal(parse(text, typeLine).ok, false, text);
}

const licid = { ref: "this", noun: "creature" };
const becomesAura = { effect: "modify-characteristics", what: licid, snapshot: "effect-created", modifiers: [
  { modifier: "types", change: { cardTypes: { operation: "set", types: ["enchantment"] }, subtypes: [{ family: "enchantment", operation: "set", types: ["Aura"] }] } },
  { modifier: "abilities", abilities: [{ gained: "keyword", keyword: { keyword: "enchant", filter: { types: ["creature"] } } }] },
] };

describe("Licid abilities", () => {
  it("removes only the resolving ability, attaches, and offers a paid ending", () => {
    assert.deepEqual(only("{1}{U}{U}, {T}: This creature loses this ability and becomes an Aura enchantment with enchant creature. Attach it to target creature. You may pay {U} to end this effect.", "Creature — Licid"), {
      kind: "activated", costs: [{ cost: "mana", symbols: ["1", "U", "U"] }, { cost: "tap-self" }],
      effects: [
        { steps: [{ effect: "lose-abilities", what: licid, removal: { removal: "this-ability" } }, becomesAura] },
        { steps: [{ effect: "attach", what: { ref: "it" }, to: { ref: "target", filter: { types: ["creature"] }, count: { amount: "fixed", value: 1 } }, who: { player: "you" } }] },
        { steps: [{ effect: "end-effect-action", who: { player: "you" }, costs: [{ cost: "mana", symbols: ["U"] }], ends: "open-ended-effects-of-this-resolution" }] },
      ],
    });
  });

  it("needs an open-ended effect for 'this effect' to end", () => {
    rejects("{W}, {T}: Attach this creature to target creature. You may pay {W} to end this effect.", "Creature — Licid");
    rejects("{W}, {T}: This creature gets +1/+1 until end of turn. You may pay {W} to end this effect.", "Creature — Licid");
    rejects("{W}, {T}: This creature loses flying until end of turn. You may pay {W} to end this effect.", "Creature — Licid");
  });

  it("keeps an ordinary optional payment a payment", () => {
    const ability = only("{T}: This creature loses flying. You may pay {W}.", "Creature — Licid");
    if (ability.kind !== "activated") throw new Error("kind");
    assert.deepEqual(ability.effects[1], { steps: [{ effect: "pay-cost", who: { player: "you" }, costs: [{ cost: "mana", symbols: ["W"] }], optional: true }] });
  });

  it("lets only the ability's own source lose 'this ability'", () => {
    rejects("{T}: Target creature loses this ability.", "Creature — Licid");
    rejects("Creatures you control lose this ability.", "Enchantment");
    rejects("{T}: This creature loses that ability.", "Creature — Licid");
  });
});
