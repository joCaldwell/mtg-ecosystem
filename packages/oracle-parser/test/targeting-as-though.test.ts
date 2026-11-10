import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, ParsedLine } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Card wordings from the cached Scryfall Oracle corpus. CR 609.4, 702.11b–e
// (hexproof), 702.18a (shroud).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function line(text: string, typeLine = "Artifact"): ParsedLine {
  const result = parseOracleText(text, "Autumn Willow", { typeLine });
  assert.equal(result.lines.length, 1, text);
  return result.lines[0];
}
function ability(text: string, typeLine?: string): Ability {
  const parsed = line(text, typeLine);
  assert.ok(parsed.ok, `${text}: ${parsed.ok ? "" : parsed.error}`);
  return JSON.parse(JSON.stringify(parsed.ability)) as Ability;
}
function fails(text: string, typeLine?: string): void {
  assert.equal(line(text, typeLine).ok, false, text);
}
const you = { player: "you" } as const;

describe("targeting as though without hexproof or shroud (CR 609.4, 702.11, 702.18)", () => {
  it("keeps every printed subject and whose spells and abilities benefit", () => {
    // Glaring Spotlight; Kaya, Bane of the Dead.
    assert.deepEqual(ability("Creatures your opponents control with hexproof can be the targets of spells and abilities you control as though they didn't have hexproof."), {
      kind: "static", effect: { static: "targeting-as-though", lacking: "hexproof", by: you,
        what: [{ ref: "each", filter: { types: ["creature"], control: "opponent", withAbility: ["hexproof"] } }] },
    });
    assert.deepEqual(ability("Your opponents and permanents your opponents control with hexproof can be the targets of spells and abilities you control as though they didn't have hexproof.", "Legendary Planeswalker — Kaya"), {
      kind: "static", effect: { static: "targeting-as-though", lacking: "hexproof", by: you,
        what: [{ player: "each-opponent" }, { ref: "each", filter: { cls: "permanent", control: "opponent", withAbility: ["hexproof"] } }] },
    });
  });

  it("is a resolving effect with its duration, and may name a targeted controller", () => {
    // Detection Tower; Autumn Willow.
    assert.deepEqual(ability("{1}, {T}: Until end of turn, your opponents and creatures your opponents control with hexproof can be the targets of spells and abilities you control as though they didn't have hexproof.", "Land"), {
      kind: "activated", costs: [{ cost: "mana", symbols: ["1"] }, { cost: "tap-self" }],
      effects: [{ steps: [{ effect: "targeting-as-though", lacking: "hexproof", by: you, duration: { duration: "end-of-turn" },
        what: [{ player: "each-opponent" }, { ref: "each", filter: { types: ["creature"], control: "opponent", withAbility: ["hexproof"] } }] }] }],
    });
    assert.deepEqual(ability("{G}: Until end of turn, Autumn Willow can be the target of spells and abilities controlled by target player as though it didn't have shroud.", "Legendary Creature — Avatar"), {
      kind: "activated", costs: [{ cost: "mana", symbols: ["G"] }],
      effects: [{ steps: [{ effect: "targeting-as-though", what: [{ ref: "self" }], lacking: "shroud", by: { player: "target-player" },
        duration: { duration: "end-of-turn" } }] }],
    });
  });

  it("rejects number disagreement, other abilities and unrepresented riders", () => {
    fails("Creatures your opponents control can be the target of spells and abilities you control as though they didn't have hexproof.");
    fails("Creatures your opponents control can be the targets of spells and abilities you control as though it didn't have hexproof.");
    fails("Creatures your opponents control can be the targets of spells and abilities you control as though they didn't have flying.");
    fails("Creatures your opponents control can be the targets of spells you control as though they didn't have hexproof.");
    // Nowhere to Run: the ward sentence is not represented.
    fails("Creatures your opponents control can be the targets of spells and abilities as though they didn't have hexproof. Ward abilities of those creatures don't trigger.", "Enchantment");
  });
});
