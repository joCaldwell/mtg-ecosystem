// Antecedent and pronoun resolution for object references.
import { isDeepStrictEqual } from "node:util";
import type { Ability, ObjectFilter, ObjectRef, PlayerRef } from "../../ast.ts";
import { isPluralPlayerRef } from "../../parser/refs.ts";
import type { BindingCardinality, BindingNode, ReferenceBinding, ReferenceExpression } from "../types.ts";
import type { Analyzer } from "./analyzer.ts";
import type { ChoiceReference } from "./choices.ts";
import { ENTRY_CHOICE_RULES, child, combine, describes, description, domain, impliesPermanent, objectCardinality, referenceCardinality, type Candidate, type Context } from "./context.ts";
import { isRecord } from "./predicates.ts";

export function antecedent(this: Analyzer, expression: ReferenceExpression, path: string, candidates: Candidate[], desired: BindingCardinality,
  rules: string[]): Candidate | undefined {
  if (candidates.length !== 1) {
    this.unresolved(expression, path, candidates.length ? "ambiguous-antecedent" : "missing-antecedent",
      candidates.length ? "More than one eligible antecedent remains; no proximity heuristic is applied."
        : "No antecedent is established in this scope.", rules, candidates.map(candidate => candidate.id));
    return undefined;
  }
  const candidate = candidates[0];
  if (candidate.domain !== domain(expression)) {
    this.unresolved(expression, path, "unsupported-reference", "An object/player-union selection does not establish the kind of this pronoun.", rules, [candidate.id]);
    return undefined;
  }
  // A group of unknown size ("up to X cards") is what a plural reference
  // ("those cards", "them") names; a singular one needs exactly one.
  if (desired !== "unknown" && candidate.cardinality !== desired && !(desired === "many" && candidate.cardinality === "unknown")) {
    this.unresolved(expression, path, "cardinality-mismatch", "The antecedent's cardinality does not establish this reference.", rules, [candidate.id]);
    return undefined;
  }
  if ("ref" in expression && expression.ref === "that" && !describes(candidate.description, expression.filter)) {
    this.unresolved(expression, path, "unsupported-reference", "The antecedent's established description does not resolve this qualified reference.", rules, [candidate.id]);
    return undefined;
  }
  this.bound(expression, path, candidate.id, rules);
  const group = this.groupedTokenEvents.get(candidate.id);
  if (group) this.engineFeature("group-instruction", group,
    "Preserve the actual tokens created by every participant within the one grouped instruction.", ["101.4", "111.2", "608.2f"]);
  return candidate;
}
export function chosenValue(this: Analyzer, expression: ChoiceReference, path: string, context: Context): void {
  const local = context.availableChoice;
  const origin = local ?? (context.sourceMoved ? undefined : context.entryChoice);
  const category = origin?.category;
  const matches = expression.choice === "chosen-value"
    ? expression.which === "chosen" && expression.category === category
    : expression.kind === undefined && category !== undefined &&
      ["card-type", "creature-type", "basic-land-type", "land-type"].includes(category);
  // CR 601.2c: a resolution choice is unavailable when targets are announced.
  // CR 607.5a: an entry choice is undefined if that linked choice never ran.
  if (origin && matches && !context.barrier && !(local && (context.announcementOperand || context.phase !== "resolution"))) {
    const rules = local ? ["607.1c", "607.2d", "608.2c"] : [...ENTRY_CHOICE_RULES, "113.7a", "608.2h"];
    this.bound(expression, path, this.id(origin.path, "value-choice"), rules);
  } else {
    this.unresolved(expression, path, "unsupported-reference", expression.choice === "chosen-type"
      ? "This type needs a linked choice definition; its kind cannot be inferred from the filtered object's type."
      : "This symbolic value requires the linked originating choice and its scope; a nearby choice is not proof of that association.",
      expression.choice === "chosen-type" ? ["205.3e", "607.2d"] : ["607.2d"]);
  }
}
export function linkedExileFilter(this: Analyzer, filter: Record<string, unknown>, path: string, context: Context): boolean {
  const origin = context.linkedExile;
  if (context.barrier || context.sourceMoved || !origin || (context.lexicalAbilityPath === origin.ability && origin.rule !== "607.1c") ||
      !isRecord(filter.source) || (filter.source.ref !== "self" && filter.source.ref !== "this")) return false;
  this.walkChildren(filter, path, context);
  const rules = ["400.7", origin.rule, "607.3", "607.5"];
  this.node({ id: this.id(path, "linked-exile-filter"), path, node: "linked-exile-filter", scope: context.ability,
    pool: this.id(origin.path, "linked-exile-pool"), rules });
  return true;
}

/** Every established object antecedent is the source itself. */
export function onlySource(this: Analyzer, context: Context): boolean {
  return context.objects.every(object => object.id === context.source || this.sourceAliases.has(object.id));
}
/** The object or player the source permanent is attached to (CR 301.5f, 303.4m). */
export function attachedTo(this: Analyzer, path: string, context: Context): string {
  const id = `${context.ability}/attached-to`;
  if (!this.result.nodes.some(node => node.id === id))
    this.node({ id, path, node: "derived-object", scope: context.ability, role: "attached-to", of: context.source, rules: ["301.5f", "303.4m"] });
  return id;
}
export function exileAndSearchPool(this: Analyzer, path: string, context: Context): string | undefined {
  const previous = context.precedingSentence;
  if (context.barrier || context.phase !== "resolution" || !previous || previous.steps.length !== 2) return;
  const [exile, search] = previous.steps;
  if (exile.effect !== "exile" || exile.what.ref !== "target" || objectCardinality(exile.what) !== "one" ||
      exile.faceDown || exile.forEach || exile.forEachVote ||
      search.effect !== "search" || search.optional || search.forEach || search.forEachVote || search.decidedBy ||
      search.who.player !== "you" || !isDeepStrictEqual(search.zones, { selection: "all", of: [{ zone: "library", owner: "your" }] }) ||
      search.for.ref !== "filter" || search.for.filter.cls !== "card" || objectCardinality(search.for) !== "one") return;
  const exiled = this.id(`${previous.path}/steps/0`, "objects");
  const foundPath = `${previous.path}/steps/1`;
  const searchEvent = this.id(foundPath, "event");
  const choice = this.at(child(foundPath, "for"));
  const exileResult = this.result.nodes.find(node => node.node === "event-result" && node.id === exiled);
  if (!exileResult || !choice) return;

  const rules = ["400.7j", "608.2c", "609.3", "701.23b"];
  const found = this.eventResult(foundPath, searchEvent, "objects", "one", "same-objects", choice.description);
  const pool = this.node({ id: this.id(path, "exile-and-search"), path, node: "result-union", scope: context.ability,
    from: [exiled, found.id], cardinality: "many", rules });
  this.engineFeature("exile-and-search-results", path,
    "Use only the actual exile successor and the actual library find, each of which can be absent.", rules);
  return pool;
}

export function object(this: Analyzer, expression: ObjectRef, path: string, context: Context): Candidate | undefined {
  if (!context.barrier && !context.sourceMoved && context.linkedExile && expression.ref === "related" &&
      expression.number === "plural" && expression.relation.relation === "linked-exile" &&
      isDeepStrictEqual(expression.filter, { cls: "card" }) &&
      (expression.relation.source.ref === "self" || expression.relation.source.ref === "this") &&
      (context.linkedExile.ability !== context.lexicalAbilityPath || context.linkedExile.rule === "607.1c")) {
    const rules = [context.linkedExile.rule, "607.2a", "607.3", "400.7"];
    this.walk(expression.relation.source, `${path}/relation/source`, context);
    return this.bound(expression, path, this.id(context.linkedExile.path, "linked-exile-pool"), rules);
  }

  if (!context.barrier && expression.ref === "self" && context.quotedFightSide?.paths.includes(path)) {
    const rules = ["113.10", "201.5a", "400.7", "701.14b"];
    this.engineFeature("quoted-source-reference", path,
      "Use the same granting creature incarnation for the named fight participant and require both creatures to still be on the battlefield.", rules);
    return this.bound(expression, path, context.quotedFightSide.source, rules);
  }
  if (!context.barrier && context.boundOperand?.path === path) {
    const { candidate, rules } = context.boundOperand;
    return this.antecedent(expression, path, [candidate], objectCardinality(expression), rules);
  }
  if (expression.ref === "related" && expression.number === "plural" &&
      isDeepStrictEqual(expression.filter, { cls: "card" }) &&
      expression.relation.relation === "exile-and-search-results" && expression.relation.from === "preceding-sentence") {
    const pool = this.exileAndSearchPool(path, context);
    if (pool) return this.bound(expression, path, pool, ["400.7j", "608.2c", "701.23b"]);
  }
  if (!context.barrier && context.exactEventObject?.path === path &&
      (expression.ref === "replacement-event-object" || expression.ref === "prevention-event-object"))
    return this.bound(expression, path, context.exactEventObject.id, context.exactEventObject.rules);
  if (expression.ref === "related" && expression.relation.relation === "event-result" &&
      expression.relation.event === "exile" && expression.filter.cls === "card" &&
      Object.keys(expression.filter).length === 1 && !context.barrier && !context.uncertainExileOrigin &&
      context.exileResults?.length === 1) {
    const [capture] = context.exileResults;
    const rules = ["400.7j", "608.2c"];
    const at = child(path, "relation");
    this.node({ id: this.id(at, "event-link"), path: at, node: "event-link", scope: context.ability,
      action: "exile", event: capture.event, rules });
    return this.antecedent(expression, path, [capture.result], objectCardinality(expression), rules);
  }
  if (!context.barrier && !context.uncertainScanOrigin && context.scans?.length === 1 &&
      expression.ref === "related" && expression.relation.relation === "event-result" &&
      expression.relation.event === context.scans[0].operation && expression.filter.cls === "card" &&
      (expression.relation.event !== "reveal" || !context.uncertainRevealOrigin) &&
      Object.keys(expression.filter).length === 1 &&
      (expression.relation.event !== "exile" || !context.exileResults?.length)) {
    const { event, pool, matches } = context.scans[0];
    const rules = ["608.2c", expression.relation.event === "reveal" ? "701.20" : "400.7j"];
    const at = child(path, "relation");
    this.node({ id: this.id(at, "event-link"), path: at, node: "event-link", scope: context.ability,
      action: expression.relation.event, event, rules });
    return this.antecedent(expression, path, [expression.number === "plural" ? pool : matches], objectCardinality(expression), rules);
  }
  // A paid sacrifice and a later zone successor are different objects.
  // This contextual reference is supported only for information about the
  // uniquely identified original object (CR 400.7, 608.2h, 701.21a).
  if (!context.barrier && context.lastKnown && context.sacrificedCost &&
      expression.ref === "related" && expression.number === "singular" &&
      expression.relation.relation === "event-result" && expression.relation.event === "sacrifice" &&
      expression.relation.scope === "contextual" && describes(context.sacrificedCost.description, expression.filter)) {
    const rules = ["601.2h", "602.2b", "608.2h", "701.21a"];
    this.bound(expression, path, context.sacrificedCost.id, rules);
    return context.sacrificedCost;
  }
  if (expression.ref === "card-extraction-group") context = { ...context, barrier: "Separate card-extraction slots require distinct allocation, shared-pool, branch-choice and aggregate result bindings." };
  const target = expression.ref === "target" || expression.ref === "any-target";
  const operands = target ? { ...context, announcementOperand: true } : context;
  const count = this.boundTargetCount(expression, context) ?? this.boundResolutionCount(expression, path, context);
  for (const [key, value] of Object.entries(expression)) {
    if (key === "filter" && isRecord(value) && Array.isArray(value.characteristicRelations) &&
        !operands.barrier && (expression.ref === "target" || expression.ref === "each" ||
          expression.ref === "all" || expression.ref === "filter")) {
      const at = child(path, key);
      const timing = target || context.targetLegalityQuery ? "target-legality" : context.phase;
      const subject = this.node({ id: this.id(at, "filter-candidate"), path: at, node: "characteristic-subject",
        scope: context.ability, role: "evaluated-candidate", timing, rules: ["601.2c", "608.2b", "608.2h"] });
      this.walk(value, at, { ...operands, characteristicSubject: subject, ...(target ? { targetLegalityQuery: true as const } : {}) });
    } else if (key !== "count" || !count) this.walk(value, child(path, key), operands);
  }
  if ("groupConstraints" in expression && expression.groupConstraints?.length) {
    this.unresolved(expression, path, "unsupported-scope", "Collective selection restrictions need a selected-group scope and legality binding; per-candidate eligibility alone is insufficient.", ["201.2b", "601.2c", "608.2d"]);
    return undefined;
  }
  if (context.barrier && !(path === context.replacementSubjectPath && expression.ref === "self" && Object.keys(expression).length === 1)) {
    this.unresolved(expression, path, "unsupported-scope", context.barrier, ["201.5", "607"]);
    return undefined;
  }
  switch (expression.ref) {
    case "self": case "this":
      if (expression.ref === "this" && expression.noun === "ability") return this.bound(expression, path, context.ability, ["113.7"]);
      if (context.sourceMoved) {
        // Its owner, controller or characteristics come from last-known information (CR 608.2h).
        if (context.lastKnown) return this.bound(expression, path, context.source, ["201.5", "608.2h"]);
        // The card it became is what an instruction acts on ("return this
        // card to your hand"); questions about it use last-known information.
        if (context.sourceSuccessor && /\/what(\/\d+)?$/.test(path)) return this.bound(expression, path, context.sourceSuccessor.id, ["400.7e", "603.6c"]);
        this.unresolved(expression, path, "unsupported-reference", "After a source zone change, this reference needs an action-specific source/last-known/successor rule.", ["400.7", "608.2h"]);
        return undefined;
      }
      return this.bound(expression, path, context.source, ["201.5"]);
    case "it": case "them": case "that": {
      const printed = printedPronoun(this, expression, path, context);
      if (printed) return this.antecedent(expression, path, [printed.candidate], "one", printed.rules);
      const role = this.roleAntecedents.get(path);
      if (role) return this.antecedent(expression, path, [role], objectCardinality(expression), ["614.15", "608.2c"]);
      // Older templating says "it" for the permanent with the ability ("{T}:
      // It deals 1 damage"). With no other object named in the ability, it
      // can mean nothing else.
      if (expression.ref === "it" && (context.permanentSource || context.staticAbility) && !context.mentionedObject && !context.spellPronoun && context.objects.length === 0
        && (!context.sourceMoved || context.lastKnown))
        return this.bound(expression, path, context.source, context.sourceMoved ? ["201.5", "113.7a", "608.2h"] : ["201.5"]);
      // CR 608.2h: information about an object that has left its zone comes
      // from its last-known information ("its controller", "its power").
      let candidates = context.lastKnown && context.departed?.length ? combine(context.objects, context.departed) : context.objects;
      // Asked for its information, an object a trigger saw move is known by
      // its last-known information, not as the object it became (CR 603.10a).
      // A card the effect put onto the battlefield is where it was expected
      // to be, so its current information answers (CR 608.2h).
      // "That card" still means the card it was.
      const onBattlefield = (candidate: Candidate) => candidate.description?.cls === "permanent"
        && !(expression.ref === "that" && expression.filter.cls === "card");
      if (context.lastKnown) candidates = candidates.filter(candidate => candidate.successorOf && onBattlefield(candidate)
        ? true : !candidates.some(other => other.successorOf === candidate.id && onBattlefield(other))
          && (!candidate.successorOf || !candidates.some(other => other.id === candidate.successorOf)));
      // Acted on, it is the object it became (CR 400.7e): "its owner shuffles it into their library".
      else candidates = candidates.filter(candidate => !candidates.some(other => other.successorOf === candidate.id));
      // A pronoun means the source (or the definite subject of its trigger),
      // or an object a cost chose, only when nothing else could be meant (as
      // for a bare "it" above): another object the ability introduces is the
      // antecedent. Those objects are named explicitly when they are meant.
      // "That creature" never means an object the text calls "this creature".
      if (expression.ref === "that") candidates = candidates.filter(candidate => candidate.id !== context.source && !this.sourceAliases.has(candidate.id));
      else candidates = candidates.filter(candidate => !candidate.qualifiedOnly);
      // "When a player casts a spell, ... counter that spell", "if at least
      // four mana was spent to cast it": the spell the trigger is about,
      // when nothing else is named (CR 603.2c).
      if (!candidates.length && context.triggerObject && (expression.ref === "that"
        ? describes(context.triggerObject.candidate.description, expression.filter) : context.spellPronoun)) candidates = [context.triggerObject.candidate];
      const named = expression.ref !== "that" && context.named
        && !candidates.some(candidate => !context.named!.ids.includes(candidate.id) && !context.named!.before.includes(candidate.id))
        ? context.named.ids : [];
      // The card a trigger saw an object become is, like the source, meant
      // only when the effect names nothing else.
      const fallback = (candidate: Candidate) => !named.includes(candidate.id) && (this.costObjects.has(candidate.id)
        || candidate.triggerSuccessor === true
        || (!context.sourceNamed && (candidate.id === context.source || this.sourceAliases.has(candidate.id))));
      // A possessive means its own instruction's subject: "This creature
      // deals damage equal to its power to that creature."
      const subject = context.subjects?.length === 1 ? context.subjects[0] : undefined;
      if (expression.ref === "it" && path.endsWith("/of") && subject && candidates.some(candidate => candidate.id === subject.id)) candidates = [subject];
      else {
        // "If that creature has power 4 or greater, it gains first strike":
        // a pronoun governed by a condition means the condition's subject,
        // even one that would otherwise be meant only as a last resort. An
        // object the governed instructions introduce after the condition is
        // meant instead ("if this creature is tapped, search for a card, put
        // it into your graveyard").
        const governing = context.conditionSubject;
        if (candidates.length > 1 && governing && candidates.some(candidate => candidate.id === governing.candidate.id)
          && (expression.ref !== "that" || describes(governing.candidate.description, expression.filter))
          && candidates.every(candidate => candidate.id === governing.candidate.id || governing.before.includes(candidate.id)))
          candidates = [governing.candidate];
        if (candidates.some(fallback) && candidates.some(candidate => !fallback(candidate))) candidates = candidates.filter(candidate => !fallback(candidate));
        // "That land" means an antecedent established as a land; only when
        // every candidate's kind is established (the source's by its type line).
        if (expression.ref === "that" && candidates.length > 1) {
          const established = (candidate: Candidate) => candidate.id === context.source || this.sourceAliases.has(candidate.id)
            ? this.faceTypes.get(context.face) : candidate.description;
          if (candidates.every(candidate => established(candidate))) {
            const agreeing = candidates.filter(candidate => describes(established(candidate), expression.filter));
            if (agreeing.length) candidates = agreeing;
          }
        }
      }
      const hand = this.revealedHand(expression, path, candidates);
      if (hand) return this.antecedent(expression, path, [hand], "many", ["400.2", "608.2c", "701.20a"]);
      const picked = this.poolPickAntecedent(expression, path, candidates, context);
      if (picked) return this.antecedent(expression, path, [picked], objectCardinality(expression), ["608.2c", "400.7j"]);
      return this.antecedent(expression, path, candidates, objectCardinality(expression), ["608.2k", "400.7j", ...(candidates !== context.objects ? ["608.2h"] : [])]);
    }
    case "trigger-event-object":
      if (expression.event === "enter" && context.exchangeEntrant)
        return this.bound(expression, path, context.exchangeEntrant.id, ["603.2c", "603.3d", "608.2b"]);
      if (context.triggerObject?.event === expression.event)
        return this.bound(expression, path, context.triggerObject.candidate.id, ["603.2c", "707.10"]);
      this.unresolved(expression, path, "missing-antecedent", "No unique matching stack-object trigger occurrence is established in this lexical ability.", ["603.2c", "707.10"]);
      return undefined;
    case "card-extraction-group":
      this.unresolved(expression, path, "unsupported-scope", "Separate card-extraction slots require distinct allocation, shared-pool, branch-choice and aggregate result bindings.", ["608.2d", "701.23b"]);
      return undefined;
    case "choice-result":
      if (expression.scope === "contextual" && expression.which === "chosen" &&
          !context.uncertainTargetOrigin && context.declaredTargets?.length === 1 &&
          describes(context.declaredTargets[0].description, expression.filter))
        return this.antecedent(expression, path, context.declaredTargets, objectCardinality(expression), ["601.2c", "608.2b", "608.2c"]);
      this.unresolved(expression, path, "unsupported-reference", "This chosen object needs its exact announcement, resolution/search or linked-choice result; proximity does not establish the origin.", ["601.2c", "608.2d", "607.2d"]);
      return undefined;
    case "copy-result":
      if (context.copiedCards?.length === 1 && !context.uncertainCardCopyOrigin &&
          /\/instruction\/(?:what|fromAmong)$/.test(path))
        return this.antecedent(expression, path, context.copiedCards, objectCardinality(expression), ["707.12", "707.12a"]);
      this.unresolved(expression, path, "unsupported-reference", "This printed copy reference needs a specific copy-result binding; it is not the most recent object.", ["707.10"]);
      return undefined;
    case "amassed-army":
      this.unresolved(expression, path, "unsupported-reference", "The amassed Army needs the Army creature chosen by the preceding amass.", ["701.47c"]);
      return undefined;
    case "vote-leaders":
      this.unresolved(expression, path, "unsupported-reference", "The objects with the most votes need the preceding vote's tally.", ["701.38"]);
      return undefined;
    case "soulbond-pair":
      this.unresolved(expression, path, "unsupported-reference", "Both members of a soulbond pair need the pairing relation of the referenced creature.", ["702.95b"]);
      return undefined;
    case "pile":
      this.unresolved(expression, path, "unsupported-reference", "The objects of a pile need the preceding separation and the pile choice.", ["700.3"]);
      return undefined;
    case "equipped": case "enchanted": case "fortified": {
      // CR 301.5f, 303.4m: whatever the permanent with this ability is attached to.
      if (context.sourceMoved) {
        this.unresolved(expression, path, "unsupported-reference", "After a source zone change, the attachment relation needs the source's last-known information.", ["400.7", "608.2h"]);
        return undefined;
      }
      return this.bound(expression, path, this.attachedTo(path, context), ["301.5f", "303.4m"]);
    }
    case "rest": {
      const scan = context.scans?.length === 1 && !context.uncertainScanOrigin &&
        (context.scans[0].operation !== "reveal" || !context.uncertainRevealOrigin) ? context.scans[0] : undefined;
      const explicitPool = expression.of ? this.at(child(path, "of")) : undefined;
      if (scan && (explicitPool ? explicitPool.id === scan.pool.id :
          !expression.of && !this.poolSelections.has(context.ability) &&
            !context.objects.some(candidate => candidate.id !== scan.matches.id && candidate.cardinality !== "one"))) {
        const rules = ["608.2c", scan.operation === "reveal" ? "701.20" : "400.7j"];
        const id = this.node({ id: this.id(path, "remainder"), path, node: "remainder", scope: context.ability,
          pool: scan.pool.id, excluding: scan.matches.id, rules });
        return this.bound(expression, path, id, rules);
      }

      // A special recognizer's pick (no printed selection) from the same
      // sentence's pool. A printed pick goes through pickRemainder below.
      const sentence = /^(.*\/effects\/\d+)\//.exec(path)?.[1];
      const chosen = this.poolSelections.get(context.ability);
      const pool = expression.of ? this.at(child(path, "of")) : chosen?.pool;
      if (chosen && pool && sentence && chosen.path.startsWith(`${sentence}/`) && chosen.pool.id === pool.id &&
          !this.result.nodes.some(node => node.node === "selection" && node.path === chosen.path)) {
        const id = this.node({ id: this.id(path, "remainder"), path, node: "remainder", scope: context.ability, pool: pool.id,
          excluding: chosen.selected.id, rules: ["608.2c"] });
        return this.bound(expression, path, id, ["608.2c"]);
      }
      const remainder = this.pickRemainder(expression, path, context);
      if (remainder) return remainder;
      this.unresolved(expression, path, "unsupported-reference", "This relation needs an attachment, event, pool, or linked-ability binding.", ["303.4", "301.5", "607"]);
      return undefined;
    }
    case "haunted-creature": case "targeted-by": case "related": case "replacement-event-object": case "prevention-event-object":
      this.unresolved(expression, path, "unsupported-reference", "This relation needs an attachment, event, pool, or linked-ability binding.", ["303.4", "301.5", "607"]);
      return undefined;
    case "target": case "any-target": return this.selection(expression, path, context, true);
    case "from-pool": {
      const selected = this.selection(expression, path, context, false);
      const pool = this.at(child(path, "pool"));
      if (selected && pool) this.poolSelections.set(context.ability, { pool, selected, path });
      return selected;
    }
    case "filter": case "each": case "all": case "library-cards": case "graveyard-cards": case "zone-cards": case "commander":
      return this.selection(expression, path, context, false);
    default: {
      const unreachable: never = expression;
      throw new Error(`Unhandled ObjectRef variant: ${JSON.stringify(unreachable)}`);
    }
  }
}
// CR 303.4b, 301.5, 611.3a and 613.6: these modifiers read the one permanent
// the Aura or Equipment is attached to. A set of affected permanents, or a
// pronoun printed in a granted ability, needs a different scope.
function attachedModifier(analyzer: Analyzer, ability: Extract<Ability, { kind: "static" }>, suffix: string,
  context: Context): Candidate | undefined {
  const { effect } = ability;
  if (effect.static !== "modify" || effect.condition || effect.duration ||
      !(isDeepStrictEqual(effect.what, { ref: "enchanted", noun: "creature" }) ||
        isDeepStrictEqual(effect.what, { ref: "equipped", noun: "creature" }))) return;
  const at = /^\/effect\/modifiers\/(\d+)\/(multiplier\/among|forEach\/attachedTo)$/.exec(suffix);
  if (!at) return;
  const modifier = effect.modifiers[Number(at[1])];
  if (modifier?.modifier !== "pt") return;
  const { power, toughness } = modifier;
  const attachmentCount = {
    allOf: [{ anyOf: [{ subtypes: ["Aura"] }, { subtypes: ["Equipment"] }] }], attachedTo: { ref: "it" },
  };
  const shape = at[2] === "multiplier/among"
    ? { modifier: "pt", power, toughness, multiplier: { amount: "distinct-count", characteristic: "color", among: { ref: "it" } },
        valueScope: "each-affected-object" }
    : { modifier: "pt", power, toughness, forEach: attachmentCount };
  if (!isDeepStrictEqual(modifier, shape)) return;
  const recipient = analyzer.at(`${context.lexicalAbilityPath}/effect/what`);
  if (recipient?.domain === "object" && recipient.cardinality === "one" && !context.objects.length) return recipient;
}

// CR 601.2c and 608.2b-c: "target creature and all other creatures with the
// same name as that creature" uses the announced target, not a result of
// resolving the simultaneous instruction.
function simultaneousNameTarget(analyzer: Analyzer, ability: Extract<Ability, { kind: "spell" }>,
  suffix: string, context: Context): Candidate | undefined {
  const at = "/effects/0/steps/0";
  if (suffix !== `${at}/steps/1/what/filter/characteristicRelations/0/with/what` || context.objects.length) return;
  const group = ability.effects[0]?.steps[0];
  if (group?.effect !== "simultaneous" || group.steps.length !== 2) return;
  const [first, second] = group.steps;
  if (first.effect !== "pump" || second.effect !== "pump" || first.what.ref !== "target" ||
      typeof first.what.selectionId !== "string") return;
  const { selectionId } = first.what;
  const creature = { types: ["creature"] };
  const { power, toughness, duration } = first;
  if (!isDeepStrictEqual(ability.effects[0], { steps: [{
    effect: "simultaneous", steps: [
      { effect: "pump", what: { ref: "target", filter: creature, count: { amount: "fixed", value: 1 }, selectionId },
        power, toughness, duration },
      { effect: "pump", what: { ref: "all", filter: { ...creature, other: true, characteristicRelations: [
        { relation: "shared-value", characteristic: "name", with: { operand: "object", what: { ref: "that", filter: creature } } },
      ] } }, power, toughness, duration },
    ],
  }] })) return;
  const target = analyzer.at(`${context.lexicalAbilityPath}${at}/steps/0/what`);
  if (target?.domain === "object" && target.cardinality === "one" && analyzer.announced(target)) return target;
}

// CR 608.2h: the decision maker and the player who searches, moves and
// shuffles are all the destroyed target's controller. The nested steps
// repeat the group header's subject. Their land-card result is the separate
// object that the move acts on.
function controllerSearchTarget(analyzer: Analyzer, ability: Extract<Ability, { kind: "spell" }>,
  suffix: string, context: Context): { candidate: Candidate; rules: string[] } | undefined {
  const groupPath = "/effects/1/steps/0";
  const isLand = suffix === `${groupPath}/steps/1/what`;
  if (!isLand && (!context.lastKnown || !["/steps/1/who/of", "/steps/2/who/of", "/steps/2/library/owner/of"]
    .some(operand => suffix === groupPath + operand))) return;
  const destroy = ability.effects[0]?.steps[0];
  const group = ability.effects[1]?.steps[0];
  if (ability.effects.length !== 2 || destroy?.effect !== "destroy" || destroy.what.ref !== "target" ||
      objectCardinality(destroy.what) !== "one" || !impliesPermanent(destroy.what.filter) ||
      !isDeepStrictEqual(ability.effects[0], { steps: [{ effect: "destroy", what: destroy.what }] }) ||
      group?.effect !== "optional-group") return;
  const who = { player: "controller", of: { ref: "it" } };
  const move = group.steps[1];
  if (move?.effect !== "move-zone" || !isDeepStrictEqual(ability.effects[1], { steps: [{
    effect: "optional-group", who, steps: [
      { effect: "search", who, zones: { selection: "all", of: [{ zone: "library", owner: "their" }] },
        for: { ref: "filter", filter: { supertypes: ["basic"], types: ["land"], cls: "card" },
          count: { amount: "fixed", value: 1 } } },
      { effect: "move-zone", what: { ref: "it" }, to: { zone: "battlefield" }, who, ...(move.tapped === true ? { tapped: true } : {}) },
      { effect: "shuffle", who, library: { zone: "library", owner: who } },
    ],
  }] })) return;
  const target = analyzer.at(`${context.lexicalAbilityPath}/effects/0/steps/0/what`);
  const header = `${context.lexicalAbilityPath}${groupPath}/who`;
  if (target?.domain !== "object" || target.cardinality !== "one" || !analyzer.announced(target) ||
      analyzer.at(`${header}/of`)?.id !== target.id || analyzer.at(header)?.id !== context.decider) return;
  if (!isLand) return { candidate: target, rules: ["110.2a", "608.2c", "608.2d", "608.2h"] };
  const search = `${context.lexicalAbilityPath}${groupPath}/steps/0`;
  const found = analyzer.at(`${search}/for`);
  const cards = context.objects.filter(candidate => candidate.id !== target.id);
  if (found && cards.length === 1 && cards[0].id === found.id)
    return { candidate: cards[0], rules: ["608.2c", "608.2d", "701.23b"] };
}

// CR 108.3, 400.7j, 608.2c and 712.14a: the two steps exile this transforming
// permanent and return the card it became. A previously drawn card or the
// land that triggered the ability supplies neither the return nor its owner.
function returningTransformedSource(analyzer: Analyzer, ability: Extract<Ability, { kind: "triggered" | "activated" }>,
  suffix: string, context: Context): Candidate | undefined {
  const at = /^\/effects\/(\d+)\/steps\/1\/what$/.exec(suffix);
  if (!at || !context.sourceMoved || !context.permanentSource || context.objects.length < 2) return;
  const sentence = ability.effects[Number(at[1])];
  if (!sentence || !isDeepStrictEqual(sentence.steps, [
    { effect: "exile", what: { ref: "self" } },
    { effect: "move-zone", what: { ref: "it" }, to: { zone: "battlefield" }, controller: "owner",
      entry: { face: "transformed" } },
  ])) return;
  const exile = `${context.lexicalAbilityPath}/effects/${at[1]}/steps/0`;
  const successors = context.objects.filter(candidate => candidate.successorOf === context.source);
  if (successors.length === 1 && successors[0].id === analyzer.id(exile, "objects") &&
      analyzer.at(`${exile}/what`)?.id === context.source) return successors[0];
}

// The Aura deals the damage, while the tapped creature supplies its controller
// as the recipient (CR 108.4, 603.2c, 608.2h).
function tappedAttachment(analyzer: Analyzer, ability: Extract<Ability, { kind: "triggered" }>,
  suffix: string, context: Context): Candidate | undefined {
  if (suffix !== "/effects/0/steps/0/to/ref/of" || !context.lastKnown) return;
  const damage = ability.effects[0]?.steps[0];
  if (damage?.effect !== "damage" || !isDeepStrictEqual(ability.trigger,
    { trigger: "becomes-tapped", what: { ref: "enchanted", noun: "creature" } }) ||
      !isDeepStrictEqual(ability.effects[0], { steps: [{
        effect: "damage", source: { ref: "this", noun: "aura" }, amount: damage.amount,
        to: { to: "player", ref: { player: "controller", of: { ref: "that", filter: { types: ["creature"] } } } },
      }] })) return;
  const candidates = context.objects.filter(candidate => candidate.id !== context.source);
  if (candidates.length === 1 && candidates[0].id === analyzer.id(`${context.lexicalAbilityPath}/trigger`, "objects"))
    return candidates[0];
}

function printedPronoun(analyzer: Analyzer, expression: Extract<ObjectRef, { ref: "it" | "that" | "them" }>,
  path: string, context: Context): { candidate: Candidate; rules: string[] } | undefined {
  const root = context.lexicalAbilityPath;
  if (!root || context.ability !== analyzer.id(root, "ability") || !path.startsWith(`${root}/`) ||
      context.announcementOperand || context.targetLegalityQuery) return;
  // Only face-level abilities are stored under their own path in this map.
  const ability = analyzer.abilityAsts.get(root) as Ability | undefined;
  const suffix = path.slice(root.length);
  let candidate: Candidate | undefined;
  if (isDeepStrictEqual(expression, { ref: "it" })) {
    if (ability?.kind === "static" && context.phase === "continuous") {
      candidate = attachedModifier(analyzer, ability, suffix, context);
      if (candidate) return { candidate, rules: ["301.5", "303.4b", "611.3a", "613.6"] };
    } else if (ability?.kind === "spell" && context.phase === "resolution") {
      return controllerSearchTarget(analyzer, ability, suffix, context);
    } else if ((ability?.kind === "activated" || ability?.kind === "triggered") && context.phase === "resolution") {
      candidate = returningTransformedSource(analyzer, ability, suffix, context);
      if (candidate) return { candidate, rules: ["108.3", "400.7j", "608.2c", "712.14a"] };
    }
  } else if (context.phase === "resolution") {
    if (isDeepStrictEqual(expression, { ref: "that", filter: { types: ["creature"] } })) {
      if (ability?.kind === "spell") {
        candidate = simultaneousNameTarget(analyzer, ability, suffix, context);
        if (candidate) return { candidate, rules: ["201.2a", "601.2c", "608.2b"] };
      } else if (ability?.kind === "triggered") {
        candidate = tappedAttachment(analyzer, ability, suffix, context);
        if (candidate) return { candidate, rules: ["108.4", "603.2c", "608.2h"] };
      }
    }
  }
}

/** The step at `path` whose `what` this selection is ("reveal a card from
 * among them", "you choose a card from it", "put one of them …"). */
function pickStep(selectionPath: string): string | undefined {
  return /^(.*\/steps\/\d+)\/what$/.exec(selectionPath)?.[1];
}
/** The event an instruction at `step` performed. */
function stepEvent(analyzer: Analyzer, step: string): Extract<BindingNode, { node: "event" }> | undefined {
  const node = analyzer.result.nodes.find(entry => entry.node === "event" && entry.path === step);
  return node?.node === "event" ? node : undefined;
}
/** The selections of `scope` that pick "from among" an established pool
 * (CR 608.2c): a filtered or "one of them" selection whose `fromAmong` or
 * `pool` is bound. `pool` narrows them to the picks from that pool. */
export function poolPicks(this: Analyzer, scope: string, pool?: string): { id: string; path: string; pool: string; cardinality: BindingCardinality }[] {
  return this.result.nodes.flatMap(node => {
    if (node.node !== "selection" || node.scope !== scope || node.selection === "target" || !("ref" in node.expression)) return [];
    const ref = node.expression.ref;
    const at = ref === "filter" || ref === "each" || ref === "all" ? `${node.path}/filter/fromAmong` : ref === "from-pool" ? `${node.path}/pool` : undefined;
    const from = at ? this.at(at) : undefined;
    return from && from.domain === "object" && (pool === undefined || from.id === pool)
      ? [{ id: node.id, path: node.path, pool: from.id, cardinality: node.cardinality }] : [];
  });
}
/** Every pick "from among" a pool or "one of them" the ability at `path` has
 * attempted so far, bound or not: the instructions whose `fromAmong` or
 * `pool` operand was walked. An unresolved pick still takes cards from its
 * pool, so a remainder or pronoun cannot be proven past it. */
function attemptedPicks(analyzer: Analyzer, path: string): string[] {
  const root = /^(.*\/abilities\/\d+)\//.exec(path)?.[1];
  if (!root) return [];
  return [...new Set(analyzer.result.references.flatMap(reference =>
    reference.path.startsWith(`${root}/`) && /\/(?:fromAmong|pool)$/.test(reference.path) ? [reference.path] : []))];
}
/** A singular pronoun after a pick "from among" a pool: "You may reveal an
 * Elf card from among them and put it into your hand", "You choose a
 * nonland card from it. That player discards that card." The pool itself is
 * plural, so the one pick (or the object it became, CR 400.7j) is the only
 * object the pronoun can mean (CR 608.2c). Bound only when the remaining
 * candidates are exactly that pool and that pick, the pick is the only pick
 * the ability has attempted from any pool, and the pronoun is inside the pick's own optional group or
 * sentence; a `choose-object` pick ("you choose … that card") is meant by any
 * later pronoun of the ability. */
export function poolPickAntecedent(this: Analyzer, expression: ObjectRef, path: string, candidates: Candidate[], context: Context): Candidate | undefined {
  // "From it" names a set to pick from, never an earlier pick.
  if (objectCardinality(expression) !== "one" || candidates.length !== 2 || /\/(?:fromAmong|pool)$/.test(path)) return;
  const attempted = attemptedPicks(this, path);
  for (const [pool, picked] of [[candidates[0], candidates[1]], [candidates[1], candidates[0]]]) {
    if (pool.domain !== "object" || pool.cardinality === "one" || picked.domain !== "object" || picked.cardinality !== "one") continue;
    const picks = this.poolPicks(context.ability, pool.id);
    const [pick] = picks;
    if (picks.length !== 1 || pick.cardinality !== "one" || pick.id !== (picked.successorOf ?? picked.id) ||
        attempted.length !== 1 || !attempted[0].startsWith(`${pick.path}/`)) continue;
    const step = pickStep(pick.path);
    const event = step ? stepEvent(this, step) : undefined;
    if (!step || !event || path.startsWith(`${step}/`)) continue;
    const container = /^(.*)\/steps\/\d+$/.exec(step)?.[1];
    const ability = /^(.*\/abilities\/\d+)\//.exec(step)?.[1];
    const scope = event.operation === "choose-object" ? ability : container;
    // A card picked from among cards is a card ("that card").
    if (scope && path.startsWith(`${scope}/`))
      return picked.description || picked.successorOf || !pool.description ? picked : { ...picked, description: pool.description };
  }
  return undefined;
}
/** "From it" after "target opponent reveals their hand": the revealed or
 * looked-at hand as a set of cards (CR 400.2, 701.20a). Only a whole hand
 * named by the instruction just performed on it, revealed by its owner or
 * looked at, establishes this zone pronoun. */
export function revealedHand(this: Analyzer, expression: ObjectRef, path: string, candidates: Candidate[]): Candidate | undefined {
  if (!isDeepStrictEqual(expression, { ref: "it" }) || !path.endsWith("/filter/fromAmong") || candidates.length !== 1) return;
  const [hand] = candidates;
  const node = this.result.nodes.find(entry => entry.id === hand.id);
  if (node?.node !== "selection" || node.selection !== "evaluate-reference" || !("ref" in node.expression)) return;
  // "Their hand" is either all cards in that zone or the zone's cards.
  const shown = node.expression;
  const zone = shown.ref === "all" && Object.keys(shown).length === 2 &&
    Object.keys(shown.filter).length === 2 && shown.filter.cls === "card" ? shown.filter.zone : undefined;
  const ownerPath = zone?.zone === "hand" && Object.keys(zone).length === 2 ? `${node.path}/filter/zone/owner`
    : shown.ref === "zone-cards" && shown.zone === "hand" && Object.keys(shown).length === 3 ? `${node.path}/owner` : undefined;
  if (!ownerPath) return;
  const step = pickStep(node.path);
  const event = step ? stepEvent(this, step) : undefined;
  const owner = this.at(ownerPath);
  if (!step || !event || !owner || owner.domain !== "player" || owner.cardinality !== "one" || event.optional || event.repeated) return;
  if (event.operation === "reveal" ? this.at(`${step}/who`)?.id !== owner.id : event.operation !== "look-at") return;
  return hand;
}
/** Instructions that cannot take a card out of a pool of looked-at,
 * revealed, milled, searched or exiled cards (they move nothing). */
const POOL_PRESERVING = new Set(["reveal", "look-at", "choose-object", "gain-life", "lose-life", "damage", "pump", "set-pt",
  "create-token", "put-counters", "tap", "untap", "add-mana", "gain-abilities", "become", "choose-value", "player-gain-abilities"]);
/** Instructions that move only their printed `what` operand(s). */
const OPERAND_MOVES = new Set(["move-zone", "exile", "return-to-hand", "sacrifice", "destroy", "shuffle-into-library", "attach"]);
/** Instructions with no object operand that take cards only from the library. */
const LIBRARY_ONLY = new Set(["draw", "mill", "scry", "surveil"]);
/** Containers whose nested instructions are walked; any other instruction
 * holding nested instructions is not on the allowlist. */
const CONTAINERS = new Set(["optional-group", "simultaneous", "modal", "roll-dice", "flip-coin"]);
/** Keys under which sibling paths are mutually exclusive alternatives. */
const ALTERNATIVES = new Set(["options", "rows", "outcomes"]);

/** The value at a JSON pointer below `root`. */
function pointer(root: unknown, segments: string[]): unknown {
  let value = root;
  for (const segment of segments) value = isRecord(value) || Array.isArray(value) ? (value as Record<string, unknown>)[segment] : undefined;
  return value;
}
/** Two instruction paths that cannot both be performed in one resolution:
 * different modes, die rows or coin outcomes, or a sentence's steps and its
 * otherwise branch (CR 608.2c). */
function exclusive(a: string, b: string): boolean {
  const x = a.split("/"), y = b.split("/");
  let common = 0;
  while (common < x.length && common < y.length && x[common] === y[common]) common++;
  if (common === x.length || common === y.length) return false;
  if (ALTERNATIVES.has(x[common - 1])) return true;
  const pair = [x[common], y[common]].sort().join(" ");
  return pair === "otherwise steps" || pair === "instead original";
}
/** Printed order of two instruction paths that are not exclusive. */
function before(a: string, b: string): boolean {
  const x = a.split("/"), y = b.split("/");
  for (let index = 0; index < Math.min(x.length, y.length); index++) {
    if (x[index] === y[index]) continue;
    const m = Number(x[index]), n = Number(y[index]);
    return Number.isInteger(m) && Number.isInteger(n) ? m < n : x[index] < y[index];
  }
  return x.length < y.length;
}
/** Every printed instruction of an ability, by path. */
function instructions(value: unknown, path: string, out: { path: string; effect: Record<string, unknown> }[] = []) {
  if (Array.isArray(value)) value.forEach((entry, index) => instructions(entry, `${path}/${index}`, out));
  else if (isRecord(value)) {
    if (typeof value.effect === "string" && /\/steps\/\d+$/.test(path)) out.push({ path, effect: value });
    for (const [key, entry] of Object.entries(value)) if (key !== "trigger" && key !== "costs") instructions(entry, `${path}/${key}`, out);
  }
  return out;
}
/** Every zone a selection can take cards from, or undefined when one of its
 * arms names none (a permanent or spell has no zone and no `card` class). */
function selectionZones(expression: unknown): string[] | undefined {
  if (!isRecord(expression)) return undefined;
  if (expression.ref === "library-cards") return ["library"];
  if (expression.ref === "zone-cards") return typeof expression.zone === "string" ? [expression.zone] : undefined;
  if (!["filter", "each", "all", "target"].includes(String(expression.ref)) || !isRecord(expression.filter)) return undefined;
  const arms = (filter: Record<string, unknown>): string[] | undefined => {
    const own = isRecord(filter.zone) && typeof filter.zone.zone === "string" ? filter.zone.zone : undefined;
    const nested = ["anyOf", "allOf", "oneOf"].flatMap(key => Array.isArray(filter[key]) ? filter[key] as unknown[] : []);
    if (own) return nested.length ? undefined : [own];
    if (!nested.length) return JSON.stringify(filter).includes('"cls":"card"') || filter.fromAmong ? undefined : ["battlefield"];
    const zones = nested.map(arm => isRecord(arm) ? arms(arm) : undefined);
    return zones.every(zone => zone) ? zones.flat() as string[] : undefined;
  };
  return arms(expression.filter);
}
/** The zone the pool's cards are in, from the printed instruction that
 * established it. */
function poolZone(operation: string, step: Record<string, unknown>, selection: unknown): string | undefined {
  if (operation === "mill") return "graveyard";
  if (operation === "exile") return "exile";
  if (operation === "scan-library") return "library";
  if (operation === "look-at" && isRecord(step.what) && isRecord(step.what.of) && !("ref" in step.what)) return String(step.what.of.zone);
  if (operation === "search" && isRecord(step.zones) && Array.isArray(step.zones.of) && step.zones.of.length === 1 && isRecord(step.zones.of[0]))
    return String(step.zones.of[0].zone);
  const zones = selectionZones(selection);
  return zones?.length === 1 && zones[0] !== "battlefield" ? zones[0] : undefined;
}

/** "The rest" (CR 608.2c): the one pool that can reach this reference, less
 * its one pick. Bound only when all of these hold:
 * - the ability attempted exactly one pick from among any pool, on any path,
 *   bound or not, and that pick is bound to this pool;
 * - the pool still reaches the reference and is the only multi-object set
 *   that can be meant (a set still established here, one made under a
 *   condition, or a reveal-until scan's revealed cards competes);
 * - the pool's actor and owner are bound single players, the same player
 *   for a reveal, and it is either "Put one of them … and the rest …" in one
 *   sentence or the ability controller's own look, reveal or mill;
 * - every printed instruction between the pool and this reference, on any
 *   path that can reach it (modes, die rows, coin outcomes and otherwise
 *   branches are exclusive), is on the allowlist: it emitted its event
 *   without a diagnostic and moves nothing, moves only the pick (or what it
 *   became), the source or this same remainder, or takes cards only from
 *   other zones, every arm of a compound selection included.
 * The pick's own "may", "up to" or "any number" (not a choice that also
 * skips the rest) leaves the whole pool. */
export function pickRemainder(this: Analyzer, expression: Extract<ObjectRef, { ref: "rest" }>, path: string, context: Context): Candidate | undefined {
  const attempted = attemptedPicks(this, path);
  const picks = this.poolPicks(context.ability);
  if (expression.exclude || attempted.length !== 1 || picks.length !== 1 || !attempted[0].startsWith(`${picks[0].path}/`)) return;
  const [pick] = picks;
  const pool = context.objects.find(candidate => candidate.id === pick.pool);
  if (!pool || pool.domain !== "object" || pool.cardinality === "one" || this.invalidNodes.has(pool.id) ||
      (expression.of && this.at(child(path, "of"))?.id !== pool.id)) return;
  const step = pickStep(pick.path);
  const rest = /^(.*\/steps\/\d+)\/what(?:\/\d+)?$/.exec(path)?.[1];
  const root = /^(.*\/abilities\/\d+)\//.exec(path)?.[1];
  const face = root ? [...this.abilityAsts.keys()].find(at => root === at || root.startsWith(`${at}/`)) : undefined;
  if (!step || !rest || !root || !face) return;
  const ability = pointer(this.abilityAsts.get(face), root.slice(face.length).split("/").filter(Boolean));
  const inAbility = (node: BindingNode) => node.path.startsWith(`${root}/`);
  // Competing sets.
  const reaching = new Set(context.objects.map(candidate => candidate.id));
  const conditional = (eventId: string) => this.result.nodes.some(entry => entry.id === eventId && entry.node === "event" && entry.conditional);
  const scanned = new Set(this.result.nodes.flatMap(node => node.node === "library-scan" && inAbility(node) ? [node.pool] : []));
  const sets = this.result.nodes.filter(node => inAbility(node) && "cardinality" in node && node.cardinality !== "one" &&
    (node.node === "event-result" ? node.result === "objects" && node.path !== step && !node.path.startsWith(`${step}/`) &&
        (reaching.has(node.id) || conditional(node.event) || scanned.has(node.id))
      : node.node === "selection" && node.path !== pick.path && /\/steps\/\d+\/(?:what|for)(?:\/\d+)?$/.test(node.path) &&
        (reaching.has(node.id) || conditional(this.id(node.path.replace(/\/(?:what|for)(?:\/\d+)?$/, ""), "event")))));
  if (sets.length !== 1 || sets[0].id !== pool.id) return;
  const node = sets[0];
  // The instruction that established the pool: its actor, owner and zone.
  const origin = node.node === "event-result" ? node.path
    : node.node === "selection" ? /^(.*\/steps\/\d+)\/(?:what|for)(?:\/\d+)?$/.exec(node.path)?.[1] : undefined;
  const event = origin ? stepEvent(this, origin) : undefined;
  const originStep = origin ? pointer(ability, origin.slice(root.length).split("/").filter(Boolean)) : undefined;
  if (!origin || !event || event.repeated || !isRecord(originStep)) return;
  const actor = this.at(`${origin}/who`);
  const owner = [`${origin}/what/of/owner`, `${origin}/what/owner`, `${origin}/what/filter/zone/owner`, `${origin}/for/filter/zone/owner`]
    .map(at => this.at(at)).find(candidate => candidate);
  const single = (player: Candidate | undefined) => player?.domain === "player" && player.cardinality === "one";
  if ((actor && !single(actor)) || (owner && !single(owner)) ||
      (event.operation === "reveal" && actor && owner && actor.id !== owner.id)) return;
  const sameSentence = /\/pool$/.test(attempted[0]) && path.startsWith(`${/^(.*\/effects\/\d+)\//.exec(pick.path)?.[1]}/`);
  const ownLibrary = ["look-at", "reveal", "mill"].includes(event.operation) && actor?.id === context.you &&
    (owner ? owner.id === context.you : event.operation === "mill");
  if (!sameSentence && !ownLibrary) return;
  const zone = poolZone(event.operation, originStep, node.node === "selection" ? node.expression : undefined);
  if (ownLibrary && zone !== "library" && zone !== "graveyard") return;
  // Everything the pick became (CR 400.7j).
  const derived = new Set([pick.id]);
  for (const later of this.result.nodes) if (later.node === "event-result" && inAbility(later) &&
    (later.path === step || this.result.nodes.some(entry => entry.id === later.event && entry.node === "event" && entry.inputs.some(id => derived.has(id)))))
    derived.add(later.id);
  // The printed instructions between the pool and this reference.
  for (const { path: at, effect } of instructions(ability, root)) {
    if (at === origin || at === step || at === rest || !before(origin, at) || !before(at, rest) ||
        exclusive(at, rest) || exclusive(at, origin)) continue;
    const operation = String(effect.effect);
    // Die rolls and coin flips are events themselves (their rows and
    // outcomes are walked as exclusive paths); the other containers are not.
    if (operation === "roll-dice" || operation === "flip-coin") {
      if (!stepEvent(this, at) || this.result.diagnostics.some(diagnostic => diagnostic.path === at)) return;
      continue;
    }
    if (CONTAINERS.has(operation)) continue;
    const performed = stepEvent(this, at);
    if (!performed || this.result.diagnostics.some(diagnostic => diagnostic.path === at || diagnostic.path.startsWith(`${at}/`))) return;
    if (POOL_PRESERVING.has(operation)) continue;
    if (LIBRARY_ONLY.has(operation)) { if (zone === "library" || !zone) return; continue; }
    if (operation === "search") {
      const searched = isRecord(effect.zones) && Array.isArray(effect.zones.of) ? effect.zones.of.map(entry => isRecord(entry) ? entry.zone : undefined) : [];
      if (!zone || !searched.length || searched.some(entry => entry === zone || entry === undefined)) return;
      continue;
    }
    if (!OPERAND_MOVES.has(operation)) return;
    const operands = Array.isArray(effect.what) ? effect.what.map((entry, index) => ({ expression: entry, at: `${at}/what/${index}` }))
      : [{ expression: effect.what, at: `${at}/what` }];
    for (const operand of operands) {
      const target = this.at(operand.at);
      const targetNode = target ? this.result.nodes.find(entry => entry.id === target.id) : undefined;
      if (target && (derived.has(target.id) || targetNode?.node === "source" ||
          (targetNode?.node === "remainder" && targetNode.pool === pool.id && targetNode.excluding === pick.id))) continue;
      const zones = selectionZones(operand.expression);
      if (!zone || !zones || zones.includes(zone)) return;
    }
  }
  const rules = sameSentence ? ["608.2c"] : ["608.2c", ...(event.operation === "reveal" ? ["701.20a"] : [])];
  const id = this.node({ id: this.id(path, "remainder"), path, node: "remainder", scope: context.ability,
    pool: pool.id, excluding: pick.id, rules });
  // The pick's own choice to take nothing: its own "may" or an optional
  // group that does not also hold this reference, "up to", "any number".
  const pickAst = pointer(ability, step.slice(root.length).split("/").filter(Boolean));
  const group = /^(.*\/steps\/\d+)\/steps\/\d+$/.exec(step)?.[1];
  const groupAst = group ? pointer(ability, group.slice(root.length).split("/").filter(Boolean)) : undefined;
  const pickNode = this.result.nodes.find(entry => entry.id === pick.id);
  const chosen = pickNode?.node === "selection" && "ref" in pickNode.expression ? pickNode.expression : undefined;
  const count = chosen && "count" in chosen ? chosen.count : undefined;
  const mayTakeNothing = (isRecord(pickAst) && pickAst.optional === true) ||
    (group && isRecord(groupAst) && groupAst.effect === "optional-group" && !path.startsWith(`${group}/`)) ||
    (chosen && "upTo" in chosen && chosen.upTo) || (count?.amount === "chosen" && !count.minimum);
  if (ownLibrary && mayTakeNothing)
    this.engineFeature("optional-look-remainder", path, "Use all remaining looked-at cards. When the choice was declined, use the full pool.", rules);
  return this.bound(expression, path, id, rules);
}
export function selection(this: Analyzer, expression: ObjectRef | PlayerRef, path: string, context: Context, targeted: boolean): Candidate | undefined {
  if (targeted && context.phase !== "resolution") {
    this.unresolved(expression, path, "unsupported-scope", "Target declarations and target predicates outside a resolving ability need a specific announcement scope.", ["601.2c", "602.2b"]);
    return undefined;
  }
  const selectionId = "selectionId" in expression && typeof expression.selectionId === "string" ? expression.selectionId : undefined;
  const id = targeted && selectionId ? `${context.targetScope ?? context.ability}/targets/${encodeURIComponent(selectionId)}` : this.id(path, targeted ? "target" : "query");
  const cardinality = "ref" in expression ? objectCardinality(expression)
    : isPluralPlayerRef(expression) ? "many" : "one";
  if (targeted && !selectionId) {
    this.unresolved(expression, path, "missing-target-identity",
      "A target requires its parser-minted selection identity; structural equality and proximity cannot identify it.", ["601.2c"]);
    return undefined;
  }
  let countChoice: string | undefined;
  if (targeted) {
    const previous = this.targets.get(id);
    const signature = JSON.stringify(expression);
    if (previous) {
      if (previous.signature !== signature) {
        this.invalidNodes.set(id, "ambiguous-target-identity");
        this.unresolved(expression, path, "ambiguous-target-identity", "The same target identity carries conflicting selectors.", ["601.2c"], [id]);
        this.diagnostic("ambiguous-target-identity", previous.path, "The same target identity carries conflicting selectors.", ["601.2c"]);
        return undefined;
      }
      return this.bound(expression, path, id, ["601.2c"]);
    }
    this.targets.set(id, { id, path, signature });
    const count = this.boundTargetCount(expression, context);
    if (count) {
      const scope = context.targetScope ?? context.ability;
      const announcing = this.result.nodes.find(node => node.id === scope);
      if (announcing?.node === "ability") {
        const at = child(path, "count");
        countChoice = this.node({ id: `${id}/count`, path: at, node: "target-count", scope, chooser: announcing.you,
          ...count, rules: ["115.3", "601.2c", "602.2b", "603.3d"] });
        if (count.maximumNumber) this.bound({ amount: "x" }, at, count.maximumNumber, ["107.3a", "601.2b", "601.2c"]);
      } else this.diagnostic("unsupported-number-binding", child(path, "count"),
        "The target count requires its announcing ability.", ["601.2c"]);
    }
  }
  const resolution = this.boundResolutionCount(expression, path, context);
  if (resolution && context.resolutionCount) {
    const at = child(path, "count");
    const operation = context.resolutionCount.operation;
    countChoice = this.node({ id: this.id(at, "resolution-count"), path: at, node: "resolution-count",
      scope: context.ability, chooser: context.you, ...resolution, operation,
      rules: operation === "search" ? ["608.2d", "701.23b"] : ["608.2c", "608.2d"] });
  }
  const predicateRules = context.eventPredicate ? ["603.2c"] : context.phase === "replacement-match" ? ["614.1a"] : undefined;
  const rules = targeted ? ["601.2c"] : predicateRules ?? ["608.2h"];
  this.node({ id, path, node: "selection", scope: context.ability, expression, cardinality,
    ...(context.directCreatureSpellCopy?.what === expression ? { chooser: context.you } : {}),
    ...(countChoice ? { countChoice } : {}),
    ...(targeted && context.announcementMember ? { perMember: context.announcementMember } : {}),
    timing: targeted ? "stack-announcement" : context.targetLegalityQuery ? "target-legality"
      : context.exchangeEntrant && context.announcementOperand ? "stack-announcement" : context.phase,
    selection: targeted ? "target" : resolution ? "resolution-choice" : predicateRules ? "event-participant-filter" : "evaluate-reference", rules });
  const candidate = this.bound(expression, path, id, rules);
  return { ...candidate, cardinality };
}
export function at(this: Analyzer, path: string): Candidate | undefined {
  let binding: ReferenceBinding | undefined;
  for (let index = this.result.references.length - 1; index >= 0; index--) {
    if (this.result.references[index].path === path) { binding = this.result.references[index]; break; }
  }
  if (binding?.binding !== "bound") return undefined;
  const node = this.result.nodes.find(node => node.id === binding.target);
  const cardinality = node && "cardinality" in node ? node.cardinality
    : referenceCardinality(binding.expression);
  const established: ObjectFilter | undefined = node?.node === "distinct-library-search" ? { cls: "card" }
    : node?.node === "event-result" ? node.description
    : node?.node === "selection" && "ref" in node.expression ? description(node.expression)
    : "ref" in binding.expression && binding.expression.ref !== "affected-object" && binding.expression.ref !== "those-counters" ? description(binding.expression) : undefined;
  return { id: binding.target, cardinality, domain: domain(binding.expression), description: established };
}
/** The one object or group an "exile ... until" instruction at `path` exiled. */
export function exiledUntil(this: Analyzer, path: string): Candidate | undefined {
  const what = this.at(`${path}/what/0`);
  return what?.domain === "object" && !this.at(`${path}/what/1`) ? what : undefined;
}
/** "Each permanent destroyed this way": the one earlier instruction of this
 * ability, in a branch visible from here, that performed that action. */
export function thisWay(this: Analyzer, relation: Record<string, unknown>, path: string, context: Context): boolean {
  const actions: Record<string, string[]> = { exile: ["exile"], sacrifice: ["sacrifice"], discard: ["discard"], reveal: ["reveal"], destroy: ["destroy"],
    tap: ["tap"], return: ["return-to-hand"], counter: ["counter"], mill: ["mill"], draw: ["draw"], cast: ["play-card"] };
  const operations = actions[String(relation.event)];
  const root = /^(.*\/abilities\/\d+)\//.exec(path)?.[1];
  if (!operations || !root || context.barrier) return false;
  const branches = new Set(["options", "outcomes", "rows", "otherwise", "instead", "original"]);
  const visible = (eventPath: string) => {
    const a = eventPath.split("/"), b = path.split("/");
    let common = 0;
    while (common < a.length && common < b.length && a[common] === b[common]) common++;
    // A sibling mode, outcome or branch diverges right after its container.
    return !branches.has(a[common - 1]) && !a.slice(common).some(segment => branches.has(segment));
  };
  const events = this.result.nodes.filter(node => node.node === "event" && operations.includes(node.operation)
    && node.path.startsWith(`${root}/`) && visible(node.path));
  if (events.length !== 1) return false;
  this.node({ id: this.id(path, "event-link"), path, node: "event-link", scope: context.ability, action: String(relation.event), event: events[0].id, rules: ["608.2c"] });
  return true;
}
/** A target is announced with its spell or ability (CR 601.2c), whichever
 * branch of it names the target. */
export function announced(this: Analyzer, candidate: Candidate): boolean {
  const node = this.result.nodes.find(entry => entry.id === candidate.id);
  return node?.node === "selection" && node.timing === "stack-announcement";
}
/** An intervening "if" clause (CR 603.4) or a static ability's "as long as"
 * condition that names the source or what it is attached to makes that
 * object an antecedent of the effect: "if this creature has defender, it
 * loses defender", "if enchanted permanent is tapped, untap it". */
export function namingCondition(this: Analyzer, condition: unknown, path: string, context: Context): void {
  const start = this.result.references.length;
  const before = context.objects.map(candidate => candidate.id);
  this.walk(condition, path, context);
  // Only the clause's subjects; not a possessive ("Drizzt's power") or a
  // filter's exclusion ("another snow permanent").
  const named = this.result.references.slice(start).flatMap(reference => {
    if (reference.binding !== "bound" || !("ref" in reference.expression)) return [];
    if (/\/(of|filter|other)(\/|$)/.test(reference.path.slice(path.length))) return [];
    const ref = reference.expression.ref;
    if (!(ref === "self" || (ref === "this" && reference.expression.noun !== "ability") || ref === "equipped" || ref === "enchanted" || ref === "fortified")) return [];
    const candidate = this.at(reference.path);
    return candidate?.domain === "object" ? [candidate] : [];
  });
  if (!named.length) return;
  context.objects = combine(context.objects, named);
  context.named = { ids: named.map(candidate => candidate.id), before };
  if (named.length === 1) context.conditionSubject = { candidate: named[0], before: [...before, ...(context.departed ?? []).map(candidate => candidate.id)] };
  if (named.some(candidate => candidate.id !== context.source)) context.mentionedObject = true;
}
