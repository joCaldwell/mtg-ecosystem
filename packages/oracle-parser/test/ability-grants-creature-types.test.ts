import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Amoeboid Changeling, Nameless Inversion, Wings of Velis Vel, Curse of
// Conformity, Oko Lorwyn Liege. CR 205.1b, 205.3m: a type change worded with
// "gains" / "loses"; it is not an ability grant or removal.
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

const fixed = (value: number) => ({ amount: "fixed" as const, value });
const target = { ref: "target", filter: { types: ["creature"] }, count: fixed(1) };
const allTypes = (operation: "add" | "remove") => ({ modifier: "types", change: { subtypes: [{ family: "creature", operation, types: { all: true } }] } });
const untilTurn = { duration: "end-of-turn" };
const change = (what: object, operation: "add" | "remove", extra: object = { duration: untilTurn }) =>
  ({ effect: "modify-characteristics", what, modifiers: [allTypes(operation)], snapshot: "effect-created", ...extra });

describe("gaining and losing all creature types", () => {
  it("adds or removes every creature type as a type change", () => {
    assert.deepEqual(only("{T}: Target creature gains all creature types until end of turn.", "Creature — Shapeshifter"),
      { kind: "activated", costs: [{ cost: "tap-self" }], effects: [{ steps: [change(target, "add")] }] });
    assert.deepEqual(only("{T}: Target creature loses all creature types until end of turn.", "Creature — Shapeshifter"),
      { kind: "activated", costs: [{ cost: "tap-self" }], effects: [{ steps: [change(target, "remove")] }] });
  });

  it("shares the duration of a coordinated pump", () => {
    assert.deepEqual(only("Target creature gets +3/-3 and loses all creature types until end of turn.", "Kindred Instant — Shapeshifter"), {
      kind: "spell", effects: [{ steps: [
        { effect: "pump", what: target, power: { sign: 1, amount: fixed(3) }, toughness: { sign: -1, amount: fixed(3) }, duration: untilTurn },
        change(target, "remove"),
      ] }] });
  });

  it("keeps base P/T, the type change and a keyword grant as separate parts", () => {
    assert.deepEqual(only("Until end of turn, target creature has base power and toughness 4/4, gains all creature types, and gains flying.", "Kindred Instant — Shapeshifter"), {
      kind: "spell", effects: [{ steps: [
        { effect: "set-pt", what: target, power: fixed(4), toughness: fixed(4), duration: untilTurn },
        change(target, "add"),
        { effect: "gain-abilities", what: target, abilities: [{ gained: "keyword", keyword: { keyword: "flying" } }], duration: untilTurn },
      ] }] });
  });

  it("has no duration when none is printed", () => {
    assert.deepEqual(only("+2: Up to one target creature gains all creature types.", "Legendary Planeswalker — Oko"),
      { kind: "loyalty", cost: { sign: 1, amount: 2 }, effects: [{ steps: [change({ ...target, upTo: true }, "add", {})] }] });
  });

  it("is a continuous type modifier in a static ability", () => {
    assert.deepEqual(only("Nonlegendary creatures enchanted player controls have base power and toughness 3/3 and lose all creature types.", "Enchantment — Aura Curse"), {
      kind: "static", effect: { static: "modify",
        what: { ref: "each", filter: { nonSupertypes: ["legendary"], types: ["creature"], controlledBy: { player: "enchanted-player" } } },
        modifiers: [{ modifier: "base-pt", power: fixed(3), toughness: fixed(3) }, allTypes("remove")] } });
  });

  it("rejects wordings that say more than this type change", () => {
    rejects("Target creature loses all creature types and abilities until end of turn.", "Instant");
    rejects("Target creature gains all creature types in addition to its other types until end of turn.", "Instant");
    rejects("Target creature gains all card types until end of turn.", "Instant");
    rejects("Target creature gets all creature types until end of turn.", "Instant");
  });
});
