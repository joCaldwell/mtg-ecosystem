import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, ParsedLine } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Card wordings from the cached Scryfall Oracle corpus. CR 609.4b: spending
// mana "as though" changes how a cost may be paid, never the cost or the
// mana actually spent.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
function line(text: string, typeLine = "Artifact"): ParsedLine {
  const result = parseOracleText(text, "Test Card", { typeLine });
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

describe("spending mana as though it were other mana (CR 609.4b)", () => {
  it("applies to every cost when no purpose is printed", () => {
    // Chromatic Orrery; Mycosynth Lattice; Sunglasses of Urza.
    assert.deepEqual(ability("You may spend mana as though it were mana of any color."),
      { kind: "static", effect: { static: "mana-spending", who: you, as: "any-color" } });
    assert.deepEqual(ability("Players may spend mana as though it were mana of any color."),
      { kind: "static", effect: { static: "mana-spending", who: { player: "each-player" }, as: "any-color" } });
    assert.deepEqual(ability("You may spend white mana as though it were red mana."),
      { kind: "static", effect: { static: "mana-spending", who: you, mana: { color: "white" }, as: { color: "red" } } });
  });

  it("keeps the printed purposes", () => {
    // Oath of Nissa; Agatha's Soul Cauldron; Quicksilver Elemental.
    assert.deepEqual(ability("You may spend mana as though it were mana of any color to cast planeswalker spells.", "Legendary Enchantment"), {
      kind: "static", effect: { static: "mana-spending", who: you, as: "any-color",
        uses: [{ use: "cast", filter: { types: ["planeswalker"], cls: "spell" } }] },
    });
    assert.deepEqual(ability("You may spend mana as though it were mana of any color to activate abilities of creatures you control."), {
      kind: "static", effect: { static: "mana-spending", who: you, as: "any-color",
        uses: [{ use: "activate", source: { types: ["creature"], control: "you" } }] },
    });
    assert.deepEqual(ability("You may spend blue mana as though it were mana of any color to pay the activation costs of this creature's abilities.", "Creature — Elemental"), {
      kind: "static", effect: { static: "mana-spending", who: you, mana: { color: "blue" }, as: "any-color",
        uses: [{ use: "activate-abilities-of", source: { ref: "this", noun: "creature" } }] },
    });
    assert.deepEqual(ability("You may spend mana as though it were mana of any type to cast artifact spells or to activate abilities."), {
      kind: "static", effect: { static: "mana-spending", who: you, as: "any-type",
        uses: [{ use: "cast", filter: { types: ["artifact"], cls: "spell" } }, { use: "activate" }] },
    });
  });

  it("rejects an added restriction, a same-color pretense and trailing text", () => {
    // Celestial Dawn: the second sentence restricts all other mana.
    fails("You may spend white mana as though it were mana of any color. You may spend other mana only as though it were colorless mana.", "Enchantment");
    fails("You may spend white mana as though it were white mana.");
    fails("You may spend mana as though it were mana of any color to cast.");
    fails("You may spend mana only as though it were mana of any color.");
    fails("You may spend mana as though it were mana of any color to pay the activation costs of creatures' abilities.");
  });
});
