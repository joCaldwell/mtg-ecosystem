import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

const fixed = (value: number) => ({ amount: "fixed" as const, value });
function abilities(text: string, typeLine?: string): Ability[] {
  const parsed = parseOracleText(text, undefined, typeLine ? { typeLine } : {});
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  if (!parsed.ok) throw new Error(text);
  return JSON.parse(JSON.stringify(parsed.abilities));
}
function rejects(text: string, typeLine?: string): void {
  assert.equal(parseOracleText(text, undefined, typeLine ? { typeLine } : {}).ok, false, text);
}

// Scryfall: Kalain, Reclusive Painter; Coin of Mastery; Verazol, the Split Current; The Spike Cactus.
// CR 106.3 (mana remembers its source), 601.2f/h, 609.4b (actual mana spent).
describe("for each mana spent to cast", () => {
  it("counts only mana produced by a matching source", () => {
    assert.deepEqual(abilities("Other creatures you control enter with an additional +1/+1 counter on them for each mana from a Treasure spent to cast them."), [{
      kind: "static", effect: { static: "enters-with-counters", counter: { type: "+1/+1" }, count: fixed(1), additional: true,
        what: { ref: "each", filter: { other: true, types: ["creature"], control: "you" } },
        multiplier: { amount: "mana-spent", what: { ref: "them" }, quantity: "total", source: { subtypes: ["Treasure"] } } },
    }]);
  });

  it("counts all mana spent when no source is stated", () => {
    assert.deepEqual(abilities("~ enters with a +1/+1 counter on it for each mana spent to cast it.", "Creature — Serpent"), [{
      kind: "static", effect: { static: "enters-with-counters", what: { ref: "self" }, counter: { type: "+1/+1" }, count: fixed(1),
        multiplier: { amount: "mana-spent", what: { ref: "it" }, quantity: "total" } },
    }]);
  });

  it("reads plural counters equal to an amount as that amount of counters", () => {
    assert.deepEqual(abilities("~ enters the battlefield with +1/+1 counters on it equal to the amount of mana spent to cast it.", "Creature — Spike"), [{
      kind: "static", effect: { static: "enters-with-counters", what: { ref: "self" }, counter: { type: "+1/+1" },
        count: { amount: "mana-spent", what: { ref: "it" }, quantity: "total" } },
    }]);
  });

  it("keeps near-misses failing", () => {
    rejects("~ enters with a +1/+1 counter on it for each mana from spent to cast it.", "Creature — Serpent");
    rejects("~ enters with a +1/+1 counter on it for each mana spent to activate it.", "Creature — Serpent");
    rejects("~ enters with +1/+1 counters on it equal to.", "Creature — Serpent");
  });
});

// Scryfall: Cryptic Cruiser, Void Maw, Leashling, Battlefield Scrounger.
// CR 118.1 (a cost is paid by the player), 400.3 (cards go to their owner's zone), 401.4.
describe("put a card into a zone as a cost", () => {
  it("puts a card an opponent owns from exile into its owner's graveyard", () => {
    assert.deepEqual(abilities("{2}{U}, Put a card an opponent owns from exile into that player's graveyard: Tap target creature.", "Creature — Eldrazi Processor"), [{
      kind: "activated", costs: [{ cost: "mana", symbols: ["2", "U"] },
        { cost: "move-zone", what: { ref: "filter", filter: { cls: "card", own: "opponent", zone: { zone: "exile" } }, count: fixed(1) },
          to: { zone: "graveyard", owner: "that-player" } }],
      effects: [{ steps: [{ effect: "tap", what: { ref: "target", filter: { types: ["creature"] }, count: fixed(1) } }] }],
    }]);
  });

  it("puts cards from a graveyard on the bottom of a library", () => {
    assert.deepEqual(abilities("Put three cards from your graveyard on the bottom of your library: Draw a card.", "Creature — Centaur"), [{
      kind: "activated", costs: [{ cost: "move-zone", what: { ref: "filter", filter: { cls: "card", zone: { zone: "graveyard", owner: "your" } }, count: fixed(3) },
        to: { zone: "library", owner: "your", position: "bottom" } }],
      effects: [{ steps: [{ effect: "draw", who: { player: "you" }, amount: fixed(1) }] }],
    }]);
  });

  it("keeps counter costs and unsupported destinations failing", () => {
    rejects("Put a card from your hand onto the battlefield: Draw a card.", "Creature — Dog");
    rejects("Put a card from your hand into your library: Draw a card.", "Creature — Dog");
    rejects("Put each card from your hand on top of your library: Draw a card.", "Creature — Dog");
    assert.equal(abilities("{T}, Put a +1/+1 counter on ~: Draw a card.", "Creature — Dog")[0]!.kind, "activated");
  });
});

// Scryfall: The Ur-Dragon; Cunning Nightbonder; Thryx, the Sudden Storm.
describe("Eminence zones and cost changes that also make spells uncounterable", () => {
  it("applies while the commander is in either of two zones", () => {
    assert.deepEqual(abilities("Eminence — As long as ~ is in the command zone or on the battlefield, other Dragon spells you cast cost {1} less to cast.", "Legendary Creature — Dragon"), [{
      kind: "static", abilityWord: "Eminence", effect: { static: "conditional",
        condition: { condition: "is-filter", what: { ref: "self" }, filter: { anyOf: [{ zone: { zone: "command" } }, { zone: { zone: "battlefield" } }] } },
        effect: { static: "cost-modification", change: { direction: "reduce", symbols: ["1"] },
          scope: { action: "cast", caster: { player: "you" }, spells: { ref: "each", filter: { other: true, subtypes: ["Dragon"], cls: "spell" } } } } },
    }]);
  });

  it("does not read a bare second zone as a zone", () => {
    rejects("As long as ~ is in the command zone or battlefield, other Dragon spells you cast cost {1} less to cast.", "Legendary Creature — Dragon");
  });

  it("makes the same spells uncounterable", () => {
    const filter = { cls: "spell", withAbility: ["flash"] };
    assert.deepEqual(abilities("Spells with flash you cast cost {1} less to cast and can't be countered.", "Creature — Human Rogue"), [{
      kind: "static", effect: { static: "group", effects: [
        { static: "cost-modification", change: { direction: "reduce", symbols: ["1"] },
          scope: { action: "cast", caster: { player: "you" }, spells: { ref: "each", filter } } },
        { static: "cant", action: "be-countered", what: { ref: "each", filter: { ...filter, cast: { by: { player: "you" } } } } }] },
    }]);
  });

  it("keeps ordinal, own-spell and trailing-clause forms failing", () => {
    rejects("The first spell you cast each turn costs {1} less to cast and can't be countered.");
    rejects("This spell costs {1} less to cast and can't be countered.", "Instant");
    rejects("Spells you cast cost {1} less to cast and can't be countered if you control a Wizard.");
  });
});

// Scryfall: Belt of Giant Strength, Warrior's Blades, Dragonfire Blade.
// CR 601.2c: targets are chosen before 601.2f determines the total cost.
describe("cost changes that depend on what the ability targets", () => {
  const targeted = { ref: "targeted-by", source: { ref: "this", noun: "ability" }, filter: { types: ["creature"] } };
  const equip = (cost: string, modification: object) => [{ kind: "keywords", keywords: [{ keyword: "equip",
    cost: [{ cost: "mana", symbols: [cost] }], costModifications: [modification] }] }];
  it("uses a characteristic of the targeted creature as X", () => {
    assert.deepEqual(abilities("Equip {10}. This ability costs {X} less to activate, where X is the power of the creature it targets.", "Artifact — Equipment"),
      equip("10", { change: { direction: "reduce", symbols: ["X"] },
        bindings: [{ variable: "x", value: { amount: "attribute", of: targeted, attribute: "power" } }] }));
  });
  it("counts counters on, or colors of, the targeted creature", () => {
    assert.deepEqual(abilities("Equip {3}. This ability costs {1} less to activate for each +1/+1 counter on the creature it targets.", "Artifact — Equipment"),
      equip("3", { change: { direction: "reduce", symbols: ["1"] }, multiplier: { amount: "counter-count", counter: { type: "+1/+1" }, on: targeted } }));
    assert.deepEqual(abilities("Equip {4}. This ability costs {1} less to activate for each color of the creature it targets.", "Artifact — Equipment"),
      equip("4", { change: { direction: "reduce", symbols: ["1"] }, multiplier: { amount: "distinct-count", characteristic: "color", among: targeted } }));
  });
  it("keeps other antecedents for \"it\" failing", () => {
    rejects("Equip {3}. This ability costs {1} less to activate for each +1/+1 counter on the creature it blocks.", "Artifact — Equipment");
    rejects("Equip {3}. This ability costs {1} less to activate for each +1/+1 counter on the creature it targets and control.", "Artifact — Equipment");
  });
});

// Scryfall: Font of Magic; Henzie "Toolbox" Torre (CR 903.8 counts casts from the command zone).
describe("commander casts as a cost-change multiplier", () => {
  const casts = (commander: object) => ({ amount: "cast-count", who: { player: "you" }, what: { cls: "spell", commander: true, ...commander },
    during: "this-game", from: { zone: "command" } });
  it("counts casts of any commander", () => {
    assert.deepEqual(abilities("Instant and sorcery spells you cast cost {1} less to cast for each time you've cast a commander from the command zone this game."), [{
      kind: "static", effect: { static: "cost-modification", change: { direction: "reduce", symbols: ["1"] }, multiplier: casts({}),
        scope: { action: "cast", caster: { player: "you" }, spells: { ref: "each", filter: { allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }], cls: "spell" } } } },
    }]);
  });
  it("counts casts of the player's own commander and adjusts blitz costs", () => {
    assert.deepEqual(abilities("Blitz costs you pay cost {1} less for each time you've cast your commander from the command zone this game."), [{
      kind: "static", effect: { static: "cost-modification", change: { direction: "reduce", symbols: ["1"] }, multiplier: casts({ own: "you" }),
        scope: { action: "pay-cost", cost: "blitz", payer: { player: "you" } } },
    }]);
  });
  it("keeps other histories failing", () => {
    rejects("Spells you cast cost {1} less to cast for each time you've cast your commander this game.");
    rejects("Spells you cast cost {1} less to cast for each time you've cast your commander from your hand this game.");
  });
});

// Scryfall: Torrent Elemental, Startled Awake // Persistent Nightmare, Urban Retreat.
describe("put this card from a zone onto the battlefield", () => {
  it("states the origin of the card", () => {
    assert.deepEqual(abilities("{3}{U}{U}: Put this card from your graveyard onto the battlefield transformed. Activate only as a sorcery.", "Creature — Human"), [{
      kind: "activated", costs: [{ cost: "mana", symbols: ["3", "U", "U"] }],
      effects: [{ steps: [{ effect: "move-zone", what: { ref: "this", noun: "card" }, to: { zone: "battlefield" },
        entry: { face: "transformed" }, from: { zone: "graveyard", owner: "your" } }] }], restriction: { restriction: "sorcery" },
    }]);
  });
  it("rejects a second origin", () => {
    rejects("{3}: Put this card from your graveyard onto the battlefield from exile.", "Creature — Human");
  });
});
