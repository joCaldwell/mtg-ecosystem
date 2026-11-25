// Antecedent and pronoun resolution for object references.
import { isDeepStrictEqual } from "node:util";
import type { ObjectFilter, ObjectRef, PlayerRef } from "../../ast.ts";
import { isPluralPlayerRef } from "../../parser/refs.ts";
import type { BindingCardinality, ReferenceBinding, ReferenceExpression } from "../types.ts";
import type { Analyzer } from "./analyzer.ts";
import { type ChoiceReference } from "./choices.ts";
import { ENTRY_CHOICE_RULES, child, combine, describes, description, domain, objectCardinality, referenceCardinality, type Candidate, type Context } from "./context.ts";
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

      // "Put one of them into your hand and the rest into your graveyard":
      // the pool the same sentence's selection chose from, less that choice.
      const sentence = /^(.*\/effects\/\d+)\//.exec(path)?.[1];
      const chosen = this.poolSelections.get(context.ability);
      const pool = expression.of ? this.at(child(path, "of")) : chosen?.pool;
      if (chosen && pool && sentence && chosen.path.startsWith(`${sentence}/`) && chosen.pool.id === pool.id) {
        const id = this.node({ id: this.id(path, "remainder"), path, node: "remainder", scope: context.ability, pool: pool.id,
          excluding: chosen.selected.id, rules: ["608.2c"] });
        return this.bound(expression, path, id, ["608.2c"]);
      }
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
