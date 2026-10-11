import { describe, it } from "node:test";
import assert from "node:assert/strict";
import type { Ability, Effect, TokenSpec } from "../src/ast.ts";
import { parseOracleText } from "../src/index.ts";

// Named text checked against cached Scryfall oracle-cards.json. Entry state:
// CR 508.4; copiable token characteristics and copy exceptions: CR 111.3,
// 707.9a–d. https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
const n = (value: number) => ({ amount: "fixed" as const, value });
const target = { ref: "target", filter: { types: ["creature"] }, count: n(1) } as const;
const flying = { gained: "keyword", keyword: { keyword: "flying" } } as const;

function ability(text: string): Ability {
  const result = parseOracleText(text);
  assert.equal(result.ok, true, JSON.stringify(result));
  const line = result.lines[0];
  assert.ok(line.ok);
  return line.ability;
}

function effects(text: string): Effect[] {
  const parsed = ability(text);
  assert.equal(parsed.kind, "spell");
  assert.ok(parsed.kind === "spell");
  return parsed.effects.flatMap(sentence => sentence.steps);
}

function token(text: string): TokenSpec {
  const result = effects(text);
  assert.equal(result.length, 1);
  assert.equal(result[0].effect, "create-token");
  assert.ok(result[0].effect === "create-token");
  return result[0].token;
}

describe("token copy exception scope", () => {
  it("retains coordinated P/T and added abilities inside copiable values", () => {
    assert.deepEqual(token("Create a token that's a copy of target creature, except it's 2/2 and it has flying."), {
      copyOf: target, modifications: [
        { modification: "characteristics", spec: { power: n(2), toughness: n(2) } },
        { modification: "abilities", abilities: [flying] },
      ],
    });
    assert.deepEqual(token("Create a token that's a copy of target creature, except it has flying and it's 2/2."), {
      copyOf: target, modifications: [
        { modification: "abilities", abilities: [flying] },
        { modification: "characteristics", spec: { power: n(2), toughness: n(2) } },
      ],
    });
  });

  it("keeps Brenard's quoted Food ability as a copy modification", () => {
    const result = effects('Create a token that\'s a copy of that creature, except it\'s a 1/1 Food Golem artifact creature in addition to its other types and it has "{2}, {T}, Sacrifice this token: You gain 3 life."');
    assert.equal(result.length, 1);
    const create = result[0];
    assert.ok(create.effect === "create-token" && "copyOf" in create.token);
    assert.deepEqual(create.token, {
      copyOf: { ref: "that", filter: { types: ["creature"] } }, modifications: [
        { modification: "characteristics", spec: { types: ["artifact", "creature"], subtypes: ["Food", "Golem"], power: n(1), toughness: n(1), inAddition: "types" } },
        { modification: "abilities", abilities: [{ gained: "quoted", ability: {
          kind: "activated", abilityWord: undefined, restriction: undefined,
          costs: [{ cost: "mana", symbols: ["2"] }, { cost: "tap-self" }, { cost: "sacrifice", what: { ref: "this", noun: "token" } }],
          effects: [{ steps: [{ effect: "gain-life", who: { player: "you" }, amount: n(3) }] }],
        } }] },
      ],
    });
  });

  it("supports multiple exception categories without changing an unrelated later action", () => {
    assert.deepEqual(effects("Create a token that's a copy of target creature, except it's not legendary, it's a 4/4 Hero in addition to its other types, and it has flying. Draw a card."), [
      { effect: "create-token", count: n(1), token: { copyOf: target, modifications: [
        { modification: "not-legendary" },
        { modification: "characteristics", spec: { power: n(4), toughness: n(4), subtypes: ["Hero"], inAddition: "types" } },
        { modification: "abilities", abilities: [flying] },
      ] } },
      { effect: "draw", who: { player: "you" }, amount: n(1) },
    ]);
  });

  it("keeps later ordinary gains distinct from the copy definition", () => {
    assert.deepEqual(effects("Create a token that's a copy of target creature. It gains haste until end of turn."), [
      { effect: "create-token", count: n(1), token: { copyOf: target } },
      { effect: "gain-abilities", what: { ref: "it" }, abilities: [{ gained: "keyword", keyword: { keyword: "haste" } }], duration: { duration: "end-of-turn" } },
    ]);
  });

  it("rejects Seven Dwarves as a seven-object selector and preserves legitimate untargeted selection", () => {
    assert.equal(parseOracleText("Create a token that's a copy of Seven Dwarves.").ok, false);
    assert.deepEqual(token("Create a token that's a copy of a creature you control."), {
      copyOf: { ref: "filter", filter: { types: ["creature"], control: "you" }, count: n(1) },
    });
    for (const text of [
      "Create a token that's a copy of three creatures.",
      "Create a token that's a copy of each creature you control.",
      "Create a token that's a copy of target creature, except it's an enchantment and loses all other card types.",
      "Create a token that's a copy of target creature, except it's 2/2 and it has an unknown ability.",
      "Create a token that's a copy of target creature, except it's 2/2 and.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });
});

describe("token entry state stays outside copied characteristics", () => {
  it("records tapped prefixes for predefined tokens (Powerstone)", () => {
    assert.deepEqual(token("Create a tapped Powerstone token."), { predefined: "Powerstone", tapped: true });
    assert.deepEqual(token("Create a Food token that's tapped."), { predefined: "Food", tapped: true });
  });

  it("records tapped/attacking prefixes and plural suffixes for specified tokens", () => {
    const expected: TokenSpec = { colors: ["white"], supertypes: [], types: ["creature"], subtypes: ["Soldier"], keywords: [], power: n(1), toughness: n(1), tapped: true, attacking: true };
    assert.deepEqual(token("Create a tapped and attacking 1/1 white Soldier creature token."), expected);
    assert.deepEqual(token("Create two 1/1 white Soldier creature tokens that are tapped and attacking."), expected);
    const { tapped: _, ...attackingOnly } = expected;
    assert.deepEqual(token("Create a 1/1 white Soldier creature token that's attacking."), attackingOnly);
  });

  it("records entry state on copied tokens without making it a copy exception", () => {
    assert.deepEqual(token("Create a tapped and attacking token that's a copy of target creature, except it's not legendary."), {
      copyOf: target, modifications: [{ modification: "not-legendary" }], tapped: true, attacking: true,
    });
    assert.deepEqual(token("Create a token that's a copy of target tapped creature."), {
      copyOf: { ...target, filter: { status: ["tapped"], types: ["creature"] } },
    });
    assert.deepEqual(token("Create a token that's a copy of target creature and that's tapped and attacking."), {
      copyOf: target, tapped: true, attacking: true,
    });
  });

  it("rejects dangling or contradictory state and unsupported specified defenders", () => {
    for (const text of [
      "Create a 1/1 white Soldier creature token that's tapped and.",
      "Create a tapped and 1/1 white Soldier creature token.",
      "Create a tapped and tapped Food token.",
      "Create a colorless and red Goblin creature token.",
      "Create a red and creature and token.",
      "Create a 1/1 white Soldier creature token that's tapped and attacking that player.",
      "Create a tapped token that's a copy of target creature that's tapped.",
      "Create a token that's a copy of target creature that's a Food.",
      "Create a token that's a copy of target creature and that's mysterious.",
    ]) assert.equal(parseOracleText(text).ok, false, text);
  });
});
