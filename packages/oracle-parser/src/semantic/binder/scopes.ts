// The recursive AST walk, its scope barriers and branch contexts.
import { isDeepStrictEqual } from "node:util";
import type { Ability, Amount, CharacteristicRelation, Condition, Cost, Effect, GainedAbility, LookPermission, ObjectFilter, ObjectRef, PlayerDesignation, PlayerRef, Sentence, StaticEffect } from "../../ast.ts";
import type { Analyzer } from "./analyzer.ts";
import { chosenRange, type ChoiceReference } from "./choices.ts";
import { DESIGNATION_RULES, child, combine, impliesPermanent, unambiguousPluralRecipient, type Candidate, type Context } from "./context.ts";
import { SYMBOLIC_AMOUNTS, isKeywordInstance, isRecord, stackClasses, type StaticRule } from "./predicates.ts";

export function walkChildren(this: Analyzer, value: Record<string, unknown>, path: string, context: Context): void {
  for (const [key, entry] of Object.entries(value)) this.walk(entry, child(path, key), context);
}
export function walk(this: Analyzer, value: unknown, path: string, context: Context): void {
  if (Array.isArray(value)) { value.forEach((entry, index) => this.walk(entry, child(path, index), context)); return; }
  if (!isRecord(value)) return;
  if (value.recipient === "they") {
    const recipient = unambiguousPluralRecipient(context);
    if (recipient) {
      this.bound({ recipient: "they" }, path, recipient.id, ["608.2c", "611.2c"]);
    } else this.unresolved({ recipient: "they" }, path, "unsupported-reference",
      "The ability recipient's pronoun needs an antecedent and a player/object domain; parser ordering cannot establish either.", ["109.1", "113.1"]);
    return;
  }
  if (value.ref === "affected-object") {
    if (context.affectedMember && !context.barrier) {
      this.bound({ ref: "affected-object" }, path, context.affectedMember.id, ["611.3a", "613.6"]);
      return;
    }
    this.unresolved({ ref: "affected-object" }, path, "unsupported-reference", "This exclusion requires a bound per-affected-object scope.", ["611.3"]);
    return;
  }
  if (value.ref === "those-counters") {
    // Counters are not objects (CR 122.1); the counters an earlier clause named need that clause's binding.
    this.unresolved({ ref: "those-counters" }, path, "missing-antecedent", "These counters need the earlier clause of the same ability that named them.", ["122.1", "608.2c"]);
    return;
  }
  // "A copy of that creature", "protection from each of that creature's
  // colors": its copiable values and colors as it last existed (CR 608.2h, 707.2).
  if (typeof value.ref === "string" && !context.lastKnown && (path.endsWith("/copyOf") || /\/from\/\d+\/what$/.test(path))) {
    context.lastKnown = true;
    try { this.object(value as ObjectRef, path, context); } finally { context.lastKnown = false; }
    return;
  }
  if (typeof value.ref === "string") { this.object(value as ObjectRef, path, context); return; }
  if (!context.lastKnown && ((isRecord(value.of) && (value.player === "controller" || value.player === "owner" || value.amount === "attribute"))
      || (value.amount === "counter-count" && isRecord(value.on))
      || (value.amount === "attribute-sum" && isRecord(value.among) && value.among.ref === "that" && value.among.plural === true)
      // "Put its counters on target creature": the counters it had (CR 608.2h).
      || (path.endsWith("/counters") && isRecord(value.of))
      // "If it was attacking", "if it had a counter on it": as it last existed.
      || (typeof value.condition === "string" && value.time === "previous"))) {
    context.lastKnown = true;
    try { this.walk(value, path, context); } finally { context.lastKnown = false; }
    return;
  }
  if (typeof value.player === "string") { this.player(value as PlayerRef, path, context); return; }
  if (value.to === "any-target") {
    this.object({ ref: "any-target", ...(typeof value.selectionId === "string" ? { selectionId: value.selectionId } : {}) }, path, context);
    return;
  }
  if (isKeywordInstance(value)) { this.otherObjectKeyword(value, path, context); return; }
  if (value.amount === "x" && (typeof value.keyword === "string" || typeof value.sign === "number")) {
    this.walk({ amount: "x" }, child(path, "amount"), context);
  } else if (typeof value.amount === "string" && SYMBOLIC_AMOUNTS.has(value.amount)) {
    const expression = value as Extract<Amount, { amount: "x" | "that-number" | "that-much" | "replacement-event-quantity" }>;
    if (context.barrier) this.unresolved(expression, path, "unsupported-scope", context.barrier, ["107.3"]);
    else if (expression.amount === "x" && context.x) {
      const definition = context.announcementOperand
        ? this.result.nodes.find(node => node.id === context.x) : undefined;
      const amount = definition?.node === "defined-number" ? JSON.stringify(definition.definition) : "";
      if (definition?.node === "bounded-payment-amount" || amount.includes('"amount":"dice-result"') ||
          (amount.includes('"amount":"that-much"') && (context.quantities.length !== 1 ||
            !this.triggerQuantity(context.quantities[0].id))))
        this.unresolved(expression, path, "unsupported-number-binding",
          "An announced choice cannot use a number established during resolution.", ["601.2b", "601.2c", "603.3d", "608.2d", "700.2a"]);
      else this.bound(expression, path, context.x, ["107.3c"]);
    }
    else if (expression.amount === "x" && context.enteringKickerX)
      this.bound(expression, path, context.enteringKickerX, ["107.3m", "400.7d", "702.33e"]);
    else if (expression.amount === "x" && context.triggerX) this.bound(expression, path, context.triggerX, ["701.37c"]);
    else if (expression.amount === "x" && context.announcedX) this.bound(expression, path, context.announcedX, ["107.3a"]);
    else if (expression.amount === "x" && this.bindPrintedQuantity(expression, path, context)) return;
    else if (expression.amount === "that-much") {
      // "Discard all the cards in your hand, then draw that many cards"; "You
      // may pay that much life. If you do, draw that many cards": the
      // quantity of the instruction this one follows in its sentence, or of
      // the decision "if you do" refers to.
      // An instruction whose own amount is "that many" repeats the number it
      // refers to ("you draw that many cards and lose that much life"); it
      // is not a new antecedent.
      let candidates = context.quantities.some(candidate => !this.echoQuantities.has(candidate.id))
        ? context.quantities.filter(candidate => !this.echoQuantities.has(candidate.id)) : context.quantities;
      const instruction = (candidate: Candidate) => candidate.id.slice(this.prefix.length).replace(/#[^#]*$/, "");
      const sentence = /^(.*\/effects\/\d+)\//.exec(path)?.[1];
      if (candidates.length > 1) {
        const linked = context.linkedPath ? candidates.filter(candidate => instruction(candidate).startsWith(context.linkedPath!)) : [];
        const same = sentence ? candidates.filter(candidate => instruction(candidate).startsWith(`${sentence}/`) && path.startsWith(`${sentence}/`)) : [];
        if (linked.length === 1) candidates = linked;
        else if (same.length) candidates = [same[same.length - 1]];
      }
      if (context.announcementOperand && candidates.length === 1 && !this.triggerQuantity(candidates[0].id))
        this.unresolved(expression, path, "unsupported-number-binding",
          "An announced choice cannot use a quantity produced during resolution.", ["601.2b", "601.2c", "603.3d", "608.2d", "700.2a"]);
      else this.antecedent(expression, path, candidates, "unknown", ["608.2h", "608.2c"]);
    }
    else this.unresolved(expression, path, "unsupported-number-binding",
      "This number needs a rule-specific definition, choice, or capture; no default value is inferred.", ["107.3a", "107.3f", "107.3j"]);
    return;
  }
  if (typeof value.gained === "string") { this.gained(value as unknown as GainedAbility, path, context); return; }
  if (value.trigger === "phase-in-turn" || value.trigger === "phase-instance") {
    const barrier = "This delayed header needs its exact turn or phase occurrence linked, including future schedule changes and skipped occurrences.";
    this.diagnostic("unsupported-scope", path, barrier, ["500.7", "500.8", "603.7b", "614.10a"]);
    this.walkChildren(value, path, { ...context, barrier });
    return;
  }
  if (value.amount === "combat-history-count") {
    const barrier = "This cardinality requires a historical set of distinct declared attacker incarnations, including departed objects.";
    this.diagnostic("unsupported-scope", path, barrier, ["400.7", "508.4"]);
    this.walkChildren(value, path, { ...context, barrier });
    return;
  }
  if (value.amount === "cast-mana-value-sum" && (!context.spellAbility || context.sourceMoved)) {
    const barrier = "The excluded cast must be the resolving spell's own cast occurrence.";
    this.diagnostic("unsupported-scope", path, barrier, ["202.3e", "601.2i", "608.2i"]);
    this.walkChildren(value, path, { ...context, barrier });
    return;
  }
  if (value.static === "as-cascade") {
    this.cascade(value as Extract<StaticEffect, { static: "as-cascade" }>, path, context);
    return;
  }
  if (value.static === "skip-time" && !context.barrier && context.phase === "continuous" &&
      isDeepStrictEqual(value.who, { player: "you" }) && isDeepStrictEqual(value.what, { kind: "step", step: "draw" })) {
    const rules = ["504.1", "611.3", "614.10", "614.10a"];
    this.player({ player: "you" }, child(path, "who"), context);
    this.node({ id: this.id(path, "skip-draw-step"), path, node: "skip-draw-step", scope: context.ability, who: context.you, rules });
    this.engineFeature("phase-scheduling", path, "Replace the controller's draw step each time it would begin while the static ability applies.", rules);
    return;
  }
  if (value.static === "skip-time" || value.effect === "skip-time") {
    const barrier = "Skipping requires exact upcoming occurrences, replacement consumption, and future-turn scope bindings.";
    this.diagnostic("unsupported-scope", path, barrier, ["500.11", "614.10"]);
    this.walkChildren(value, path, { ...context, barrier });
    return;
  }
  if (value.static === "level-band") {
    const barrier = "A level symbol needs the source's level-counter range, battlefield evaluation, and granted-ability scopes bound.";
    this.diagnostic("unsupported-scope", path, barrier, ["711.2", "711.4", "711.6"]);
    this.walkChildren(value, path, { ...context, barrier });
    return;
  }
  if (value.static === "station-band") {
    const barrier = "A station symbol needs the source's charge-counter threshold, battlefield evaluation, printed power/toughness box, and granted-ability scopes bound.";
    this.diagnostic("unsupported-scope", path, barrier, ["721.2", "721.2b", "721.4"]);
    this.walkChildren(value, path, { ...context, barrier });
    return;
  }
  if (value.effect === "scan-library") {
    const barrier = "A sequential library scan needs its actor, complete processed pool, matching subset, and per-card iteration scope bound.";
    this.diagnostic("unsupported-scope", path, barrier, ["609.3", "701.13", "701.20"]);
    this.walkChildren(value, path, { ...context, barrier });
    return;
  }
  // Shields watching future damage events: the engine applies them; their
  // printed references bind as anywhere else.
  if (value.static === "redirect-damage" || value.effect === "redirect-damage")
    this.engineFeature("damage-prevention", path, "Redirection needs the engine's future damage events, source-choice eligibility, budget consumption, and the new recipient's legality.", ["609.7", "614.9", "615.7", "615.8"]);
  if (value.static === "prevent-damage" || value.effect === "prevent-damage" || value.choice === "damage-source"
    || value.amount === "prevention-event-quantity" || value.condition === "damage-prevented")
    this.engineFeature("damage-prevention", path, "Prevention needs the engine's future damage events, source-choice eligibility, shield consumption, and prevented quantities.", ["609.7", "615.5", "615.7", "615.8", "615.10", "615.12"]);
  if (value.amount === "prevention-event-quantity" && value.quantity === "prevented" && !context.barrier &&
      context.exactEventQuantity?.path === path) {
    this.bound({ amount: "prevention-event-quantity", quantity: "prevented" }, path, context.exactEventQuantity.id, ["615.5", "615.12"]);
    return;
  }
  if (value.static === "prevent-damage" &&
      (this.preventionPair((value as Extract<StaticEffect, { static: "prevent-damage" }>).prevention, child(path, "prevention"), context) ||
        this.preventionOutcome((value as Extract<StaticEffect, { static: "prevent-damage" }>).prevention, child(path, "prevention"), context)))
    return;
  if ((value.static === "prohibit" || value.static === "untap-during-step" || value.static === "untap-step-limit") &&
      this.staticRule(value as unknown as StaticRule, path, context)) return;
  if (value.static === "look-permission" && this.privateLibraryLook(value.permission as LookPermission, path, context)) return;
  if (value.static === "look-permission" || value.effect === "grant-look-permission"
    || value.static === "commander-eligibility" || value.static === "optional-untap"
    || value.static === "untap-during-step" || value.static === "untap-step-limit") {
    const barrier = "Persistent information queries, pregame commander eligibility, and turn-based untap choices require their own rule-specific binding scopes.";
    this.diagnostic("unsupported-scope", path, barrier, ["401.5", "502.3", "708.5", "903.3a", "113.6n"]);
    this.walkChildren(value, path, { ...context, barrier });
    return;
  }
  if (value.condition === "characteristic-relation" && !context.barrier) {
    const condition = value as Extract<Condition, { condition: "characteristic-relation" }>;
    const at = child(path, "what");
    this.walk(condition.what, at, { ...context, lastKnown: context.phase === "resolution" && !context.announcementOperand });
    const subject = this.at(at);
    if (subject?.domain === "object" && subject.cardinality === "one") {
      this.characteristicPredicate(condition.test, child(path, "test"), subject.id, context);
      return;
    }
    this.diagnostic("unsupported-scope", path, "The characteristic comparison requires one bound subject.", ["608.2h"]);
    this.walk(condition.test, child(path, "test"), { ...context, barrier: "The comparison has no bound subject." });
    return;
  }
  if (Array.isArray(value.characteristicRelations) && context.characteristicSubject && !context.barrier) {
    const { characteristicRelations, ...operand } = value;
    this.walk(operand, path, { ...context, characteristicSubject: undefined });
    for (const [index, relation] of characteristicRelations.entries())
      this.characteristicPredicate(relation as CharacteristicRelation, `${path}/characteristicRelations/${index}`,
        context.characteristicSubject, context);
    return;
  }
  if (value.characteristicRelations || value.condition === "characteristic-relation" || value.constraint === "shared-value") {
    const barrier = "Characteristic relations need subject, operand, quantified-set and common-value scopes bound before comparison.";
    this.diagnostic("unsupported-scope", path, barrier, ["201.2a", "201.2b", "201.2c", "608.2h"]);
    this.walkChildren(value, path, { ...context, barrier });
    return;
  }
  if (value.static === "entry-copy") {
    const barrier = "Entry-copy needs separate entrant, model choice, copied characteristic, and entry-exception scopes before entry.";
    this.diagnostic("unsupported-scope", path, barrier, ["614.12", "707.2", "707.5", "707.9"]);
    this.walkChildren(value, path, { ...context, barrier });
    return;
  }
  // A predefined token's abilities are fixed by CR 111.10 and name nothing
  // outside the token; entering attacking needs the token's own choices.
  if (value.attacking === true)
    this.diagnostic("unsupported-scope", path, "Entering-attacking choices require scopes belonging to the new token.", ["508.4"]);
  if (value.amount === "chosen")
    this.diagnostic("unsupported-number-binding", path, "This number needs a phase-specific choice identity and constraint scope.", ["107.3", "601.2b"]);
  if (value.amount === "dice-result" && value.measure === "result" &&
      isDeepStrictEqual(value.roll, { roll: "contextual" }) && !context.barrier && !context.uncertainDieOrigin &&
      !context.announcementOperand && context.phase === "resolution" &&
      context.dieResults?.length === 1) {
    this.bound({ amount: "dice-result", roll: { roll: "contextual" }, measure: "result" },
      path, context.dieResults[0].id, ["706.2", "706.3", "706.6"]);
    return;
  }
  if (value.amount === "dice-result" || value.condition === "dice-result")
    this.diagnostic("unsupported-scope", path, "A die result needs its exact roll occurrence, natural/final stage, retained-results set, and table/trigger scope bound.", ["706.2", "706.3", "706.6", "706.7"]);
  if (value.amount === "kicker-count" && this.bindPrintedQuantity(value as Extract<Amount, { amount: "kicker-count" }>, path, context)) return;
  if (value.amount === "kicker-count")
    this.diagnostic("unsupported-number-binding", path, "This count requires the linked originating spell's recorded kicker payments.", ["400.7d", "702.33"]);
  if (value.amount === "life-change")
    this.engineFeature("event-history", path, "How much life a player gained or lost this turn needs the engine's life-change history.", ["119.3", "608.2i"]);
  else if (value.amount === "event-quantity") {
    if (this.boundEventAggregate(value as Extract<Amount, { amount: "event-quantity" }>, path, context)) return;
    const barrier = "An event or history quantity needs its exact measured event(s), participants and period bound; no nearby number substitutes for it.";
    this.diagnostic("unsupported-scope", path, barrier, ["119.3", "120.3", "608.2h"]);
    this.walkChildren(value, path, { ...context, barrier });
    return;
  }
  // "For each evidence vote": the tally of the one vote held earlier in this ability.
  if (value.amount === "vote-count" && !(context.votes?.length === 1 && !context.barrier))
    this.diagnostic("unsupported-reference", path, "This vote count needs the one vote it tallies held earlier in the ability.", ["701.38"]);
  if (value.amount === "devotion") {
    const { who, colors } = value as Extract<Amount, { amount: "devotion" }>;
    const playerPath = child(path, "who");
    const colorPath = child(path, "colors");
    this.walk(who, playerPath, { ...context });
    this.walk(colors, colorPath, context);
    const player = this.at(playerPath);
    const rules = ["700.5", "700.5a"];
    if (!["you", "owner", "controller", "that-player"].includes(who.player) || player?.cardinality !== "one" ||
        (!Array.isArray(colors) && !this.at(colorPath))) {
      this.diagnostic("unsupported-number-binding", path, "Devotion requires the player and its color operands to be bound.", rules);
      return;
    }
    this.node({ id: this.id(path, "devotion"), path, node: "devotion",
      scope: context.ability, who: player.id, colors, rules });
    this.engineFeature("devotion", path,
      "Compute devotion after copy, control, and text-changing effects, including effects that modify devotion.", rules);
    return;
  }
  if (value.amount === "mana-spent") {
    if (this.bindPrintedQuantity(value as Extract<Amount, { amount: "mana-spent" }>, path, context)) return;
    const what = child(path, "what");
    if (!context.barrier && context.phase === "resolution" && context.triggerObject?.event === "cast" &&
        value.quantity === "total" && value.source === undefined && !context.uncertainStackOrigin &&
        isDeepStrictEqual(value.what, { ref: "that", filter: { cls: "spell" } }) &&
        [...context.objects, ...(context.departed ?? [])].every(candidate =>
          candidate.id === context.triggerObject?.candidate.id ||
          (candidate.id === context.source && !context.spellAbility) ||
          (candidate.description !== undefined && !stackClasses(candidate.description)))) {
      const rules = ["601.2f", "601.2h", "601.2i", "603.2c", "608.2i"];
      const occurrence = context.triggerObject.candidate.id;
      this.bound({ ref: "that", filter: { cls: "spell" } }, what, occurrence, rules);
      this.node({ id: this.id(path, "cast-mana-paid"), path, node: "cast-mana-paid",
        scope: context.ability, occurrence, measure: "total", rules });
      this.engineFeature("event-history", path, "Read the actual mana spent in the one recorded cast, including additional costs.", rules);
      return;
    }
    const barrier = "Mana payment quantities need the exact originating spell's payment event, including additional costs and actual colors spent.";
    this.diagnostic("unsupported-scope", path, barrier, ["400.7d", "601.2f", "601.2h", "609.4b"]);
    this.walkChildren(value, path, { ...context, barrier });
    return;
  }
  if (value.condition === "controls-subset" &&
      this.commonNameSubset(value as Extract<Condition, { condition: "controls-subset" }>, path, context)) return;
  if (value.condition === "controls-subset") {
    const barrier = "A controlled-subset existence predicate requires candidate, cardinality and pairwise characteristic scopes; it does not select or capture objects.";
    this.diagnostic("unsupported-scope", path, barrier, ["201.2b", "603.4", "608.2h"]);
    this.walkChildren(value, path, { ...context, barrier });
    return;
  }
  // Filtered-player counter predicates already use player()'s per-candidate
  // scope diagnostic; this covers standalone quantified conditions.
  if (value.condition === "player-counters") {
    if (!context.barrier && context.phase === "resolution" && isRecord(value.who) &&
        (value.who.player === "target-player" || value.who.player === "target-opponent") && !value.who.count) {
      this.walkChildren(value, path, context);
      return;
    }
    const barrier = "A counter predicate needs its quantified player's candidate scope; counters must not be summed across opponents.";
    this.diagnostic("unsupported-scope", path, barrier, ["122.1", "608.2h"]);
    this.walkChildren(value, path, { ...context, barrier });
    return;
  }
  if (value.choice === "chosen-type" || value.choice === "chosen-value") {
    this.chosenValue(value as ChoiceReference, path, context);
    return;
  }
  if ((value.condition === "you-do" || value.condition === "you-dont") && !context.linkedDecision) {
    this.diagnostic("unsupported-reference", path, "Payment/performance conditions need an explicit preceding-action link.", ["118.12"]);
  }
  if (value.condition === "searched-zone" && !this.searchedZoneLink(value, path, context))
    this.diagnostic("unsupported-reference", path, "This condition needs the specific search event, including a search that found no cards.", ["701.23b", "701.23d"]);
  if (value.condition === "put-action-history") {
    const barrier = "The named movement outcome needs its exact preceding instruction, actor, historical object incarnation, and actual destination; attempting an action or choosing to pay is insufficient.";
    this.diagnostic("unsupported-scope", path, barrier, ["118.12", "608.2c", "608.2i", "609.3"]);
    this.walkChildren(value, path, { ...context, barrier });
    return;
  }
  if (value.effect === "search" && isRecord(value.zones) &&
      (value.zones.selection !== "all" || Array.isArray(value.zones.of) && value.zones.of.length > 1) && !this.ownZonesSearch(value, context))
    this.diagnostic("unsupported-scope", path, "Zone choices and found-card selection across multiple searched zones need one search-event scope.", ["701.23"]);
  if (value.condition === "pays") {
    // CR 118.12a: "unless [player] pays": that player pays, not the
    // ability's controller.
    this.walk(value.who, child(path, "who"), context);
    const payer = this.at(child(path, "who"));
    const payment: Context = payer && payer.domain === "player" && payer.cardinality === "one"
      ? { ...context, you: payer.id, phase: "payment" }
      : { ...context, barrier: "The optional payment event must bind its payer separately from the resolving ability's controller." };
    if (payment.barrier) this.diagnostic("unsupported-scope", path, payment.barrier, ["118.12a"]);
    // "Unless its controller pays {1} for each creature you control": the
    // count is in the ability's own voice, so its "you" is the ability's
    // controller (CR 109.5); only the payment is the payer's.
    for (const [key, entry] of Object.entries(value)) if (key !== "who" && key !== "condition")
      this.walk(entry, child(path, key), key === "multiplier" && !payment.barrier ? { ...context, phase: "payment" } : payment);
    // "Counter target spell unless its controller pays {1}. That player discards a card."
    if (payer && payer.domain === "player" && payer.cardinality === "one" && payer.id !== context.you) context.players = combine(context.players, [payer]);
    return;
  }
  // Questions about the game's history ("if you attacked this turn", "if
  // this spell was cast from a graveyard", "creatures that attacked this
  // turn"): the engine keeps that record. The objects and players they name
  // bind as anywhere else; a permanent's ability can ask about the spell it
  // was (CR 400.7d).
  // What happened is asked of the object as it was (CR 608.2h): "each
  // creature it blocked this combat" after it died.
  const history = value.combatHistory || value.damageHistory || ["attacked", "combat-action-history", "was-cast", "zone-change-history",
    "damage-history", "mana-spent-to-cast", "entered-from", "player-action-history", "was-attacked"].includes(String(value.condition));
  if (history && !context.lastKnown) {
    context.lastKnown = true;
    try { this.walk(value, path, context); } finally { context.lastKnown = false; }
    return;
  }
  if (value.combatHistory)
    this.engineFeature("event-history", child(path, "combatHistory"), "Combat history needs the engine's record of each candidate's declared attacks in the stated window.", ["508.4", "400.7", "608.2h"]);
  if (value.damageHistory)
    this.engineFeature("event-history", child(path, "damageHistory"), "Damage history needs the engine's record of damage events in the stated window.", ["120.3", "120.7", "400.7", "608.2h"]);
  if (["attacked", "combat-action-history", "was-cast", "zone-change-history", "damage-history", "mana-spent-to-cast", "entered-from",
    "player-action-history", "life-change", "was-attacked", "ability-resolution-ordinal", "ability-activation-count",
    "alternative-cost-paid", "additional-cost-paid", "keyword-choice"].includes(String(value.condition)))
    this.engineFeature("event-history", path, "This condition asks about earlier events or how this object was cast; the engine keeps that record.", ["603.4", "607.2", "608.2i", "400.7d"]);
  // Conditions about an earlier instruction of this ability ("if you won
  // the clash", "if [a player] did this way") need that instruction.
  if (value.condition === "vote-result" && value.result === "more" && typeof value.choice === "string" &&
      !context.barrier && context.namedVote?.path === context.previousEffect && context.namedVote?.names.includes(value.choice)) {
    const rules = ["608.2c", "701.38"];
    this.node({ id: this.id(path, "vote-outcome"), path, node: "vote-outcome", scope: context.ability,
      event: context.namedVote.event, choice: value.choice, comparison: "strict-majority", rules });
    this.engineFeature("vote-outcome", path, "Count the actual votes, and require the named option to beat the other one.", rules);
    return;
  }
  if (["vote-result", "clash-result", "preceding-action"].includes(String(value.condition))) {
    const barrier = "This condition queries earlier events or a linked decision; the exact events, period, actor and source incarnation have not been bound.";
    this.diagnostic("unsupported-scope", path, barrier, ["603.4", "607.2", "608.2i"]);
    this.walkChildren(value, path, { ...context, barrier });
    return;
  }
  // State of the game or of players as a whole. Their player operands are
  // still walked, but the quantified comparison itself has no binding rule.
  const query = value.condition;
  if (!context.barrier && isRecord(value.who) && value.who.player === "you" &&
      (query === "designation" || query === "completed-dungeon" || query === "opponent-count" || query === "turn-of")) {
    const scoped = { ...context };
    this.walkChildren(value, path, scoped);
    const rules = query === "designation"
      ? [DESIGNATION_RULES[value.designation as PlayerDesignation], "109.5"]
      : query === "completed-dungeon" ? ["309.7", "608.2i"]
      : query === "opponent-count" ? ["102.2", "102.3", "800.4"] : ["102.1", "800.4j"];
    this.node({ id: this.id(path, "player-state"), path, node: "player-state-condition",
      scope: context.ability, who: context.you, query, rules });
    if (query === "completed-dungeon")
      this.engineFeature("event-history", path, "Whether the specified player has completed a dungeon this game.", rules);
    return;
  }
  if (["designation", "day-night", "completed-dungeon", "full-party", "turn-of", "player-comparison", "opponent-count"].includes(String(value.condition)))
    this.diagnostic("unsupported-scope", path, "Game and player designations, turn ownership and per-player comparisons need the participating player set and its quantifier bound.", ["102.1", "102.2", "102.3", "608.2h"]);
  if (value.static === "combat-rule" && this.attackPayment(value as unknown as Extract<StaticEffect, { static: "combat-rule" }>, path, context)) return;
  if (value.rule === "declaration-cost") {
    const barrier = "A cost to attack or block needs the declaring player, each affected creature's controller, and the per-declaration cost unit bound; its payer is not the ability's controller by default.";
    this.diagnostic("unsupported-scope", path, barrier, ["508.1h", "509.1d"]);
    this.walkChildren(value, path, { ...context, barrier });
    return;
  }
  if (value.trigger === "player-attacks") {
    const barrier = "Declared-attacker groups, attacked players, and per-attacker trigger participants have not been bound.";
    this.diagnostic("unsupported-scope", path, barrier, ["508.3c", "508.3d", "508.3e"]);
    this.walkChildren(value, path, { ...context, barrier });
    return;
  }
  if (value.conditionScope === "each-affected-object") {
    const barrier = "A continuously evaluated condition needs a distinct affected-object binding for each match.";
    this.diagnostic("unsupported-scope", path, barrier, ["611.3"]);
    this.walkChildren(value, path, { ...context, barrier });
    return;
  }
  if (value.valueScope === "each-affected-object") {
    if (this.dynamicStaticValue(value, path, context)) return;
    const barrier = "A dynamic characteristic value needs a distinct affected-object binding for each match.";
    this.diagnostic("unsupported-scope", path, barrier, ["611.3", "613.6"]);
    this.walkChildren(value, path, { ...context, barrier });
    return;
  }
  if (value.static === "prohibit" || value.effect === "prohibit" || value.static === "cast-limit" || value.effect === "cast-limit") {
    const barrier = "Dynamically prohibited actors, source-zone ownership, ability sources, and per-player cast-history totals need action-scope binding.";
    this.diagnostic("unsupported-scope", path, barrier, ["601.3", "602.1c", "611.2c", "615.12"]);
    this.walkChildren(value, path, { ...context, barrier });
    return;
  }
  if (value.effect === "grant-play-permission" && value.durationScope === "each-permitted-card" &&
      !context.barrier && !context.uncertainExileOrigin && context.exileResults?.length === 1 &&
      context.exileResults[0].result.cardinality === "one" &&
      isDeepStrictEqual(value.permission, {
        who: { player: "you" }, action: "play", what: { ref: "that", filter: { cls: "card" } },
      }) && isDeepStrictEqual(value.duration, { duration: "as-long-as", condition: {
        condition: "remains", what: { ref: "it" }, zone: { zone: "exile" },
      } })) {
    const exile = context.exileResults[0];
    const rules = ["400.7", "400.7j", "611.2a", ...(exile.faceDown ? ["406.3a"] : [])];
    this.walk(value.permission, child(path, "permission"), context);
    if (this.at(`${path}/permission/what`)?.id === exile.result.id) {
      this.walk(value.duration, child(path, "duration"), { ...context,
        boundOperand: { path: `${path}/duration/condition/what`, candidate: exile.result, rules } });
      this.node({ id: this.id(path, "exile-play-lifetime"), path, node: "exile-play-lifetime", scope: context.ability,
        exile: exile.result.id, player: context.you, duration: "while-that-object-is-exiled",
        ...(exile.faceDown ? { visibility: "turn-face-up-before-announcement-unless-cast-face-down" as const } : {}), rules });
      this.engineFeature("linked-exile-play", path, exile.faceDown
        ? "Keep the card face down until play. Turn that exact card face up immediately before announcement unless cast face down. Apply normal timing and costs."
        : "Allow the captured player to play the same exiled card under normal timing and costs while it remains in exile, even if the source leaves.", rules);
    } else {
      this.walk(value.duration, child(path, "duration"), context);
      this.diagnostic("unsupported-scope", path, "The permission and its duration must refer to the same exile result.", rules);
    }
    this.walk(value.decidedBy, child(path, "decidedBy"), context);
    return;
  }
  if (value.effect === "grant-play-permission" && isRecord(value.permission) && isRecord(value.permission.what) &&
      (value.permission.what.ref === "it" || isDeepStrictEqual(value.permission.what, { ref: "that", filter: { cls: "card" } }))) {
    this.walkChildren(value, path, context);
    const at = `${path}/permission/what`;
    const subject = this.at(at);
    if (subject?.description && impliesPermanent(subject.description))
      this.diagnostic("unsupported-reference", at, "Playing a card requires a card outside the battlefield.", ["305.1", "601.2a"]);
    if (subject && context.exileResults?.some(capture => capture.faceDown && capture.result.id === subject.id) &&
        !this.result.nodes.some(node => node.node === "exile-play-lifetime" && node.path === path && node.exile === subject.id)) {
      const rules = ["400.7j", "406.3a", "611.2a"];
      const duration = isRecord(value.duration) ? value.duration.duration : undefined;
      if (!context.uncertainExileOrigin && subject.cardinality === "one" &&
          isDeepStrictEqual({ ...value.permission, what: undefined }, { who: { player: "you" }, action: "play", what: undefined }) &&
          (duration === "end-of-turn" || duration === "this-turn" || duration === "your-next-end-step")) {
        this.node({ id: this.id(path, "exile-play-lifetime"), path, node: "exile-play-lifetime", scope: context.ability,
          exile: subject.id, player: context.you, duration: duration === "your-next-end-step" ? duration : "end-of-turn",
          visibility: "turn-face-up-before-announcement-unless-cast-face-down", rules });
        this.engineFeature("linked-exile-play", path, "Turn the permitted card face up before announcement unless cast face down.", rules);
      } else this.diagnostic("unsupported-scope", at, "Playing the face-down card requires an explicit visibility and permission lifetime binding.", rules);
    }
    return;
  }
  if ((value.action === "cast" || value.action === "play") && isRecord(value.who) && isRecord(value.what)
      && (isRecord(value.alternativeCost) || Array.isArray(value.additionalCosts))) {
    // The player who casts the spell pays its costs (CR 601.2f-h), whoever
    // controls the effect that lets them.
    this.walk(value.who, child(path, "who"), context);
    const caster = this.at(child(path, "who"));
    const payment: Context = caster?.domain === "player" && caster.cardinality === "one" ? { ...context, you: caster.id, phase: "payment" }
      : { ...context, barrier: "The casting player who pays these costs is not one bound player." };
    if (payment.barrier) this.diagnostic("unsupported-scope", path, payment.barrier, ["601.2f"]);
    for (const [key, entry] of Object.entries(value)) {
      if (key === "who") continue;
      this.walk(entry, child(path, key), key === "alternativeCost" || key === "additionalCosts" ? payment : context);
    }
    return;
  }
  if (value.static === "conditional" && isRecord(value.condition)) {
    // "As long as enchanted creature is black, it gets +1/+1": the
    // condition's subject is the effect's antecedent.
    const scoped = { ...context };
    const pronoun = (ref: unknown) => isRecord(ref) && (ref.ref === "it" || ref.ref === "them");
    // "Enchanted creature gets +1/+2 as long as it's white. Otherwise, it
    // gets -2/-1": the condition follows the subject it refers to. In a cost
    // change, that subject is the spell or ability whose cost changes ("Each
    // creature spell you cast costs {1} less if it has mutate").
    const effect = isRecord(value.effect) ? value.effect : undefined;
    const scope = effect && effect.static === "cost-modification" && isRecord(effect.scope) ? effect.scope : undefined;
    const subjectPath = effect && isRecord(effect.what) && !pronoun(effect.what) ? child(child(path, "effect"), "what")
      : scope && isRecord(scope.spells) ? child(child(child(path, "effect"), "scope"), "spells")
      : scope && isRecord(scope.abilities) ? child(child(child(path, "effect"), "scope"), "abilities") : undefined;
    if (pronoun(value.condition.what) && subjectPath) {
      this.walk(value.effect, child(path, "effect"), scoped);
      const subject = this.at(subjectPath);
      scoped.mentionedObject = true;
      if (subject?.domain === "object") scoped.objects = combine(scoped.objects, [subject]);
      for (const [key, entry] of Object.entries(value)) if (key !== "effect") this.walk(entry, child(path, key), scoped);
      return;
    }
    this.namingCondition(value.condition, child(path, "condition"), scoped);
    for (const [key, entry] of Object.entries(value)) if (key !== "condition") this.walk(entry, child(path, key), scoped);
    return;
  }
  if (["entry-choice", "enters-with-counters", "enters-tapped", "as-enters"].includes(String(value.static))) { this.entering(value, path, context); return; }
  if (isRecord(value.entry) && (value.entry.attachedTo !== undefined || value.entry.attacking !== undefined)) {
    const barrier = "The entering permanent's pre-entry controller, payment, attachment, and attack-choice scopes have not been bound.";
    this.diagnostic("unsupported-scope", path, barrier, ["614.12a", "303.4f", "508.4"]);
    this.walkChildren(value, path, { ...context, barrier });
    return;
  }
  if (value.condition === "cast-count" && context.spellAbility && !context.sourceMoved && !context.barrier &&
      isRecord(value.who) && value.who.player === "you" && isRecord(value.what) && value.what.cls === "spell" &&
      value.what.other === true && value.during === "this-game" && isRecord(value.what.name) &&
      value.what.name.name === "literal") {
    const filter = child(path, "what");
    this.walkChildren(value, path, { ...context, castHistoryExclusionPath: filter });
    const rules = ["601.2i", "608.2i", "707.10"];
    this.node({ id: this.id(path, "cast-history-exclusion"), path, node: "cast-history-exclusion",
      scope: context.ability, caster: context.you, occurrence: context.source, rules });
    this.engineFeature("event-history", path, "Count matching cast occurrences other than this resolving spell's own cast.", rules);
    return;
  }
  if (value.other === true && !value.otherThan) {
    // "Other creatures you control", "another creature enters": in a
    // permanent's ability, before any other object is named, the excluded
    // object is the permanent itself. Elsewhere (a spell, or after a
    // trigger or instruction names an object) it may be that object.
    if (!context.barrier && context.castHistoryExclusionPath === path) {
      this.bound({ ref: "this", noun: "spell" }, child(path, "other"), context.source, ["601.2i", "608.2i"]);
    } else if (!context.barrier && context.comparisonExclusion && path.startsWith(context.comparisonExclusion.path) &&
        /^(?:\/anyOf\/\d+)*$/.test(path.slice(context.comparisonExclusion.path.length))) {
      this.bound({ ref: "affected-object" }, child(path, "other"), context.comparisonExclusion.subject, ["201.2a", "608.2b"]);
    } else if (!context.barrier && context.graveyardExclusion?.filterPath === path) {
      this.bound({ ref: "that", filter: { cls: "card" } }, child(path, "other"), context.graveyardExclusion.view,
        ["400.7e", "601.2c", "603.6c", "608.2b"]);
    } else if (!context.barrier && context.copyModelExclusion?.path === path) {
      this.bound(context.copyModelExclusion.reference, child(path, "other"), context.copyModelExclusion.model.id, ["611.2c", "707.2"]);
    } else if (!context.barrier && context.phase === "payment" && path === context.sourceExclusionPath &&
        path.includes("/keywords/") && value.cls === "card") {
      this.bound({ ref: "this", noun: "card" }, child(path, "other"), context.source, ["201.5", "601.2a", "702.138a"]);
    } else if (context.permanentSource && !context.barrier && !context.sourceMoved &&
        (path === context.sourceExclusionPath || (!context.mentionedObject && this.onlySource(context))))
      this.bound({ ref: "self" }, child(path, "other"), context.source,
        path !== context.sourceExclusionPath ? ["201.5"]
          : context.phase === "replacement-match" ? ["201.5", "615.1a"]
            : context.phase === "payment" ? ["201.5", "602.2b", "701.21a"] : ["201.5", "608.2h"]);
    else if (!bindOther(this, value, path, context))
      this.diagnostic("unsupported-reference", child(path, "other"), "The excluded antecedent of other/another is unresolved.", ["201.5"]);
  }
  // CR 702.33e: "kicked" refers to the kicker ability printed on the object;
  // with one kicker ability (or the kicker cost it names) that link is fixed.
  const kickers = value.condition === "kicked" ? (this.kickers.get(context.face) ?? []) : [];
  const kicker = value.condition === "kicked" && !context.barrier
    ? (isRecord(value) && Array.isArray(value.kickerCost)
      ? kickers.filter(entry => entry.costs.includes(JSON.stringify(value.kickerCost)))
      : kickers.length === 1 ? kickers : []) : [];
  if (kicker.length === 1) {
    // "If it was kicked": the object with the kicker ability (CR 702.33e).
    if (isRecord(value.what) && value.what.ref === "it" && !context.sourceMoved)
      this.roleAntecedents.set(child(path, "what"), { id: context.source, cardinality: "one", domain: "object" });
    this.node({ id: this.id(path, "linked-ability"), path, node: "linked-ability", scope: context.ability, condition: "kicked", ability: kicker[0].id, rules: ["702.33e", "607.2"] });
    // The kicker cost it names was paid by the player who cast the spell.
    if (isRecord(value) && Array.isArray(value.kickerCost)) {
      for (const [key, entry] of Object.entries(value)) this.walk(entry, child(path, key), key === "kickerCost" ? { ...context, you: kicker[0].you, phase: "payment" } : context);
      return;
    }
  }
  else if (value.relation === "linked-exile" && this.linkedExileFilter(value, path, context)) return;
  else if (value.relation === "event-result" && value.scope === "this-way" && this.thisWay(value, path, context)) { /* linked */ }
  else if (value.relation === "event-result" || value.relation === "linked-exile" || value.condition === "kicked")
    this.diagnostic("unsupported-reference", path, "This reference requires a specific event/payment or linked-ability association.", ["607"]);
  if (value.static === "alternative-cost" || value.static === "play-permission") {
    // CR 109.5: "you" on an object is its would-be controller while a player
    // is casting it, or its owner where it has no controller. A card's own
    // alternative cost ("rather than pay this spell's mana cost") is the
    // caster's; its own permission ("You may cast this card from your
    // graveyard") its owner's. A permanent's permission for other cards
    // ("You may play lands from your graveyard") is its controller's.
    const subject = value.static === "alternative-cost" ? value.spells : isRecord(value.permission) ? value.permission.what : undefined;
    const own = isRecord(subject) && (subject.ref === "self" || subject.ref === "this");
    if (own) {
      const role = value.static === "alternative-cost" ? "prospective-caster" as const : "card-owner" as const;
      const player = this.node({ id: this.id(path, role), path, node: "runtime-player", scope: context.ability, role, rules: ["109.5"] });
      this.walkChildren(value, path, { ...context, you: player, ...(role === "prospective-caster" ? { phase: "payment" as const } : {}) });
      return;
    }
    if (context.permanentSource) { this.walkChildren(value, path, context); return; }
  }
  if (["alternative-cost", "replacement", "play-permission", "play-permission-choice"].includes(String(value.static))) {
    const barrier = "This pre-entry or alternative-payment scope needs a would-be controller/caster binding.";
    this.diagnostic("unsupported-scope", path, barrier, ["109.5", "614.12"]);
    // CR 201.5, 614.1: the watched move precedes the replacement. Its
    // explicit source is known even while replacement-event captures are not.
    const event = value.static === "replacement" && isRecord(value.replacement) ? value.replacement.event : undefined;
    const replacementSubjectPath = !context.barrier && isRecord(event) && event.event === "move-zone"
      ? child(child(child(path, "replacement"), "event"), "what") : undefined;
    this.walkChildren(value, path, { ...context, barrier, replacementSubjectPath });
    return;
  }
  if (["move-zone", "put-counters", "transform", "convert", "attach", "unattach", "create-token"].includes(String(value.effect)) && value.who == null)
    this.player({ player: "you" }, child(path, "who"), context);
  if (value.cost === "tap-self" || value.cost === "untap-self")
    this.object({ ref: "self" }, child(path, "source"), context);
  if (typeof value.cost === "string") {
    this.player({ player: "you" }, child(path, "payer"), context);
    if (value.cost === "choice" && this.simplePaymentAlternatives(value as Extract<Cost, { cost: "choice" }>, path, context)) return;
    if (value.cost === "blight" && !context.barrier && context.phase === "payment" && isRecord(value.amount) &&
        value.amount.amount === "fixed" && typeof value.amount.value === "number") {
      const rules = ["118.11", "601.2h", "602.2b", "701.68a", "701.68b"];
      this.node({ id: this.id(path, "blight-payment"), path, node: "blight-payment", scope: context.ability,
        payer: context.you, count: value.amount.value, rules });
      context.mentionedObject = true;
      this.engineFeature("intrinsic-payment", path, "The payer chooses a creature they control for the blight payment.", rules);
    } else if (["choice", "collect-evidence", "waterbend", "forage", "blight"].includes(value.cost))
      this.diagnostic("unsupported-scope", path, "This cost has intrinsic payment choices and object selections requiring a distinct payment scope.", ["118", "701"]);
    if (value.cost === "mill" || (value.cost === "discard" && value.what === "hand"))
      this.player({ player: "you" }, child(path, "zone-owner"), context);
  }
  if (value.restriction === "timing" && value.turn != null) {
    if (value.turn === "your" || value.turn === "activator") this.player({ player: "you" }, child(path, "turn"), context);
    else this.diagnostic("unsupported-reference", child(path, "turn"), "This turn-owner relation needs its own scoped binding.", ["109.5"]);
  }
  if (value.activators === "opponents-of-source-controller" || value.activators === "any-player") {
    if (!context.barrier && context.phase === "payment" && context.permanentSource) {
      const rules = ["109.5", "602.2"];
      this.node({ id: this.id(path, "activation-permission"), path, node: "activation-permission", scope: context.ability,
        source: context.source, actors: value.activators, rules });
      this.engineFeature("activation-permission", path, "Check the printed eligible player set before the actual activator pays the cost.", rules);
    } else this.diagnostic("unsupported-reference", path, "Activation permission requires a player set distinct from the ability's eventual activator.", ["602.2"]);
  }
  if (value.trigger === "you-attack") this.player({ player: "you" }, child(path, "who"), context);
  if (value.condition === "cast-count" && value.who === "all-players-combined")
    this.diagnostic("unsupported-reference", child(path, "who"), "This aggregate requires the complete participating-player set.", ["109.5"]);
  if (value.condition === "your-turn" || value.condition === "not-your-turn" ||
      ["your-next-turn", "end-of-your-next-turn", "your-next-end-step", "your-next-upkeep"].includes(String(value.duration)))
    this.player({ player: "you" }, child(path, "whose"), context);
  if (Array.isArray(value.symbols)) value.symbols.forEach((symbol, index) => {
    if (symbol === "X") this.walk({ amount: "x" }, child(child(path, "symbols"), index), context);
  });
  // Filters have implicit player references as well as explicit PlayerRef
  // nodes. Record them instead of silently treating these strings as data.
  for (const key of ["control", "own", "owner", "whose", "during"] as const) {
    const token = value[key];
    if (token === "you" || token === "your") this.player({ player: "you" }, child(path, key), context);
    else if (token === "that-player" || token === "their") this.player({ player: token === "their" ? "they" : "that-player" }, child(path, key), context);
    // "An opponent controls", "your opponents' graveyards": relative to you.
    else if (token === "opponent" || token === "each-opponent" || token === "an-opponent") this.relativePlayers("opponents", child(path, key), context);
    // "You don't control": every other player, who in a team game need not be an opponent.
    else if (token === "not-you") this.relativePlayers("other-players", child(path, key), context);
    // "Its owner's", "each player's", "a graveyard" describe each candidate
    // itself or every player; they name no player outside it.
    else if (token === "its-owner" || token === "owner" || token === "each-player" || token === "any") { /* no outside reference */ }
    else if (token === "active-player") {
      this.diagnostic("unsupported-reference", child(path, key), "This implicit player relation requires its own scoped binding rule.", ["109.5"]);
    }
  }
  if (value.effect === "become" && isRecord(value.spec) && Object.keys(value.spec).length === 1 &&
      isRecord(value.spec.copyOf) && isRecord(value.what) && value.what.ref === "each" &&
      isRecord(value.what.filter) && value.what.filter.other === true && !context.barrier) {
    const modelPath = `${path}/spec/copyOf`;
    this.walk(value.spec.copyOf, modelPath, context);
    const model = this.at(modelPath);
    if (model?.domain === "object" && model.cardinality === "one") {
      const scoped = { ...context, copyModelExclusion: { path: `${path}/what/filter`, model,
        reference: value.spec.copyOf as ObjectRef } };
      this.walk(value.what, child(path, "what"), scoped);
      this.walk(value.duration, child(path, "duration"), scoped);
      this.engineFeature("characteristic-layers", path, "Set the affected group at resolution, excluding the bound copy model, and apply its copiable values.", ["611.2c", "707.2"]);
      return;
    }
  }
  this.walkWithRecipient(value, path, context);
  if (["sacrifice", "discard", "exile", "return"].includes(String(value.cost)) &&
      this.at(child(path, "what"))?.id === context.source) context.sourceMoved = true;
  if (value.controller === "you") this.player({ player: "you" }, child(path, "controller"), context);
  else if ((value.controller === "owner" || value.to === "owner") && isRecord(value.what)) {
    const input = this.at(child(path, "what"));
    const at = child(path, value.controller === "owner" ? "controller" : "to");
    const expression: PlayerRef = { player: "owner", of: { ref: "it" } };
    if (context.barrier || !input) this.unresolved(expression, at, "missing-antecedent",
      "The moved object's owner requires a bound object.", ["108.3"]);
    else {
      const owner = this.node({ id: this.id(at, "owner"), path: at, node: "derived-player", scope: context.ability,
        role: "owner", of: input.id, rules: ["108.3"] });
      this.bound(expression, at, owner, ["108.3"]);
      // "Return target permanent to its owner's hand. Then that player discards a card."
      if (input.cardinality === "one" && context.phase === "resolution") {
        this.weakPlayers.add(owner);
        context.players = combine(context.players, [{ id: owner, cardinality: "one", domain: "player" }]);
      }
    }
  } else if (value.owner === "each-affected-objects-controller" && path.endsWith("/step") && !context.barrier
      && this.at(`${path.slice(0, -"/step".length)}/what`)?.domain === "object") {
    // "Doesn't untap during its controller's untap step": the controller of
    // each object the effect applies to (CR 108.4, 502.3).
    const affected = this.at(`${path.slice(0, -"/step".length)}/what`)!;
    const at = child(path, "owner");
    const controller = this.node({ id: this.id(at, "controller"), path: at, node: "derived-player", scope: context.ability,
      role: "controller", of: affected.id, rules: ["108.4", "502.3"] });
    this.bound({ player: "controller", of: { ref: "it" } }, at, controller, ["108.4", "502.3"]);
  } else if (value.battlefieldController === "owner" && path.endsWith("/return") && !context.barrier
      && this.exiledUntil(path.slice(0, -"/return".length))) {
    // "Exile ... until this creature leaves the battlefield": each card
    // returns under its owner's control (CR 610.3c).
    const exiled = this.exiledUntil(path.slice(0, -"/return".length))!;
    const at = child(path, "battlefieldController");
    const owner = this.node({ id: this.id(at, "owner"), path: at, node: "derived-player", scope: context.ability,
      role: "owner", of: exiled.id, rules: ["108.3", "610.3c"] });
    this.bound({ player: "owner", of: { ref: "it" } }, at, owner, ["108.3", "610.3c"]);
  } else if (value.battlefieldController === "owner" || value.owner === "each-affected-objects-controller") {
    this.diagnostic("unsupported-reference", path, "This ownership rule requires a bound affected-object scope.", ["109.5"]);
  }
}
const YOUR_GRAVEYARD = { zone: "graveyard", owner: "your" } as const;
const ONE = { amount: "fixed", value: 1 } as const;

function onlyFields(value: object, ...keys: string[]): boolean {
  return Object.entries(value).every(([key, field]) => field === undefined || keys.includes(key));
}

function bindOther(analyzer: Analyzer, filter: Record<string, unknown>, path: string, context: Context): boolean {
  const root = context.lexicalAbilityPath;
  if (context.barrier || !root || context.source !== analyzer.id("/source", "object") ||
      context.ability !== analyzer.id(root, "ability")) return false;
  // Only printed face abilities are registered here. A quote or a created
  // ability cannot supply either end of the exclusion (CR 113.7a, 113.10).
  const ability = analyzer.abilityAsts.get(root) as Ability | undefined;
  const at = child(path, "other");
  if (context.phase === "payment" && ability?.kind === "activated" && context.permanentSource &&
      !context.sourceMoved && sourcePaymentFilter(ability.costs, path, root) === filter) {
    analyzer.bound({ ref: "self" }, at, context.source, ["201.5", "602.2b", "601.2h"]);
    return true;
  }
  if (context.phase !== "resolution") return false;
  if (ability?.kind === "triggered" && path === `${root}/effects/0/steps/0/what/filter` &&
      ownDiesReturnFilter(ability) === filter) {
    // This is the same graveyard exclusion as the existing view, with one
    // definite death event and its already-exported successor (CR 400.7e,
    // 603.6c). Excluding the battlefield incarnation would be ineffective.
    const successor = context.sourceSuccessor;
    if (successor?.id === analyzer.id(`${root}/trigger`, "successor") && successor.cardinality === "one" &&
        successor.triggerSuccessor && context.objects.length === 1 && context.objects[0].id === successor.id) {
      analyzer.bound({ ref: "that", filter: { cls: "card" } }, at, successor.id, ["400.7e", "603.3d", "603.6c", "608.2b"]);
      return true;
    }
  }
  if (ability?.kind !== "spell") return false;
  const origin = spellExclusionOrigin(ability.effects, root, path, filter);
  if (!origin) return false;
  const [originPath, target] = origin;
  const previous = analyzer.at(originPath);
  const node = analyzer.result.nodes.find(entry => entry.id === previous?.id);
  if (node?.node !== "selection" || node.selection !== "target" || node.scope !== context.ability ||
      node.path !== originPath || node.expression !== target || node.cardinality !== "one" || node.perMember) return false;
  analyzer.bound(target, at, node.id, ["115.3", "601.2c", "608.2b", "608.2c"]);
  return true;
}

function sourcePaymentFilter(costs: Cost[], path: string, root: string): ObjectFilter | undefined {
  // One object payment alongside only mana or the source's own tap symbol.
  // In particular, a second object payment cannot suggest an exclusion.
  const payments = costs.flatMap((cost, index) =>
    (cost.cost === "mana" && onlyFields(cost, "cost", "symbols")) || (cost.cost === "tap-self" && onlyFields(cost, "cost"))
      ? [] : [{ cost, path: `${root}/costs/${index}` }]);
  if (payments.length !== 1) return;
  const [{ cost, path: payment }] = payments;
  if (cost.cost === "tap-objects" && path === `${payment}/what/filter` &&
      cost.what.ref === "filter" && cost.what.count.amount === "fixed" && onlyFields(cost.what, "ref", "filter", "count")) {
    const { types, subtypes, ...rest } = cost.what.filter;
    if ((types?.length === 1 && !subtypes || subtypes?.length === 1 && !types) &&
        isDeepStrictEqual(rest, { other: true, status: ["untapped"], control: "you" })) return cost.what.filter;
  }
  if (cost.cost === "exile" && path === `${payment}/what/filter` && onlyFields(cost, "cost", "what") &&
      cost.what.ref === "filter" && cost.what.count.amount === "fixed" && onlyFields(cost.what, "ref", "filter", "count")) {
    const { types, ...rest } = cost.what.filter;
    if ((types === undefined || isDeepStrictEqual(types, ["creature"])) &&
        isDeepStrictEqual(rest, { other: true, cls: "card", zone: YOUR_GRAVEYARD })) return cost.what.filter;
  }
  if (cost.cost === "remove-counters" && cost.count !== "all" && cost.count.amount === "fixed" &&
      path === `${payment}/from/filter`) {
    const from = cost.from;
    if (from.ref === "filter" && cost.count.value === 1 && cost.distributed === undefined &&
        isDeepStrictEqual(from, { ref: "filter", count: ONE, filter: { cls: "permanent", control: "you", other: true } }))
      return from.filter;
    if (from.ref === "each" && cost.distributed && isDeepStrictEqual(from, { ref: "each", filter: {
      other: true, allOf: [{ anyOf: [{ types: ["artifact"] }, { types: ["creature"] }, { types: ["planeswalker"] }] }], control: "you",
    } })) return from.filter;
  }
}

type Target = Extract<ObjectRef, { ref: "target" }>;
function singleTarget(ref: ObjectRef): ref is Target {
  return ref.ref === "target" && !!ref.selectionId && isDeepStrictEqual(ref.count, ONE) &&
    onlyFields(ref, "ref", "selectionId", "count", "filter");
}

function ownDiesReturnFilter(ability: Extract<Ability, { kind: "triggered" }>): ObjectFilter | undefined {
  const sentence = ability.effects[0];
  const step = sentence?.steps[0];
  if (!onlyFields(ability, "kind", "abilityWord", "trigger", "effects") ||
      !(isDeepStrictEqual(ability.trigger, { trigger: "dies", what: { ref: "this", noun: "creature" } }) ||
        isDeepStrictEqual(ability.trigger, { trigger: "dies", what: { ref: "self" } })) ||
      !sentence || !onlyFields(sentence, "steps") || sentence.steps.length !== 1 ||
      step?.effect !== "return-to-hand" || step.to !== "your" || !onlyFields(step, "effect", "what", "to") ||
      !singleTarget(step.what)) return;
  const { types, subtypes, ...filter } = step.what.filter;
  if ((types?.length === 1 && !subtypes || subtypes?.length === 1 && !types) &&
      isDeepStrictEqual(filter, { cls: "card", zone: YOUR_GRAVEYARD, other: true })) return step.what.filter;
}

function simpleSpellSubject(step: Effect | undefined): ObjectRef | undefined {
  if (!step) return;
  switch (step.effect) {
    case "declare-target": case "destroy":
      if ("ref" in step.what && onlyFields(step, "effect", "what")) return step.what;
      break;
    case "return-to-hand":
      if (onlyFields(step, "effect", "what", "to") && (step.to === "your" || step.to === "owner")) return step.what;
      break;
    case "pump":
      if (onlyFields(step, "effect", "what", "power", "toughness", "duration") &&
          step.power.amount.amount === "fixed" && step.toughness.amount.amount === "fixed" &&
          isDeepStrictEqual(step.duration, { duration: "end-of-turn" })) return step.what;
      break;
    case "put-counters":
      if ("ref" in step.on && onlyFields(step, "effect", "on", "counter", "count") &&
          step.count.amount === "fixed" && onlyFields(step.counter, "type")) return step.on;
      break;
  }
}

function spellExclusionOrigin(sentences: Sentence[], root: string, path: string,
  filter: Record<string, unknown>): [string, Target] | undefined {
  const location = /^\/effects\/(\d+)\/steps\/(\d+)\/(.+)\/filter$/.exec(path.slice(root.length));
  if (!path.startsWith(`${root}/`) || !location) return;
  const [, sentenceIndex, stepIndex, operand] = location;
  const si = Number(sentenceIndex), ti = Number(stepIndex);
  const sentence = sentences[si];
  const step = sentence?.steps[ti];
  if (!sentence || !step) return;
  const plain = onlyFields(sentence, "steps");
  const kicked = onlyFields(sentence, "steps", "condition", "conditionKind") && sentence.conditionKind === "if" &&
    isDeepStrictEqual(sentence.condition, { condition: "kicked", what: { ref: "this", noun: "spell" },
      comparison: { op: "ge", value: ONE } });
  const stepPath = `${root}/effects/${si}/steps/${ti}`;

  // An explicit pair of operands of the same instruction. For damage the
  // power pronoun is the source's characteristic, not a second participant.
  if (si === 0 && ti === 0 && plain) {
    let pair: [string, ObjectRef, ObjectRef] | undefined;
    if (step.effect === "fight" && operand === "b" && onlyFields(step, "effect", "a", "b")) pair = ["a", step.a, step.b];
    if (step.effect === "move-counters" && operand === "to" && onlyFields(step, "effect", "counter", "count", "from", "to") &&
        (step.count === "all" || step.count.amount === "fixed")) pair = ["from", step.from, step.to];
    if (step.effect === "damage" && onlyFields(step, "effect", "source", "amount", "to") && !Array.isArray(step.source) &&
        isDeepStrictEqual(step.amount, { amount: "attribute", attribute: "power", of: { ref: "it" } })) {
      if (operand === "to/ref" && step.to.to === "object") pair = ["source", step.source, step.to.ref];
      if (operand === "to/refs/0" && step.to.to === "each" && step.to.refs.length === 2 &&
          isDeepStrictEqual(step.to.refs[1], { player: "each-opponent" }) && "ref" in step.to.refs[0])
        pair = ["source", step.source, step.to.refs[0]];
    }
    if (pair && singleTarget(pair[1]) && excludesSpellTarget(pair[2], filter, pair[1]))
      return [child(stepPath, pair[0]), pair[1]];
  }

  // The initial instruction is the sole earlier object operand. A fixed life
  // gain in between reads no objects. Kicker only gates the second target.
  const first = sentences[0];
  const producer = first?.steps[0];
  const prior = simpleSpellSubject(producer);
  if (!first || !producer || !prior || !singleTarget(prior) || !onlyFields(first, "steps") ||
      (operand !== "what" && operand !== "on")) return;
  const immediatelyAfter = (si === 0 && ti === 1) || (si === 1 && ti === 0 && first.steps.length === 1);
  const middle = sentences[1];
  const life = middle?.steps[0];
  const afterLife = si === 2 && ti === 0 && first.steps.length === 1 && middle?.steps.length === 1 &&
    onlyFields(middle, "steps") && life?.effect === "gain-life" && onlyFields(life, "effect", "who", "amount") &&
    isDeepStrictEqual(life.who, { player: "you" }) && life.amount.amount === "fixed";
  const current = simpleSpellSubject(step);
  if ((plain || kicked) && (immediatelyAfter || afterLife) && current &&
      operand === (step.effect === "put-counters" ? "on" : "what") && excludesSpellTarget(current, filter, prior))
    return [`${root}/effects/0/steps/0/${producer.effect === "put-counters" ? "on" : "what"}`, prior];
}

function excludesSpellTarget(ref: ObjectRef, filter: Record<string, unknown>, prior: Target): boolean {
  if ((ref.ref !== "target" && ref.ref !== "each") || ref.filter !== filter) return false;
  if (ref.ref === "target" && (!ref.selectionId || ref.selectionId === prior.selectionId || !isDeepStrictEqual(ref.count, ONE) ||
      !onlyFields(ref, "ref", "filter", "selectionId", "count", "upTo"))) return false;
  if (ref.ref === "each" && !onlyFields(ref, "ref", "filter")) return false;
  const creatures = [{ types: ["creature"] }, { types: ["creature"], control: "you" }];
  if (creatures.some(description => isDeepStrictEqual(prior.filter, description)) &&
      creatures.some(description => isDeepStrictEqual(filter, { ...description, other: true }))) return true;
  const lands = [{ types: ["land"] }, { nonTypes: ["land"], cls: "permanent" }];
  if (lands.some(description => isDeepStrictEqual(prior.filter, description) &&
      isDeepStrictEqual(filter, { ...description, other: true }))) return true;
  return ref.ref === "target" &&
    isDeepStrictEqual(prior.filter, { types: ["creature"], cls: "card", zone: YOUR_GRAVEYARD }) &&
    isDeepStrictEqual(filter, { cls: "card", permanent: true, zone: YOUR_GRAVEYARD, other: true });
}

/** Walk a node's children, binding `what` (or a player recipient) before
 * the abilities it is given, so a granted ability knows its recipient. */
export function walkWithRecipient(this: Analyzer, value: Record<string, unknown>, path: string, context: Context): void {
  // An instruction's subject (its acting player, its damage source) comes
  // first and is an antecedent for the rest of the same instruction:
  // "Target creature's owner puts it on top", "Target player shuffles their
  // graveyard", "deals damage equal to its power". A damage source that has
  // left its zone acts with its last-known information (CR 113.7a).
  const instruction = typeof value.effect === "string";
  const recipientKey = isRecord(value.what) ? "what" : value.static === "player-abilities" && isRecord(value.who) ? "who" : undefined;
  const first = [...(instruction ? ["who", "source"].filter(key => isRecord(value[key])) : []), ...(recipientKey ? [recipientKey] : [])]
    .filter((key, index, keys) => keys.indexOf(key) === index);
  if (!first.length && value.effect !== "create-token") { this.walkChildren(value, path, context); return; }
  const saved = { recipient: context.recipient, objects: context.objects, actor: context.actor, subjects: context.subjects, named: context.named };
  const subjects: Candidate[] = [];
  for (const key of first) {
    if (key === "source") {
      const lastKnown = context.lastKnown;
      context.lastKnown = true;
      this.walk(value[key], child(path, key), context);
      context.lastKnown = lastKnown;
    } else this.walk(value[key], child(path, key), context);
    if (!instruction) continue;
    const candidate = this.at(child(path, key));
    // "They" never means "you", which the text calls "you".
    if (key === "who" && candidate?.domain === "player" && candidate.id !== context.you) context.actor = candidate;
    if (key === "who" && isRecord(value.who) && isRecord(value.who.of)) {
      const possessor = this.at(child(child(path, "who"), "of"));
      if (possessor?.domain === "object") subjects.push(possessor);
    }
    if (key === "source" && candidate?.domain === "object") subjects.push(candidate);
    if (subjects.length) {
      context.objects = combine(context.objects, subjects);
      context.subjects = [...subjects];
    }
  }
  if (value.effect === "create-token") context.recipient = { created: true };
  else if (value.effect === "get-emblem") {
    const player = this.at(child(path, "who"));
    if (!context.barrier && player?.domain === "player" && player.cardinality === "one") {
      context.recipient = { created: true, emblemController: player.id };
      this.engineFeature("emblem", path, "Create the emblem with its printed abilities in the command zone for the bound player.",
        ["113.6p", "114.1", "114.2", "114.4"]);
    } else context.recipient = { unresolved: true };
  } else if (recipientKey) {
    const candidate = this.at(child(path, recipientKey));
    context.recipient = candidate ? { candidate } : { unresolved: true };
    // "Put a card an opponent owns from exile into that player's graveyard".
    const related = instruction && candidate && context.phase === "resolution" ? this.relationPlayer(candidate, child(path, "participant-0"), context) : undefined;
    if (related) context.players = combine(context.players, [related]);
    // "Creatures target opponent controls don't untap during their next untap step".
    if (instruction) context.players = combine(context.players, this.targetPlayersUnder(child(path, recipientKey), context));
  }
  const damageAmount = value.effect === "damage" && this.walkDamageAmount(value as unknown as Extract<Effect, { effect: "damage" }>, path, context);
  const drawRange = value.effect === "draw" && !context.barrier && context.phase === "resolution" &&
    isRecord(value.amount) && (!value.decidedBy || isDeepStrictEqual(value.decidedBy, value.who))
    ? chosenRange(value.amount as Amount) : undefined;
  const drawer = drawRange?.maximum !== undefined ? this.at(child(path, "who")) : undefined;
  const chosenDraw = drawRange && drawer?.cardinality === "one";
  if (chosenDraw) {
    const amount = child(path, "amount");
    this.node({ id: this.id(amount, "resolution-count"), path: amount, node: "resolution-count",
      scope: context.ability, chooser: drawer.id, operation: "draw", ...drawRange, rules: ["121.2", "608.2d"] });
  }
  const tested = value.condition === "is-filter" ? this.at(child(path, "what")) : undefined;
  for (const [entry, nested] of Object.entries(value)) {
    if (first.includes(entry) || (chosenDraw && entry === "amount") ||
        (damageAmount && (entry === "to" || entry === "amount"))) continue;
    const exiling = entry === "counters" && context.exileCounterFromSelected === path ? this.at(child(path, "what")) : undefined;
    const scoped = entry === "filter" && tested?.domain === "object" && tested.cardinality === "one"
      ? { ...context, characteristicSubject: tested.id } : exiling?.cardinality === "one"
        ? { ...context, boundOperand: { path: `${path}/counters/0/count/of`, candidate: exiling, rules: ["202.3", "400.7", "608.2h"] } } : context;
    this.walk(nested, child(path, entry), scoped);
  }
  Object.assign(context, saved);
}

export function rememberDecision(this: Analyzer, path: string, context: Context): void {
  const { objects, departed, players, quantities, sourceMoved, sourceSuccessor, previousEffect,
    exileResults, uncertainExileOrigin, stackCopies, uncertainStackOrigin, scans, dieResults, uncertainDieOrigin, uncertainScanOrigin, uncertainRevealOrigin,
    copiedCards, linkedCopies, uncertainCardCopyOrigin, measuredEvents, uncertainMeasureOrigin, precedingSentence } = context;
  this.beforeDecision.set(path, { objects, departed, players, quantities, sourceMoved, sourceSuccessor, previousEffect,
    exileResults, uncertainExileOrigin, stackCopies, uncertainStackOrigin, scans, dieResults, uncertainDieOrigin, uncertainScanOrigin, uncertainRevealOrigin,
    copiedCards, linkedCopies, uncertainCardCopyOrigin, measuredEvents, uncertainMeasureOrigin, precedingSentence });
}
/** Branch contexts start from the antecedents established so far; nothing a
 * branch establishes is visible to a sibling or after the construct. */
export function branch(this: Analyzer, context: Context): Context {
  return { ...context, objects: [...context.objects], players: [...context.players], quantities: [...context.quantities] };
}
export function blockBranchCaptures(this: Analyzer, context: Context, branches: Context[]): void {
  context.uncertainExileOrigin ||= branches.some(branch =>
    branch.uncertainExileOrigin || branch.exileResults !== context.exileResults);
  context.uncertainStackOrigin ||= branches.some(branch =>
    branch.uncertainStackOrigin || branch.stackCopies !== context.stackCopies);
  context.uncertainScanOrigin ||= branches.some(branch =>
    branch.uncertainScanOrigin || branch.scans !== context.scans);
  context.uncertainDieOrigin ||= branches.some(branch => branch.uncertainDieOrigin || branch.dieResults !== context.dieResults);
  context.uncertainRevealOrigin ||= branches.some(branch => branch.uncertainRevealOrigin);
  context.uncertainCardCopyOrigin ||= branches.some(branch =>
    branch.uncertainCardCopyOrigin || branch.copiedCards !== context.copiedCards);
  context.uncertainMeasureOrigin ||= branches.some(branch =>
    branch.uncertainMeasureOrigin || branch.measuredEvents !== context.measuredEvents);
  context.uncertainTargetOrigin ||= branches.some(branch =>
    branch.uncertainTargetOrigin || branch.declaredTargets !== context.declaredTargets);
}
/** A triggered ability created now (CR 603.7): its source is the creating
 * ability's source and its controller the player who controlled that ability
 * (603.7d-e). It may refer to objects the creating ability named; those are
 * the same objects (603.7c). What it establishes stays inside it. Targets the
 * creating ability chose belong to that ability's announcement. */
export function nestedAbility(this: Analyzer, path: string, context: Context, kind: string, rules: string[]): Context {
  const ability = this.node({ id: this.id(path, kind), path, node: "ability", source: context.source, face: context.face,
    abilityKind: kind, you: context.you, rules });
  return { ...this.branch(context), ability, phase: "resolution", decisions: undefined, previousDecisions: undefined,
    decider: undefined, conditional: undefined, availableChoice: undefined, entryChoice: undefined, resolutionChoice: undefined,
    sacrificedCost: undefined, enteringKickerX: undefined, zoneAlternatives: undefined, graveyardExclusion: undefined, castActors: undefined, stackPlacement: undefined, singleTargetStackGuard: undefined, stackCopies: undefined, previousEffect: undefined, precedingSentence: undefined, directCardChoice: undefined, lookedCardChoice: undefined, performedRepeat: undefined, groupChoice: undefined, explicitExiledCast: undefined, boundOperand: undefined, exileResults: undefined,
    uncertainExileOrigin: undefined, uncertainStackOrigin: undefined, scans: undefined, dieResults: undefined, uncertainDieOrigin: undefined, uncertainScanOrigin: undefined, uncertainRevealOrigin: undefined,
    copiedCards: undefined, linkedCopies: undefined, uncertainCardCopyOrigin: undefined, announcementMember: undefined, targetPlayerGroup: undefined, definiteEventPlayer: undefined, attackedTriggerPlayer: undefined,
    measuredEvents: undefined, uncertainMeasureOrigin: undefined, triggerX: undefined, lifeGain: undefined, resolutionPaymentX: undefined,
    linkedExile: undefined, countedLinkedPool: undefined, namedVote: undefined, weightedMode: undefined,
    exchangeEntrant: undefined, quotedFightSide: undefined, ownLeaveEvent: undefined, exileCounterFromSelected: undefined,
    lexicalAbilityPath: undefined, delayedSchedule: kind === "delayed-triggered",
    declaredTargets: undefined, uncertainTargetOrigin: undefined };
}
