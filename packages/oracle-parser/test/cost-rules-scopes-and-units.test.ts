import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { Ability } from "../src/ast.ts";
import { parseOracleText } from "../src/parser/index.ts";

const you = { player: "you" as const };
function abilities(text: string, typeLine?: string): Ability[] {
  const parsed = parseOracleText(text, undefined, typeLine ? { typeLine } : {});
  assert.equal(parsed.ok, true, JSON.stringify(parsed.lines));
  if (!parsed.ok) throw new Error(text);
  return JSON.parse(JSON.stringify(parsed.abilities));
}
function rejects(text: string, typeLine?: string): void {
  assert.equal(parseOracleText(text, undefined, typeLine ? { typeLine } : {}).ok, false, text);
}

// Scryfall: Aven Interrupter, Doc Aurlock, Gonti Canny Acquisitor, Paladin Class, Momo, Beluna Grandsquall.
// CR 601.2f: the adjustment applies to the cost of a spell as it is cast; CR 715.1: Adventurer cards.
describe("cast scopes of cost adjustments", () => {
  it("accepts a union of casting origins", () => {
    assert.deepEqual(abilities("Spells your opponents cast from graveyards or from exile cost {2} more to cast.", "Creature — Bird"), [{
      kind: "static", effect: { static: "cost-modification", change: { direction: "increase", symbols: ["2"] },
        scope: { action: "cast", spells: { ref: "each", filter: { cls: "spell" } }, caster: { player: "each-opponent" },
          from: { anyOf: [{ zone: "graveyard" }, { zone: "exile" }] } } },
    }]);
  });

  it("reads 'but don't own' as the caster not owning the spell", () => {
    assert.deepEqual(abilities("Spells you cast but don't own cost {1} less to cast.", "Creature — Human"), [{
      kind: "static", effect: { static: "cost-modification", change: { direction: "reduce", symbols: ["1"] },
        scope: { action: "cast", spells: { ref: "each", filter: { cls: "spell", own: "not-you" } }, caster: you } },
    }]);
  });

  it("turns 'during your turn' after the cast verb into the same condition as the leading form", () => {
    assert.deepEqual(abilities("Spells your opponents cast during your turn cost {1} more to cast.", "Enchantment — Class"), [{
      kind: "static", effect: { static: "conditional", condition: { condition: "your-turn" },
        effect: { static: "cost-modification", change: { direction: "increase", symbols: ["1"] },
          scope: { action: "cast", spells: { ref: "each", filter: { cls: "spell" } }, caster: { player: "each-opponent" } } } },
    }]);
  });

  it("counts an ordinal spell within each of the player's own turns", () => {
    assert.deepEqual(abilities("The first creature spell you cast during each of your turns costs {1} less to cast.", "Creature — Bird"), [{
      kind: "static", effect: { static: "conditional", condition: { condition: "your-turn" },
        effect: { static: "cost-modification", change: { direction: "reduce", symbols: ["1"] },
          scope: { action: "cast", spells: { ref: "each", filter: { types: ["creature"], cls: "spell" } }, caster: you,
            occurrence: { ordinal: 1, period: "turn" } } } },
    }]);
  });

  it("filters spells that have an Adventure", () => {
    assert.deepEqual(abilities("Permanent spells you cast that have an Adventure cost {1} less to cast.", "Creature — Giant"), [{
      kind: "static", effect: { static: "cost-modification", change: { direction: "reduce", symbols: ["1"] },
        scope: { action: "cast", spells: { ref: "each", filter: { cls: "spell", permanent: true, adventure: true } }, caster: you } },
    }]);
  });

  it("keeps near-misses failing", () => {
    rejects("Spells your opponents cast from graveyards or exile cost {2} more to cast.", "Creature — Bird");
    rejects("Spells your opponents cast but don't own cost {1} more to cast.", "Creature — Bird");
    rejects("Spells you cast during your turn and during their turn cost {1} less to cast.", "Creature — Bird");
    rejects("Spells you cast that have an Aura cost {1} less to cast.", "Creature — Bird");
    // A turn qualifier stated twice is not silently merged.
    rejects("During your turn, spells your opponents cast during your turn cost {1} more to cast.", "Creature — Bird");
  });
});

// Scryfall: Fast Forward, Search Party Captain, Astral Confrontation, Gnoll War Band, Hollow One.
// CR 506.3 (only players, planeswalkers and battles are attacked), 508.1, 702.29a (cycling discards the card).
describe("history and combat units after 'for each' in a cost reduction", () => {
  const reduce = (unit: string) => abilities(`This spell costs {1} less to cast for each ${unit}.`, "Instant");
  const multiplier = (unit: string) => (reduce(unit)[0] as { effect: { multiplier: unknown } }).effect.multiplier;

  it("counts opponents attacked or being attacked by the caster", () => {
    assert.deepEqual(multiplier("opponent you attacked this turn"),
      { amount: "attacked-player-count", players: "opponents", which: { by: you, during: "this-turn" } });
    assert.deepEqual(multiplier("opponent you're attacking"),
      { amount: "attacked-player-count", players: "opponents", which: { status: "being-attacked", by: you } });
  });

  it("counts creatures the caster attacked with", () => {
    assert.deepEqual(multiplier("creature you attacked with this turn"),
      { amount: "combat-history-count", action: "attacked", during: "this-turn", by: you, distinct: "object-incarnations" });
  });

  it("counts opponents who were dealt damage", () => {
    assert.deepEqual(multiplier("opponent who was dealt damage this turn"), {
      amount: "player-count",
      of: { player: "filtered", base: { player: "each-opponent" }, filter: { damageHistory: { role: "recipient", window: { during: "this-turn" } } } },
    });
  });

  it("treats cycled cards as discarded cards", () => {
    assert.deepEqual(multiplier("card you've cycled or discarded this turn"),
      { amount: "player-action-count", who: you, action: { action: "discard", what: { cls: "card" } }, during: "this-turn" });
  });

  it("keeps near-misses failing", () => {
    rejects("This spell costs {1} less to cast for each opponent you're attacked.", "Instant");
    rejects("This spell costs {1} less to cast for each creature you attacked this turn.", "Instant");
    rejects("This spell costs {1} less to cast for each card you've cycled this turn.", "Instant");
  });
});

// Scryfall: Huskburster Swarm, Sailors' Bane.
// CR 400.1: an object is in one zone at a time; "in exile and in your graveyard" is the union of both zones.
describe("zone unions and predicate lists in cost-reduction counts", () => {
  it("joins two zones of the same description as a union, not an intersection", () => {
    assert.deepEqual(abilities("This spell costs {1} less to cast for each creature card you own in exile and in your graveyard.", "Creature — Insect"), [{
      kind: "static", effect: { static: "cost-modification", scope: { action: "cast", spells: { ref: "this", noun: "spell" } },
        change: { direction: "reduce", symbols: ["1"] },
        multiplier: { amount: "count", of: { types: ["creature"], cls: "card", own: "you",
          allOf: [{ anyOf: [{ zone: { zone: "exile" } }, { zone: { zone: "graveyard", owner: "your" } }] }] } } },
    }]);
  });

  it("reads a comma list of predicate descriptions ending in 'or'", () => {
    assert.deepEqual(abilities("This spell costs {1} less to cast for each card in your graveyard that's an instant card, a sorcery card, or a card that has an Adventure.", "Creature — Dragon"), [{
      kind: "static", effect: { static: "cost-modification", scope: { action: "cast", spells: { ref: "this", noun: "spell" } },
        change: { direction: "reduce", symbols: ["1"] },
        multiplier: { amount: "count", of: { cls: "card", zone: { zone: "graveyard", owner: "your" }, anyOf: [
          { types: ["instant"], cls: "card" }, { types: ["sorcery"], cls: "card" }, { cls: "card", adventure: true }] } } },
    }]);
  });

  it("rejects an unterminated predicate list", () => {
    rejects("This spell costs {1} less to cast for each card in your graveyard that's an instant card, a sorcery card.", "Creature — Dragon");
  });
});
