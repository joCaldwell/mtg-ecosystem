import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability, StaticEffect } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

// Wordings from the cached Scryfall corpus (Reliquary Tower, Anvil of
// Bogardan, Jin-Gitaxias Core Augur, Midnight Oil, Cursed Rack, Marina
// Vendrell's Grimoire, Platinum Angel, Platinum Persecutor, The Bird Champion,
// Erebos, Upwelling, Leyline Tyrant, Skullcrack, Angel's Grace, Praetor's
// Counsel, Stigma Lasher, The Last Agni Kai, Everybody Lives!).
// Rules: CR 402.2, 119.7-119.8, 104.2-104.3, 704.5a, 500.5, 613.11, 611.2a.
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
function effect(text: string, typeLine: string): StaticEffect {
  const ability = only(text, typeLine);
  assert.equal(ability.kind, "static");
  if (ability.kind !== "static") throw new Error(text);
  return ability.effect;
}
function rejects(text: string, typeLine: string) {
  assert.equal(parse(text, typeLine).ok, false, text);
}

const fixed = (value: number) => ({ amount: "fixed" as const, value });
const you = { player: "you" };
const players = { player: "each-player" };
const opponents = { player: "each-opponent" };
const rule = (who: object, body: object) => ({ static: "player-rule", rule: { who, ...body } });
const noMaximum = { rule: "maximum-hand-size", change: { operation: "none" } };

describe("maximum hand size rules", () => {
  it("removes the maximum for the named players", () => {
    assert.deepEqual(effect("You have no maximum hand size.", "Land"), rule(you, noMaximum));
    assert.deepEqual(effect("Players have no maximum hand size.", "Artifact"), rule(players, noMaximum));
  });

  it("distinguishes setting the number from adjusting it", () => {
    assert.deepEqual(effect("Your maximum hand size is two.", "Enchantment"),
      rule(you, { rule: "maximum-hand-size", change: { operation: "set", value: fixed(2) } }));
    assert.deepEqual(effect("Each opponent's maximum hand size is reduced by seven.", "Legendary Creature — Phyrexian Praetor"),
      rule(opponents, { rule: "maximum-hand-size", change: { operation: "reduce", amount: fixed(7) } }));
    assert.deepEqual(effect("Your maximum hand size is increased by two.", "Creature — Human Advisor"),
      rule(you, { rule: "maximum-hand-size", change: { operation: "increase", amount: fixed(2) } }));
    assert.deepEqual(effect("Your maximum hand size is equal to the number of hour counters on this enchantment.", "Enchantment"),
      rule(you, { rule: "maximum-hand-size", change: { operation: "set",
        value: { amount: "counter-count", counter: { type: "hour" }, on: { ref: "this", noun: "enchantment" } } } }));
  });

  it("keeps a linked chosen player symbolic", () => {
    assert.deepEqual(effect("The chosen player's maximum hand size is four.", "Artifact"),
      rule({ player: "chosen-player", choice: { choice: "chosen-value", category: "player", which: "chosen" } },
        { rule: "maximum-hand-size", change: { operation: "set", value: fixed(4) } }));
  });

  it("requires a stated lifetime for the resolving form", () => {
    assert.deepEqual(only("Return all cards from your graveyard to your hand. Exile ~. You have no maximum hand size for the rest of the game.", "Sorcery"), {
      kind: "spell", effects: [
        { steps: [{ effect: "return-to-hand", what: { ref: "all", filter: { cls: "card", zone: { zone: "graveyard", owner: "your" } } }, to: "your" }] },
        { steps: [{ effect: "exile", what: { ref: "self" } }] },
        { steps: [{ effect: "player-rule", rule: { who: you, ...noMaximum }, duration: { duration: "rest-of-game" } }] },
      ],
    });
    rejects("Draw three cards. You have no maximum hand size.", "Sorcery");
  });

  it("rejects hand-size wordings it does not represent", () => {
    rejects("Your maximum hand size is reduced.", "Creature — Beast");
    rejects("Your starting hand size is four.", "Artifact");
    rejects("You have no maximum hand size during your turn.", "Artifact");
  });
});

describe("life and game-outcome prohibitions", () => {
  it("names the players who can't gain life", () => {
    assert.deepEqual(effect("Your opponents can't gain life.", "Legendary Enchantment Creature — God"), rule(opponents, { rule: "cant-gain-life" }));
    assert.deepEqual(effect("Players can't gain life.", "Enchantment"), rule(players, { rule: "cant-gain-life" }));
    assert.deepEqual(effect("Enchanted player can't gain life.", "Enchantment — Aura"), rule({ player: "enchanted-player" }, { rule: "cant-gain-life" }));
  });

  it("keeps each coordinated outcome with its own player", () => {
    assert.deepEqual(effect("You can't lose the game and your opponents can't win the game.", "Artifact Creature — Angel"), {
      static: "group", effects: [rule(you, { rule: "cant-lose-game" }), rule(opponents, { rule: "cant-win-game" })] });
    assert.deepEqual(effect("You can't win the game and your opponents can't lose the game.", "Creature — Demon"), {
      static: "group", effects: [rule(you, { rule: "cant-win-game" }), rule(opponents, { rule: "cant-lose-game" })] });
    assert.deepEqual(effect("Players can't lose the game or win the game.", "Artifact Creature — Angel Demon"), {
      static: "group", effects: [rule(players, { rule: "cant-lose-game" }), rule(players, { rule: "cant-win-game" })] });
  });

  it("scopes a leading condition over every coordinated rule", () => {
    assert.deepEqual(effect("As long as you control twelve or more Birds, you can't lose the game and opponents can't win the game.", "Legendary Creature — Bird Wizard"), {
      static: "conditional",
      condition: { condition: "control-count", who: you, of: { subtypes: ["Bird"] }, comparison: { op: "ge", value: fixed(12) } },
      effect: { static: "group", effects: [rule(you, { rule: "cant-lose-game" }), rule(opponents, { rule: "cant-win-game" })] },
    });
  });

  it("separates the zero-life exception from a general can't-lose rule", () => {
    assert.deepEqual(effect("You don't lose the game for having 0 or less life.", "Enchantment"), rule(you, { rule: "no-loss-at-zero-life" }));
    assert.deepEqual(effect("You can't lose the game.", "Legendary Enchantment"), rule(you, { rule: "cant-lose-game" }));
    assert.deepEqual(effect("You have no maximum hand size and don't lose the game for having 0 or less life.", "Legendary Artifact — Book"), {
      static: "group", effects: [rule(you, noMaximum), rule(you, { rule: "no-loss-at-zero-life" })] });
    rejects("You don't lose the game for having 1 or less life.", "Enchantment");
    rejects("You don't lose the game for having ten or more poison counters.", "Enchantment");
  });

  it("gives resolving prohibitions their own durations", () => {
    assert.deepEqual(only("Players can't gain life this turn. Damage can't be prevented this turn. ~ deals 3 damage to any target.", "Instant"), {
      kind: "spell", effects: [
        { steps: [{ effect: "player-rule", rule: { who: players, rule: "cant-gain-life" }, duration: { duration: "this-turn" } }] },
        { steps: [{ effect: "prohibit", prohibition: { prohibition: "prevent-damage", kind: "all" }, duration: { duration: "this-turn" } }] },
        { steps: [{ effect: "damage", source: { ref: "self" }, amount: fixed(3), to: { to: "any-target" } }] },
      ],
    });
    assert.deepEqual(only("You can't lose the game this turn and your opponents can't win the game this turn.", "Instant"), {
      kind: "spell", effects: [{ steps: [
        { effect: "player-rule", rule: { who: you, rule: "cant-lose-game" }, duration: { duration: "this-turn" } },
        { effect: "player-rule", rule: { who: opponents, rule: "cant-win-game" }, duration: { duration: "this-turn" } },
      ] }],
    });
    assert.deepEqual(only("Players can't lose life this turn and players can't lose the game or win the game this turn.", "Instant"), {
      kind: "spell", effects: [{ steps: [
        { effect: "player-rule", rule: { who: players, rule: "cant-lose-life" }, duration: { duration: "this-turn" } },
        { effect: "simultaneous", steps: [
          { effect: "player-rule", rule: { who: players, rule: "cant-lose-game" }, duration: { duration: "this-turn" } },
          { effect: "player-rule", rule: { who: players, rule: "cant-win-game" }, duration: { duration: "this-turn" } },
        ] },
      ] }],
    });
  });

  it("keeps the damaged player of a trigger symbolic for the rest of the game", () => {
    assert.deepEqual(only("Whenever this creature deals damage to a player, that player can't gain life for the rest of the game.", "Creature — Elemental Shaman"), {
      kind: "triggered", trigger: { trigger: "deals-damage", what: { ref: "this", noun: "creature" }, to: "any" },
      effects: [{ steps: [{ effect: "player-rule", rule: { who: { player: "that-player" }, rule: "cant-gain-life" }, duration: { duration: "rest-of-game" } }] }],
    });
  });

  it("rejects static rules about unnamed or targeted players and partial lists", () => {
    rejects("That player can't gain life.", "Enchantment");
    rejects("Target player can't gain life.", "Enchantment");
    rejects("An opponent can't gain life.", "Enchantment");
    rejects("Your opponent can't gain life or remove poison counters.", "Legendary Creature — Phyrexian Beast");
    rejects("Players can't gain life. Draw a card.", "Sorcery");
  });
});

describe("unspent mana", () => {
  it("records whose mana is kept and which color", () => {
    assert.deepEqual(effect("Players don't lose unspent mana as steps and phases end.", "Enchantment"), rule(players, { rule: "keep-unspent-mana", mana: "any" }));
    assert.deepEqual(effect("You don't lose unspent red mana as steps and phases end.", "Creature — Dragon"), rule(you, { rule: "keep-unspent-mana", mana: "red" }));
  });

  it("gives the resolving form its leading duration", () => {
    assert.deepEqual(only("Until end of turn, you don't lose unspent red mana as steps and phases end.", "Instant"), {
      kind: "spell", effects: [{ steps: [{ effect: "player-rule", rule: { who: you, rule: "keep-unspent-mana", mana: "red" }, duration: { duration: "end-of-turn" } }] }],
    });
  });

  it("does not read a phase-only or step-only wording as the whole rule", () => {
    rejects("You don't lose unspent red mana as steps end.", "Creature — Dragon");
    rejects("You don't lose unspent mana.", "Creature — Dragon");
  });
});
