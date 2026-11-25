// Exact printed paragraphs that establish links the general walk cannot prove.
import { isDeepStrictEqual } from "node:util";
import type { Ability, Effect, EntryCopy, KeywordInstance, PlayerRef, Sentence, StaticEffect } from "../../ast.ts";

// Double Major's complete spell paragraph. A retarget option, another copy
// exception or an enclosing choice would require different bindings.
export const DIRECT_CREATURE_SPELL_COPY = {
  effect: "copy-stack-object", who: { player: "you" }, count: { amount: "fixed", value: 1 },
  what: { ref: "target", count: { amount: "fixed", value: 1 }, selectionId: "line:0:1",
    filter: { cls: "spell", types: ["creature"], control: "you" } },
  copyExceptions: [{ change: "types", types: { supertypes: { operation: "remove", types: ["legendary"] } } }],
} satisfies Effect;
// The singular match, the remainder and the exile successor have different
// identities. Only the complete sequence below establishes all three links.
export const REVEAL_AND_CAST = [
  { steps: [
    { effect: "shuffle", who: { player: "you" }, library: { zone: "library", owner: "your" } },
    { effect: "scan-library", who: { player: "you" }, operation: "reveal", library: { zone: "library", owner: "your" },
      until: { count: { amount: "fixed", value: 1 }, matching: { nonTypes: ["land"], cls: "card" } } },
  ] },
  { steps: [
    { effect: "exile", what: { ref: "that", filter: { cls: "card" } } },
    { effect: "move-zone", what: { ref: "rest" }, to: { zone: "library", owner: "your", position: "bottom" }, order: "random" },
  ] },
  { steps: [{ effect: "play-card", optional: true, instruction: { who: { player: "you" }, action: "cast",
    what: { ref: "related", number: "singular", filter: { cls: "card" }, relation: { relation: "event-result", event: "exile", scope: "contextual" } },
    alternativeCost: { cost: "without-mana-cost" } } }] },
] as const satisfies Sentence[];
export function isRevealAndCast(ability: Extract<Ability, { kind: "spell" }>): ability is { kind: "spell"; effects: typeof REVEAL_AND_CAST } {
  const { abilityWord, ...paragraph } = ability;
  return abilityWord === undefined && isDeepStrictEqual(paragraph, { kind: "spell", effects: REVEAL_AND_CAST });
}
// CR 607.2a: "the exiled card" in this resolution instruction refers to a
// different printed ability. Match its entire paragraph, including the cost
// and resolution condition, before supplying a cross-ability reference.
export const HIDEAWAY_PLAY = {
  effect: "play-card", optional: true, instruction: {
    who: { player: "you" }, action: "play",
    what: { ref: "related", number: "singular", filter: { cls: "card" },
      relation: { relation: "event-result", event: "exile", scope: "contextual" } },
    alternativeCost: { cost: "without-mana-cost" },
    condition: { condition: "compare", value: { amount: "attribute-sum", attribute: "power",
      among: { ref: "each", filter: { types: ["creature"], control: "you" } } },
      comparison: { op: "ge", value: { amount: "fixed", value: 10 } } },
  },
} as const satisfies Effect;
export const HIDEAWAY_DAMAGE_PLAY = {
  ...HIDEAWAY_PLAY, instruction: {
    ...HIDEAWAY_PLAY.instruction,
    condition: { condition: "damage-history", comparison: { op: "ge", value: { amount: "fixed", value: 7 } },
      quantity: "total", to: { player: "an-opponent" }, during: "this-turn" },
  },
} as const satisfies Effect;
export const HIDEAWAY_FOUR = { keyword: "hideaway", amount: 4 } as const satisfies KeywordInstance;
// Only these independent entry and mana paragraphs may accompany the pair.
// A second hideaway, another exile producer or a granted ability must never
// supply this link. Printed order is immaterial.
export function hideawayProducer(abilities: Ability[], activation: Extract<Ability, { kind: "activated" }>):
  { index: number; play: typeof HIDEAWAY_PLAY | typeof HIDEAWAY_DAMAGE_PLAY } | undefined {
  const { abilityWord, restriction, ...paragraph } = activation;
  if (abilityWord !== undefined || restriction !== undefined) return;
  const lane = [[HIDEAWAY_PLAY, "G"], [HIDEAWAY_DAMAGE_PLAY, "R"]].find(([play, color]) =>
    isDeepStrictEqual(paragraph, { kind: "activated", costs: [{ cost: "mana", symbols: [color] }, { cost: "tap-self" }],
      effects: [{ steps: [play] }] })) as [typeof HIDEAWAY_PLAY | typeof HIDEAWAY_DAMAGE_PLAY, string] | undefined;
  if (!lane) return;
  const [play, color] = lane;
  let producer: number | undefined;
  for (const [index, ability] of abilities.entries()) {
    if (ability === activation) continue;
    if (isDeepStrictEqual(ability, { kind: "keywords", keywords: [HIDEAWAY_FOUR] }) && producer === undefined) {
      producer = index;
      continue;
    }
    if (ability.kind === "static") {
      const { abilityWord, ...entry } = ability;
      if (abilityWord === undefined && isDeepStrictEqual(entry, {
        kind: "static", effect: { static: "enters-tapped", what: { ref: "this", noun: "land" } },
      })) continue;
    }
    if (ability.kind === "activated") {
      const { abilityWord, restriction, ...mana } = ability;
      if (abilityWord === undefined && restriction === undefined && isDeepStrictEqual(mana, {
        kind: "activated", costs: [{ cost: "tap-self" }], effects: [{ steps: [{ effect: "add-mana",
          who: { player: "you" }, mana: { mana: "fixed", symbols: [color] } }] }],
      })) continue;
    }
    return undefined;
  }
  return producer === undefined ? undefined : { index: producer, play };
}
export type OptionalEntryCopy = EntryCopy & { model: Extract<EntryCopy["model"], { model: "choose" }>; optional: { who: PlayerRef } };
export function ordinaryEntryCopy(effect: EntryCopy): effect is OptionalEntryCopy {
  const { what, model, optional, duration, changes, entryExceptions } = effect;
  if ((what.ref !== "self" && what.ref !== "this") || model.model !== "choose" || model.who.player !== "you" ||
      optional?.who.player !== "you" || model.filter.zoneChangeHistory ||
      (model.filter.zone && model.filter.zone.zone !== "battlefield") ||
      (duration && duration.duration !== "end-of-turn")) return false;
  if (entryExceptions?.some(exception => exception.count.amount !== "fixed")) return false;
  return !changes?.some(change => change.change !== "types" &&
    !(change.change === "abilities" && change.abilities.every(ability => isDeepStrictEqual(ability,
      { gained: "keyword", keyword: { keyword: "haste" } }))));
}
export const PROTECTION_FROM_EVERYTHING = { keyword: "protection", from: [{ scope: "everything" }] } satisfies KeywordInstance;
// Match the complete copy clause, independently of any other printed abilities.
// Different model filters, copy exceptions or entry modifiers need their own analysis.
export const OPTIONAL_SAME_TURN_ENTRY_COPY = {
  static: "entry-copy", what: { ref: "this", noun: "creature" },
  model: { model: "choose", who: { player: "you" }, count: 1, when: "before-entry",
    filter: { cls: "permanent", zoneChangeHistory: { event: "enters", to: { zone: "battlefield" }, during: "this-turn" } } },
  optional: { who: { player: "you" } },
} as const satisfies StaticEffect;
export function isSameTurnEntryCopy(ability: Ability): ability is { kind: "static"; effect: typeof OPTIONAL_SAME_TURN_ENTRY_COPY } {
  return ability.kind === "static" && ability.abilityWord === undefined && ability.effect.static === "entry-copy" &&
    isDeepStrictEqual(ability.effect, OPTIONAL_SAME_TURN_ENTRY_COPY);
}
export const SELF_GRAVEYARD_REPLACEMENT = {
  static: "replacement",
  replacement: {
    event: { event: "move-zone", what: { ref: "self" }, to: { zone: "graveyard", owner: "any" }, from: "anywhere" },
    occurrence: "each",
    replace: { replacement: "effects", effects: [{ steps: [
      { effect: "reveal", who: { player: "you" }, what: { ref: "replacement-event-object", role: "affected" } },
      { effect: "shuffle-into-library", who: { player: "you" }, what: [{ ref: "replacement-event-object", role: "affected" }],
        library: { zone: "library", owner: "its-owner" } },
    ] }] },
  },
} as const satisfies StaticEffect;
export function isSelfGraveyardReplacement(ability: Ability, cardAbilities: Ability[] | undefined): boolean {
  if (ability.kind !== "static" || cardAbilities?.length !== 2 || cardAbilities[1] !== ability) return false;
  const { abilityWord, ...paragraph } = ability;
  return abilityWord === undefined && isDeepStrictEqual(paragraph, { kind: "static", effect: SELF_GRAVEYARD_REPLACEMENT }) &&
    isDeepStrictEqual(cardAbilities[0], { kind: "keywords", keywords: [PROTECTION_FROM_EVERYTHING] });
}
