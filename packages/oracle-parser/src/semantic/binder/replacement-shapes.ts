// Supported replacement-effect and linked-exile shapes.
import { isDeepStrictEqual } from "node:util";
import type { Ability, ObjectRef, PlayerRef, ReplacementInstruction } from "../../ast.ts";
import type { BindingNode } from "../types.ts";
import { objectCardinality } from "./context.ts";
import { isRecord } from "./predicates.ts";

export type QuantityUnit = Extract<BindingNode, { node: "quantity-replacement" }>["unit"];
export function replacementPlayer(player: PlayerRef | undefined): boolean {
  return player !== undefined && ["you", "a-player", "an-opponent"].includes(player.player);
}
export function replacementObject(object: ObjectRef | PlayerRef | undefined, excludeRecipient = false): boolean {
  if (!object || !("ref" in object)) return false;
  if (object.ref === "equipped") return object.noun === "creature";
  return object.ref === "filter" && object.count.amount === "fixed" && object.count.value === 1 &&
    Object.keys(object).length === 3 && Object.keys(object.filter).every(key => ["cls", "types", "supertypes", "control", "colors"].includes(key) ||
      (key === "anyOf" && object.filter.anyOf?.every(option =>
        Object.keys(option).length === 1 && (option.colors?.length === 1 || option.types?.length === 1))) ||
      (excludeRecipient && key === "otherThan" && isDeepStrictEqual(object.filter.otherThan, { ref: "replacement-event-object", role: "recipient" })));
}
export function quantityUnit(event: ReplacementInstruction["event"]): QuantityUnit | undefined {
  switch (event.event) {
    case "create-token":
      if (event.match === "creation-event" && replacementPlayer(event.under) &&
          event.tokens?.cls === "token" && Object.keys(event.tokens).every(key => ["cls", "types"].includes(key)) &&
          event.quantity.value.amount === "fixed") return "token-specification";
      return;
    case "tap-for-mana":
      if (replacementObject(event.what) && (!event.by || replacementPlayer(event.by)) &&
          (!event.quantity || event.quantity.value.amount === "fixed")) return "mana-specification";
      return;
    case "damage":
      if (replacementObject(event.source, event.kind === "combat" && event.to?.length === 1 &&
          !("player" in event.to[0]) && replacementObject(event.to[0])) &&
          event.to?.every(ref => "player" in ref ? replacementPlayer(ref) : replacementObject(ref)) !== false &&
          (!event.quantity || event.quantity.value.amount === "fixed")) return "damage-source-and-recipient";
      return;
    case "gain-life": return replacementPlayer(event.who) ? "life-source-and-recipient" : undefined;
    case "put-counters":
      if (event.on.length > 0 && event.on.every(ref => replacementObject(ref)) && (!event.by || replacementPlayer(event.by)) &&
          event.quantity.value.amount === "fixed") return "counter-kind-and-recipient";
      return;
  }
}
export type LinkedExileOrigin = { ability: string; path: string; imprint: boolean; rule: "607.1c" | "607.2a" | "607.2b" };
export function seeksLinkedExile(value: unknown, imprint: boolean): boolean {
  if (Array.isArray(value)) return value.some(entry => seeksLinkedExile(entry, imprint));
  if (!isRecord(value)) return false;
  return value.relation === "linked-exile" ||
    (imprint && value.relation === "event-result" && value.event === "exile" && value.scope === "contextual") ||
    Object.values(value).some(entry => seeksLinkedExile(entry, imprint));
}
const SINGLE = { amount: "fixed", value: 1 } as const;
const AFFECTED_MOVE = { ref: "replacement-event-object", role: "affected" } as const;

export function simpleExileBody(replace: ReplacementInstruction["replace"]) {
  if (replace.replacement !== "effects") return;
  const exile = replace.effects[0]?.steps[0];
  if (exile?.effect === "exile" && isDeepStrictEqual(replace, {
    replacement: "effects", effects: [{ steps: [{ effect: "exile", what: AFFECTED_MOVE }] }],
  })) return exile;
}

/** CR 400.6, 614.1a: each proposed move has one object. Category
 * alternatives such as "a card or token" describe that same object. */
export function singleGraveyardExile(replacement: ReplacementInstruction) {
  const { event, replace } = replacement;
  if (event.event !== "move-zone" || event.what.ref !== "filter" ||
      !isDeepStrictEqual(replacement, { event, replace, occurrence: "each" })) return;
  const { what, to } = event;
  if (!isDeepStrictEqual(what, { ref: "filter", filter: what.filter, count: SINGLE })) return;

  const card = { cls: "card" };
  const cardOrToken = { allOf: [{ anyOf: [card, { cls: "token" }] }] };
  const spellCard = { cls: "card", allOf: [{ anyOf: [{ types: ["instant"] }, { types: ["sorcery"] }] }] };
  const battlefield = isDeepStrictEqual(event, { event: "move-zone", what, to: { zone: "graveyard", owner: "any" } }) &&
    isDeepStrictEqual(what.filter, { cls: "permanent" });
  const anywhere = isDeepStrictEqual(event, { event: "move-zone", what, from: "anywhere", to }) &&
    ([
      { owner: "any", filters: [card, cardOrToken, spellCard] },
      { owner: "your", filters: [card, cardOrToken] },
      { owner: "an-opponent", filters: [card] },
    ].some(({ owner, filters }) => isDeepStrictEqual(to, { zone: "graveyard", owner }) &&
      filters.some(filter => isDeepStrictEqual(what.filter, filter))));
  if (!battlefield && !anywhere) return;
  const from = battlefield ? "battlefield" : "anywhere";
  const exile = simpleExileBody(replace);
  if (exile) return { event, from, exile } as const;
  // A fixed void counter on the exiled object adds no participant.
  if (from === "anywhere" && isDeepStrictEqual(to, { zone: "graveyard", owner: "an-opponent" }) &&
      replace.replacement === "effects" && isDeepStrictEqual(replace, { replacement: "effects", effects: [{ steps: [{
        effect: "exile", what: AFFECTED_MOVE, counters: [{ counter: { type: "void" }, count: SINGLE }],
      }] }] })) {
    const withCounter = replace.effects[0].steps[0];
    if (withCounter.effect === "exile") return { event, from, exile: withCounter } as const;
  }
}

/** A per-object move altered to exile, with only the supported follow-up shape. */
export function exileReplacementParts(replacement: ReplacementInstruction) {
  const { event, replace } = replacement;
  if (replacement.occurrence !== "each" || replacement.optional || replacement.condition ||
      event.event !== "move-zone" || event.resolving ||
      event.what.ref !== "filter" || event.what.count.amount !== "fixed" || event.what.count.value !== 1 ||
      replace.replacement !== "effects" || replace.effects.length < 1 || replace.effects.length > 2 ||
      replace.effects.some(sentence => Object.keys(sentence).length !== 1)) return;
  const damageHistory = isDeepStrictEqual(event.what.filter, { types: ["creature"], damageHistory: {
    role: "recipient", window: { during: "this-turn" },
    source: { ref: "filter", filter: { cls: "source", control: "you" }, count: { amount: "fixed", value: 1 } },
  } }) && replace.effects.length === 1 && replace.effects[0].steps.length === 1;
  const battlefield = isDeepStrictEqual(event.from, { zone: "battlefield" }) &&
    isDeepStrictEqual(event.to, { zone: "graveyard" }) && !event.notControlledByBefore &&
    isDeepStrictEqual(event.what.filter.types, ["creature"]) &&
    (damageHistory || Object.keys(event.what.filter).every(key => ["nonToken", "types", "control"].includes(key)));
  const opponentGraveyard = event.from === "anywhere" &&
    isDeepStrictEqual(event.to, { zone: "graveyard", owner: "an-opponent" }) &&
    isDeepStrictEqual(event.notControlledByBefore, { player: "you" }) &&
    isDeepStrictEqual(event.what, { ref: "filter", filter: { cls: "card" }, count: { amount: "fixed", value: 1 } }) &&
    replace.effects.length === 1 && replace.effects[0].steps.length === 1;
  if (!battlefield && !opponentGraveyard) return;
  const [exile, life] = replace.effects[0].steps;
  if (exile?.effect !== "exile" || Object.keys(exile).length !== 2 ||
      !isDeepStrictEqual(exile.what, { ref: "replacement-event-object", role: "affected" }) ||
      replace.effects[0].steps.length > 2) return;
  if (life && (life.effect !== "gain-life" || life.who.player !== "you" || life.amount.amount !== "fixed" ||
      Object.keys(life).length !== 3 || replace.effects.length !== 1)) return;

  const followup = replace.effects[1]?.steps[0];
  if (replace.effects.length === 2 && (replace.effects[1].steps.length !== 1 ||
      followup?.effect !== "create-reflexive-trigger" || Object.keys(followup).length !== 3 ||
      !isDeepStrictEqual(followup.event, { event: "preceding-action", who: { player: "you" }, performed: true }))) return;
  return { event, exile, life, followup, opponentGraveyard, damageHistory };
}
export function hasExileInstruction(value: unknown): boolean {
  if (Array.isArray(value)) return value.some(hasExileInstruction);
  return isRecord(value) && (
    ["exile", "exile-until", "airbend"].includes(String(value.effect)) ||
    (value.effect === "scan-library" && value.operation === "exile") ||
    (value.effect === "move-zone" && isRecord(value.to) && value.to.zone === "exile") ||
    value.keyword === "hideaway" || Object.values(value).some(hasExileInstruction));
}
export function linkedExileOrigin(abilities: Ability[], siblings: string): LinkedExileOrigin | undefined {
  const possible = abilities.flatMap((ability, index) => hasExileInstruction(ability) ? [{ ability, index }] : []);
  if (possible.length !== 1) return;
  const [{ ability, index }] = possible;
  const soughtByOther = abilities.some(other => other !== ability && seeksLinkedExile(other, false));
  if (ability.kind === "chapter" && ability.effects.length === 1 && Object.keys(ability.effects[0]).length === 1 &&
      ability.effects[0].steps.length === 1 && ability.effects[0].steps[0].effect === "exile-until" && soughtByOther) {
    const step = ability.effects[0].steps[0];
    if (!step.optional && !step.forEach && !step.forEachVote && !step.decidedBy && step.what.length === 1 &&
        step.what[0].ref === "target" && objectCardinality(step.what[0]) === "one" &&
        isDeepStrictEqual(step.until, { event: "leaves-battlefield", what: { ref: "this", noun: "saga" } }))
      return { ability: `${siblings}/${index}`, path: `${siblings}/${index}/effects/0/steps/0`, imprint: false, rule: "607.2a" };
  }
  if (ability.kind === "static" && ability.effect.static === "replacement" &&
      exileReplacementParts(ability.effect.replacement) && abilities.some(other => other !== ability && seeksLinkedExile(other, false))) {
    return { ability: `${siblings}/${index}`, path: `${siblings}/${index}/effect/replacement/replace/effects/0/steps/0`,
      imprint: false, rule: "607.2b" };
  }
  if ((ability.kind !== "activated" && ability.kind !== "triggered") || !ability.effects.length) return;
  const [sentence, ...later] = ability.effects;
  if (later.length && ability.kind === "triggered" && !hasExileInstruction(later) &&
      (seeksLinkedExile(later, false) || soughtByOther) &&
      Object.keys(sentence).length === 1 && sentence.steps.length === 1 && sentence.steps[0].effect === "exile") {
    const exile = sentence.steps[0];
    if (!exile.optional && !exile.forEach && !exile.forEachVote && !exile.decidedBy && !exile.faceDown &&
        (exile.what.ref === "library-cards" || exile.what.ref === "trigger-event-object" ||
          (exile.what.ref === "it" && ability.trigger.trigger === "cast")))
      return { ability: `${siblings}/${index}`, path: `${siblings}/${index}/effects/0/steps/0`, imprint: false,
        rule: seeksLinkedExile(later, false) ? "607.1c" : "607.2a" };
  }
  if (later.length) return;
  if (sentence.steps.length !== 1 || sentence.condition || sentence.otherwise || sentence.afterConditional) return;
  const [exile] = sentence.steps;
  if (exile.effect !== "exile" || exile.forEach || exile.forEachVote || exile.decidedBy || exile.faceDown ||
      (exile.what.ref !== "target" && exile.what.ref !== "filter") || objectCardinality(exile.what) !== "one") return;
  const imprint = ability.abilityWord === "Imprint";
  if (!abilities.some(entry => entry !== ability && seeksLinkedExile(entry, imprint))) return;
  return { ability: `${siblings}/${index}`, path: `${siblings}/${index}/effects/0/steps/0`, imprint, rule: "607.2a" };
}
