// Number bindings: target counts, damage amounts and dynamic values.
import { isDeepStrictEqual } from "node:util";
import type { Ability, Amount, CharacteristicRelation, Condition, Effect, ObjectRef, PlayerRef } from "../../ast.ts";
import type { BindingNode } from "../types.ts";
import type { Analyzer } from "./analyzer.ts";
import { chosenRange } from "./choices.ts";
import { child, type Candidate, type Context } from "./context.ts";
import { costHasX, definesX, isRecord } from "./predicates.ts";

/** Only a printed face ability can establish these quantities (CR 113.10). */
function printedNumberScope(analyzer: Analyzer, path: string, context: Context) {
  const root = context.lexicalAbilityPath;
  const face = root && /^(\/faces\/\d+)\/abilities\/\d+$/.exec(root)?.[1];
  if (!root || !face || !path.startsWith(`${root}/`) || context.barrier ||
      context.ability !== analyzer.id(root, "ability") || context.face !== analyzer.id(face, "face") ||
      context.source !== analyzer.id("/source", "object")) return;
  const ability = analyzer.abilityAsts.get(root) as Ability | undefined;
  if (ability) return { root, face, ability };
}

function faceAbilities(analyzer: Analyzer, face: string) {
  return [...analyzer.abilityAsts].flatMap(([path, ability]) =>
    path.startsWith(`${face}/abilities/`) ? [{ path, ability: ability as Ability }] : []);
}

/** CR 107.3a, 601.2b, 702.180a: Harmonize's alternative cost uses the
 * spell's announced X. This covers a single printed X alternative to a
 * spell whose mana cost already establishes X. */
function harmonizeX(analyzer: Analyzer, path: string, context: Context): boolean {
  if (context.phase !== "payment") return false;
  const scope = printedNumberScope(analyzer, path, context);
  if (scope?.ability.kind !== "keywords") return false;
  const index = /^\/keywords\/(\d+)\/cost\/0\/symbols\/0$/.exec(path.slice(scope.root.length))?.[1];
  if (index === undefined) return false;
  const keyword = scope.ability.keywords[Number(index)];
  if (keyword?.keyword !== "harmonize" || !("cost" in keyword) || keyword.cost?.length !== 1) return false;
  const [cost] = keyword.cost;
  if (cost.cost !== "mana" || cost.symbols[0] !== "X" ||
      !cost.symbols.slice(1).every(symbol => /^(?:\d+|[WUBRGC])$/.test(symbol)) ||
      !isDeepStrictEqual(keyword, { keyword: "harmonize", cost: [{ cost: "mana", symbols: cost.symbols }] })) return false;
  const face = analyzer.result.nodes.find(node => node.id === context.face);
  if (face?.node !== "face" || !face.manaCost?.includes("{X}")) return false;
  const abilities = faceAbilities(analyzer, scope.face);
  const spells = abilities.filter(({ ability }) => ability.kind === "spell");
  if (spells.length !== 1 || definesX(spells[0].ability) ||
      abilities.flatMap(({ ability }) => ability.kind === "keywords" ? ability.keywords : [])
        .filter(candidate => candidate.keyword === "harmonize" || costHasX(candidate)).length !== 1 ||
      abilities.some(({ ability }) => ability.kind === "additional-cost" && costHasX(ability.costs))) return false;
  const rules = ["107.3a", "601.2b", "702.180a"];
  // The printed spell block owns this node even when it follows the keyword.
  analyzer.bound({ amount: "x" }, path, analyzer.id(spells[0].path, "announced-x"), rules);
  return true;
}

/** CR 601.2f–i, 608.2c: actual mana paid for this casting is available
 * during resolution. Allow only a definition used by direct resolution
 * amounts; a target or other announcement operand cannot depend on it. */
function ownCastColors(analyzer: Analyzer, quantity: Extract<Amount, { amount: "mana-spent" }>,
  path: string, context: Context): boolean {
  const what = { ref: "this", noun: "spell" } as const;
  if (context.phase !== "resolution" || context.announcementOperand || context.sourceMoved ||
      !isDeepStrictEqual(quantity, { amount: "mana-spent", what, quantity: "distinct-colors" })) return false;
  const scope = printedNumberScope(analyzer, path, context);
  if (scope?.ability.kind !== "spell" || scope.ability.effects.length !== 1 || analyzer.xFaces.has(context.face)) return false;
  const abilities = faceAbilities(analyzer, scope.face);
  if (abilities.filter(({ ability }) => ability.kind === "spell").length !== 1 ||
      abilities.some(({ ability }) => {
        if (ability.kind === "keywords") return costHasX(ability.keywords);
        return ability.kind === "static" && (ability.effect.static === "alternative-cost" || ability.effect.static === "play-permission") &&
          costHasX(ability.effect);
      })) return false;
  const [sentence] = scope.ability.effects;
  const definition = `${scope.root}/effects/0/bindings/0`;
  if (path !== child(definition, "value") || context.x !== analyzer.id(definition, "number") ||
      !sentence.steps.length || !sentence.steps.every(step =>
        (step.effect === "draw" || step.effect === "lose-life" || step.effect === "gain-life" || step.effect === "scry") &&
        isDeepStrictEqual(step, { effect: step.effect, who: { player: "you" }, amount: { amount: "x" } })) ||
      !isDeepStrictEqual(sentence, { steps: sentence.steps, bindings: [{ variable: "x", value: quantity }] })) return false;
  const rules = ["105.1", "601.2f", "601.2h", "601.2i", "608.2c", "707.10"];
  analyzer.bound(what, child(path, "what"), context.source, rules);
  analyzer.engineFeature("event-history", path,
    "Count the distinct colors among mana actually spent on this cast, including additional costs; a stack copy has no cast payment.", rules);
  return true;
}

/** CR 400.7d, 614.12, 702.33c–e: this entry replacement reads the
 * originating spell's payment count for its one printed multikicker. */
function entryKickerCount(analyzer: Analyzer, quantity: Extract<Amount, { amount: "kicker-count" }>,
  path: string, context: Context): boolean {
  if (context.phase !== "continuous" || context.announcementOperand || context.sourceMoved || !context.permanentSource) return false;
  const scope = printedNumberScope(analyzer, path, context);
  if (scope?.ability.kind !== "static" || scope.ability.effect.static !== "enters-with-counters" ||
      path !== `${scope.root}/effect/multiplier`) return false;
  const entry = scope.ability.effect;
  const what = entry.what;
  const types = analyzer.faceTypes.get(context.face)?.types;
  const self = what.ref === "self" && isDeepStrictEqual(what, { ref: "self" });
  const thisPermanent = what.ref === "this" && (what.noun === "permanent" ||
    types?.some(type => type === what.noun)) && isDeepStrictEqual(what, { ref: "this", noun: what.noun });
  if ((!self && !thisPermanent) ||
      !isDeepStrictEqual(quantity, { amount: "kicker-count", of: what }) ||
      !isDeepStrictEqual(entry, { static: "enters-with-counters", what, counter: entry.counter,
        count: { amount: "fixed", value: 1 }, multiplier: quantity })) return false;
  const kickers = faceAbilities(analyzer, scope.face).flatMap(({ ability }) => ability.kind === "keywords" ? ability.keywords : [])
    .filter(keyword => keyword.keyword === "kicker" || keyword.keyword === "multikicker");
  if (kickers.length !== 1 || kickers[0].keyword !== "multikicker") return false;
  const rules = ["400.7d", "614.12", "702.33c", "702.33d", "702.33e"];
  analyzer.bound(quantity.of, child(path, "of"), context.source, rules);
  analyzer.engineFeature("event-history", path,
    "Read the count of that permanent's one printed multikicker from the spell that resolved into it.", rules);
  return true;
}

export function bindPrintedQuantity(this: Analyzer,
  quantity: Extract<Amount, { amount: "x" | "mana-spent" | "kicker-count" }>, path: string, context: Context): boolean {
  switch (quantity.amount) {
    case "x": return harmonizeX(this, path, context);
    case "mana-spent": return ownCastColors(this, quantity, path, context);
    case "kicker-count": return entryKickerCount(this, quantity, path, context);
  }
}

/** CR 601.2b-c and 603.3d: only quantities from the matching trigger
 * exist when its modes and targets are announced. */
export function triggerQuantity(this: Analyzer, id: string): boolean {
  const quantity = this.result.nodes.find(node => node.id === id);
  if (quantity?.node !== "event-result" || quantity.result !== "quantity") return false;
  return this.result.nodes.some(node => node.id === quantity.event && node.node === "event" &&
    node.operation.startsWith("trigger:"));
}
export function characteristicPredicate(this: Analyzer, test: CharacteristicRelation, path: string, subject: string, context: Context): void {
  let inner = test;
  let at = path;
  while (inner.relation === "not") { inner = inner.of; at = child(at, "of"); }
  if (inner.relation === "shared-value" && inner.among) {
    this.diagnostic("unsupported-scope", at, "The comparison needs a bound preceding set of named types.", ["205.2", "608.2c"]);
    return;
  }
  const operand = inner.relation === "shared-value" ? inner.with : inner.from;
  const operandPath = child(at, inner.relation === "shared-value" ? "with" : "from");
  const knownTiming = context.targetLegalityQuery ? "target-legality" : context.interveningIf ? "intervening-if"
    : context.announcementOperand ? "stack-announcement" : context.phase;
  const rules = ["105.2", "201.2a", "201.2c", "205.2", "205.3", "608.2h", ...(context.targetLegalityQuery ? ["601.2c", "608.2b"] : []), ...(context.interveningIf ? ["603.4"] : [])];
  const scoped = { ...context, characteristicSubject: undefined };
  let binding: Extract<BindingNode, { node: "characteristic-predicate" }>["operand"];
  if (operand.operand === "matching-set") {
    const atFilter = child(operandPath, "filter");
    this.walk(operand.filter, atFilter, inner.relation === "shared-value"
      ? { ...scoped, comparisonExclusion: { path: atFilter, subject } } : scoped);
    binding = { kind: "matching-set", filter: operand.filter, quantifier: operand.quantifier };
  } else if (inner.relation === "shared-value" && inner.characteristic === "name" &&
      context.linkedExile?.imprint && context.lexicalAbilityPath !== context.linkedExile.ability &&
      !context.sourceMoved && operand.what.ref === "related" && isDeepStrictEqual(operand.what.filter, { cls: "card" }) &&
      operand.what.relation.relation === "event-result" && operand.what.relation.event === "exile" &&
      operand.what.relation.scope === "contextual") {
    const pool = this.id(context.linkedExile.path, "linked-exile-pool");
    this.bound(operand.what, child(operandPath, "what"), pool, ["607.2a", "607.3"]);
    binding = { kind: "linked-exile-names", pool, quantifier: "any" };
  } else {
    const whatPath = child(operandPath, "what");
    const sourceOperand = operand.what.ref === "self" || (operand.what.ref === "this" && operand.what.noun !== "ability") ||
      (operand.what.ref === "trigger-event-object" && operand.what.event === "enter" && context.exchangeEntrant !== undefined);
    const lastKnown = knownTiming === "resolution" || knownTiming === "intervening-if" || (knownTiming === "target-legality" && sourceOperand);
    this.walk(operand.what, whatPath, { ...scoped, lastKnown });
    const target = this.at(whatPath);
    if (target?.domain !== "object" || target.cardinality !== "one") {
      this.diagnostic("unsupported-scope", operandPath, "The comparison operand needs one bound object.", ["608.2h"]);
      return;
    }
    binding = { kind: "object", object: target.id, information: lastKnown ? "current-or-last-known" : "current" };
  }
  this.node({ id: this.id(path, "characteristic-predicate"), path, node: "characteristic-predicate",
    scope: context.ability, subject, operand: binding, timing: knownTiming, test, rules });
  this.engineFeature("characteristic-predicate", path,
    "Evaluate the printed relation using the indicated characteristic set and quantifier at its binding time.", rules);
}

export function commonNameSubset(this: Analyzer, condition: Extract<Condition, { condition: "controls-subset" }>, path: string, context: Context): boolean {
  const { who, of, minimum, groupConstraints } = condition;
  if (context.barrier || who.player !== "you" || minimum.amount !== "fixed" || minimum.value < 2 ||
      !isDeepStrictEqual(groupConstraints, [{ constraint: "shared-value", characteristic: "name" }])) return false;
  this.walk(who, child(path, "who"), context);
  this.walk(of, child(path, "of"), context);
  const rules = ["201.2a", "602.2b", "608.2h"];
  this.node({ id: this.id(path, "common-name-subset"), path, node: "common-name-subset",
    scope: context.ability, controller: context.you, filter: of, minimum: minimum.value, rules });
  this.engineFeature("characteristic-predicate", path, "Test for a qualifying group with a common name without selecting any of its members.", rules);
  return true;
}

export function dynamicStaticValue(this: Analyzer, value: Record<string, unknown>, path: string, context: Context): boolean {
  const recipient = context.recipient;
  const multiplier = value.multiplier;
  if (context.barrier || context.phase !== "continuous" || value.modifier !== "pt" ||
      !recipient || !("candidate" in recipient) || recipient.candidate.domain !== "object" || !isRecord(multiplier)) return false;
  const countsSelf = multiplier.amount === "counter-count" && isRecord(multiplier.on) && multiplier.on.ref === "it";
  if (!countsSelf && multiplier.amount !== "distinct-count") return false;
  const group = recipient.candidate;
  const rules = ["611.3a", "613.1g", "613.6"];
  const member = this.node({ id: this.id(path, "affected-member"), path, node: "characteristic-subject",
    scope: context.ability, role: "affected-member", of: group.id, timing: "continuous", rules });
  const candidate: Candidate = { id: member, cardinality: "one", domain: "object", description: group.description };
  // In this value scope alone, "it" means the individual affected object.
  this.walkChildren(value, path, { ...context, objects: countsSelf ? [candidate] : context.objects,
    affectedMember: candidate, mentionedObject: true });
  this.engineFeature("characteristic-layers", path, "Reevaluate the modifier on each affected object in the appropriate layer.", rules);
  return true;
}

const divisionTarget = Symbol("fixed divided damage target");

function countDamageDivisions(value: unknown): number {
  if (Array.isArray(value)) return value.reduce<number>((count, child) => count + countDamageDivisions(child), 0);
  if (!isRecord(value)) return 0;
  if (value.effect === "damage" && isRecord(value.to) && value.to.to === "divided") return 1;
  return Object.values(value).reduce<number>((count, child) => count + countDamageDivisions(child), 0);
}

/** CR 601.2c–d, 603.3d: the ability announces one bounded target group and
 * divides the fixed total at the same announcement. */
function bindDamageDivision(analyzer: Analyzer, effect: Extract<Effect, { effect: "damage" }>,
  path: string, context: Context): boolean {
  if (context.phase !== "resolution" || context.announcementOperand || context.targetScope || context.decider ||
      context.conditional || effect.to.to !== "divided" || effect.amount.amount !== "fixed" ||
      !Number.isSafeInteger(effect.amount.value)) return false;
  const { among } = effect.to;
  if (among.ref !== "any-target" || !among.selectionId ||
      !isDeepStrictEqual(effect.to, { to: "divided", among }) ||
      !isDeepStrictEqual(among, { ref: "any-target", selectionId: among.selectionId, count: among.count })) return false;
  const range = chosenRange(among.count);
  if (!range || range.minimum !== 1 || range.maximum === undefined || range.maximum > effect.amount.value) return false;
  const scope = printedNumberScope(analyzer, path, context);
  if (scope?.ability.kind !== "spell" && scope?.ability.kind !== "triggered" && scope?.ability.kind !== "activated") return false;
  const direct = /^\/effects\/(\d+)\/steps\/(\d+)$/.exec(path.slice(scope.root.length));
  if (!direct) return false;
  const sentence = scope.ability.effects[Number(direct[1])];
  if (sentence?.steps[Number(direct[2])] !== effect || !isDeepStrictEqual(sentence, { steps: sentence.steps }) ||
      !isDeepStrictEqual(effect, { effect: "damage", source: effect.source, amount: effect.amount, to: effect.to })) return false;
  const producers = scope.ability.kind === "spell"
    ? faceAbilities(analyzer, scope.face).flatMap(({ ability }) => ability.kind === "spell" ? [ability] : []) : scope.ability;
  if (countDamageDivisions(producers) !== 1) return false;
  const at = child(path, "to");
  const scoped = { ...context, [divisionTarget]: among };
  analyzer.walk(effect.to, at, scoped);
  analyzer.walk(effect.amount, child(path, "amount"), context);
  const targets = analyzer.at(child(at, "among"));
  if (targets) {
    const rules = ["601.2c", "601.2d", "602.2b", "603.3d"];
    analyzer.node({ id: analyzer.id(path, "target-division"), path, node: "target-division", scope: context.ability,
      chooser: context.you, event: analyzer.id(path, "event"), targets: targets.id, total: effect.amount.value, minimumEach: 1, rules });
  }
  return true;
}

export function walkDamageAmount(this: Analyzer, effect: Extract<Effect, { effect: "damage" }>, path: string, context: Context): boolean {
  if (context.barrier) return false;
  if (effect.to.to === "divided") return bindDamageDivision(this, effect, path, context);
  if (effect.to.to === "each" && effect.to.refs.length === 2) {
    const [players, creatures] = effect.to.refs;
    const groupType = "ref" in creatures && creatures.ref === "each" && creatures.filter.types?.[0];
    if ("player" in players &&
        (isDeepStrictEqual(players, { player: "each-opponent" }) ||
          (players.player === "target-opponent" && players.selectionId && !players.count)) &&
        "ref" in creatures && creatures.ref === "each" && (groupType === "creature" || groupType === "planeswalker") &&
        isDeepStrictEqual(creatures.filter, { types: [groupType], controlledBy: { player: "they" } })) {
      const at = `${path}/to/refs`;
      const opponents = this.player(players, child(at, 0), context);
      if (opponents) {
        const rules = ["102.2", "120.4b", "608.2f", ...(players.player === "target-opponent" ? ["601.2c", "608.2b"] : [])];
        const controlled = this.object(creatures, child(at, 1), { ...context, actor: opponents });
        if (controlled) {
          this.node({ id: this.id(path, "opponent-group-damage"), path, node: "opponent-group-damage",
            scope: context.ability, event: this.id(path, "event"), players: opponents.id,
            ...(groupType === "creature" ? { creatures: controlled.id } : { planeswalkers: controlled.id }), rules });
          this.engineFeature("opponent-group-damage", path, groupType === "creature"
            ? "Use the same opponent set for the player and controlled-creature recipients of one simultaneous damage event."
            : "Use the same opponent set for the player and controlled-planeswalker recipients of one simultaneous damage event.", rules);
        }
      }
      this.walk(effect.amount, child(path, "amount"), context);
      return true;
    }
  }
  if (effect.to.to !== "player") return false;
  const quantity = effect.amount.amount === "twice" ? effect.amount.of : effect.amount;
  if (quantity.amount !== "count" || !(quantity.of.control === "that-player" ||
      quantity.of.controlledBy?.player === "they" || quantity.of.controlledBy?.player === "that-player")) return false;
  const { ref } = effect.to;
  if (ref.player !== "defending-player" && ref.player !== "target-player" && ref.player !== "target-opponent" &&
      ref.player !== "each-player" && ref.player !== "each-opponent") return false;
  const at = child(path, "to");
  this.walk(effect.to, at, context);
  const recipient = this.at(child(at, "ref"));
  if (!recipient) return true; // The operand has its own diagnostic.
  const amount = child(path, "amount");
  if (recipient.cardinality === "one") this.walk(effect.amount, amount, { ...context, actor: recipient });
  else if (ref.player === "each-player" || ref.player === "each-opponent") {
    const rules = ["608.2f", "608.2h", "608.2k"];
    const member = this.node({ id: this.id(amount, "recipient-member"), path: amount, node: "recipient-member",
      scope: context.ability, group: recipient.id, use: "damage-amount", rules });
    this.walk(effect.amount, amount, { ...context, actor: { id: member, domain: "player", cardinality: "one" } });
    this.engineFeature("recipient-dependent-damage", path,
      "Calculate each recipient's scalar damage amount from that recipient before applying the simultaneous damage event.", rules);
  } else {
    this.diagnostic("unsupported-scope", amount, "The damage amount needs an explicit member of its recipient set.", ["608.2f", "608.2h"]);
    this.walk(effect.amount, amount, { ...context, barrier: "The damage amount needs an explicit member of its recipient set." });
  }
  return true;
}

export function boundTargetCount(this: Analyzer, expression: ObjectRef | PlayerRef, context: Context & { [divisionTarget]?: ObjectRef }):
  { minimum: number; maximum?: number; maximumNumber?: string } | undefined {
  if (context.barrier || context.unannouncedDivision || context.phase !== "resolution") return;
  if ("ref" in expression && expression.ref === "any-target" && expression === context[divisionTarget])
    return chosenRange(expression.count);
  if ("ref" in expression && expression.ref === "target" && expression.selectionId &&
      !expression.chosenBy && !expression.random) {
    if (expression.count.amount === "x" && expression.upTo && context.announcedX)
      return { minimum: 0, maximumNumber: context.announcedX };
    return chosenRange(expression.count);
  }
  if (context.targetPlayerGroup && "player" in expression &&
      (expression.player === "target-player" || expression.player === "target-opponent") && expression.selectionId)
    return chosenRange(expression.count);
}
export function boundResolutionCount(this: Analyzer, expression: ObjectRef | PlayerRef, path: string, context: Context): ReturnType<typeof chosenRange> {
  if (!context.barrier && context.phase === "resolution" && context.resolutionCount?.path === path &&
      "ref" in expression && expression.ref === "filter") return chosenRange(expression.count);
}
export function boundEventAggregate(this: Analyzer, quantity: Extract<Amount, { amount: "event-quantity" }>, path: string, context: Context): boolean {
  if (context.barrier || context.uncertainMeasureOrigin || quantity.scope !== "this-way") return false;
  const damage = quantity.event === "damage" && quantity.measure === "dealt" && !quantity.by && !quantity.to;
  const life = quantity.event === "life-change" && quantity.change === "lost" && !quantity.who;
  if (!damage && !life) return false;
  const matches = context.measuredEvents?.filter(entry => entry.operation === (damage ? "damage" : "lose-life"));
  if (matches?.length !== 1) return false;
  const rules = [damage ? "120.4b" : "119.3", "608.2c", "608.2f", "608.2h"];
  this.node({ id: this.id(path, "event-aggregate"), path, node: "event-aggregate", scope: context.ability,
    event: matches[0].event, measure: damage ? "damage-dealt" : "life-lost", rules });
  this.engineFeature("event-aggregate", path, "Sum the actual outcomes of the bound instruction across its participants.", rules);
  return true;
}
