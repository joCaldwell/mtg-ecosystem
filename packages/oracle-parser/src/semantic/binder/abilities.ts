// Abilities, keywords and granted abilities.
import { isDeepStrictEqual } from "node:util";
import type { Ability, Cost, CostModification, GainedAbility, KeywordInstance, LookPermission, Sentence, StaticEffect } from "../../ast.ts";
import { KEYWORD_SCOPES } from "../keyword-scopes.ts";
import type { Analyzer } from "./analyzer.ts";
import { entryChoice, resolutionChoice } from "./choices.ts";
import { child, combine, objectCardinality, type Candidate, type Context } from "./context.ts";
import { KEYWORD_PAYMENT_FIELDS, containsEffect, costHasX, definesX, isRecord, namesObject, soleObjectPayment, type StaticRule } from "./predicates.ts";
import { linkedExileOrigin } from "./replacement-shapes.ts";
import { DIRECT_CREATURE_SPELL_COPY, HIDEAWAY_FOUR, PROTECTION_FROM_EVERYTHING, hideawayProducer, isRevealAndCast, isSameTurnEntryCopy, isSelfGraveyardReplacement, ordinaryEntryCopy } from "./templates.ts";

/** A keyword ability of the source, printed on its own line. Its printed
 * parameters belong to the ability the keyword represents (CR 702): "you"
 * and a cost's payer are that ability's player, not the line's. */
export function keyword(this: Analyzer, instance: KeywordInstance, path: string, context: Context): void {
  const scope = KEYWORD_SCOPES[instance.keyword];
  if (!scope || "unsupported" in scope) {
    const barrier = scope?.unsupported ?? "This keyword's intrinsic ability scopes, targets, and payment roles are not modeled.";
    this.diagnostic("unsupported-scope", path, barrier, ["702"]);
    this.walkChildren(instance as unknown as Record<string, unknown>, path, { ...context, barrier });
    return;
  }
  const protectionFromEverything = isDeepStrictEqual(instance, PROTECTION_FROM_EVERYTHING);
  if (protectionFromEverything) this.engineFeature("protection-from-everything", path,
    "Apply protection against all sources for targeting, damage, attachments, and blocking.", ["702.16j"]);
  const you = this.node({ id: this.id(path, "you"), path, node: "runtime-player", scope: context.ability, role: scope.role, rules: ["109.5", ...scope.rules] });
  this.node({ id: this.id(path, "keyword"), path, node: "keyword-ability", scope: context.ability, keyword: instance.keyword,
    ...(protectionFromEverything ? { protectionFrom: "everything" as const } : {}),
    abilities: scope.abilities, you, rules: scope.rules });
  if (instance.keyword === "hideaway" && context.source === this.id("/source", "object")) {
    if (isDeepStrictEqual(instance, HIDEAWAY_FOUR)) {
      this.node({ id: this.id(path, "pool"), path, node: "hideaway-exiled-pool",
        keyword: this.id(path, "keyword"), source: context.source, key: "source-incarnation",
        libraryOwner: you, lookCount: 4, exile: "one-face-down", remainder: "bottom-random",
        lookPermission: "controller-of-exiling-permanent", membership: "still-in-exile", cardinality: "many",
        rules: ["400.7", "406.3", "607.2a", "607.3", "702.75a"] });
    }
    this.engineFeature("hideaway-resolution", path,
      "On entry, the trigger controller looks at the top N cards of their library, exiles one face down if possible and puts the rest on the bottom in random order. Track the cards exiled by this keyword on this incarnation and the granted and retained permissions to look.",
      ["400.7", "406.3", "607.2a", "609.3", "702.75a"]);
  }
  if (instance.keyword === "demonstrate")
    this.engineFeature("demonstrate", path,
      "On cast, the trigger controller may copy the spell. If they do, they choose an opponent who also copies it. Each may choose new targets for their own copy as it is put onto the stack.",
      ["702.144a", "707.10", "707.10c"]);
  const own: Context = { ...context, you, objects: [], players: [], quantities: [], x: undefined };
  const kickerAnnouncement = this.kickers.get(context.face)?.find(entry => entry.id === this.id(path, "keyword"))?.announcedX;
  if (kickerAnnouncement) {
    own.announcedX = this.node({ id: kickerAnnouncement, path, node: "announced-number",
      scope: this.id(path, "keyword"), chooser: you, announcement: "cast", rules: ["107.3a", "601.2b", "702.33a"] });
  }
  this.definitions(instance.bindings, child(path, "bindings"), own);
  for (const [key, value] of Object.entries(instance)) {
    if (key === "keyword" || key === "bindings") continue;
    if (key === "costModifications" && this.equipColorDiscount(instance, path, own)) continue;
    if (key === "amount") { if (value === "x") this.walk({ amount: "x" }, child(path, "amount"), own); continue; }
    const payment: Context = { ...own, phase: "payment" };
    if (instance.keyword === "escape" && key === "cost" && Array.isArray(instance.cost)) {
      const index = instance.cost.findIndex(cost => cost.cost === "exile" && cost.what.ref === "filter" &&
        cost.what.filter.cls === "card" && cost.what.filter.other && isDeepStrictEqual(cost.what.filter.zone, { zone: "graveyard", owner: "your" }));
      if (index >= 0) payment.sourceExclusionPath = `${path}/cost/${index}/what/filter`;
    }
    this.walk(value, child(path, key), KEYWORD_PAYMENT_FIELDS.has(key) ? payment : own);
  }
  instance.bindings?.forEach((binding, index) => this.walk(binding.value, child(child(child(path, "bindings"), index), "value"), own));
}
export function equipColorDiscount(this: Analyzer, instance: KeywordInstance, path: string, context: Context): boolean {
  const mods = instance.costModifications;
  if (instance.keyword !== "equip" || mods?.length !== 1 || instance.restriction || !instance.cost?.length) return false;
  const [discount] = mods;
  if (discount.change.direction !== "reduce" || !isDeepStrictEqual(discount.change.symbols, ["1"]) ||
      Object.keys(discount).length !== 2 || !isDeepStrictEqual(discount.multiplier, {
        amount: "distinct-count", characteristic: "color", among: {
          ref: "targeted-by", source: { ref: "this", noun: "ability" }, filter: { types: ["creature"] },
        },
      })) return false;
  const at = `${path}/costModifications/0/multiplier/among`;
  const rules = ["105.2", "601.2c", "601.2f", "602.2b", "702.6a"];
  const intrinsic = this.id(path, "keyword");
  const target = this.node({ id: this.id(at, "equip-target"), path: at, node: "equip-target-color-discount",
    scope: intrinsic, chooser: context.you, source: context.source, timing: "after-target-announcement", rules });
  this.bound({ ref: "this", noun: "ability" }, child(at, "source"), intrinsic, ["113.7a", "702.6a"]);
  this.bound({ ref: "targeted-by", source: { ref: "this", noun: "ability" }, filter: { types: ["creature"] } }, at, target, rules);
  this.engineFeature("equip-color-discount", at, "Count the colors of the intrinsic equip target as the cost is determined.", rules);
  return true;
}

/** A keyword ability that belongs to another object: a token being created,
 * a characteristic an effect gives, or an ability a filter describes. With
 * no printed references it has nothing to bind; any reference in its
 * parameters would need that object's own ability scope. */
export function otherObjectKeyword(this: Analyzer, instance: Record<string, unknown>, path: string, context: Context): void {
  const barrier = "A keyword ability of another object needs that object's own ability scope for the references in its parameters.";
  const references = this.result.references.length, diagnostics = this.result.diagnostics.length;
  for (const [key, value] of Object.entries(instance)) {
    if (key === "keyword") continue;
    if (key === "amount") { if (value === "x") this.walk({ amount: "x" }, child(path, "amount"), { ...context, barrier }); continue; }
    this.walk(value, child(path, key), { ...context, barrier });
  }
  if (this.result.references.length > references || this.result.diagnostics.length > diagnostics)
    this.diagnostic("unsupported-scope", path, barrier, ["113.10", "702"]);
}
/** A replacement effect that modifies how a permanent enters (CR 614.1c,
 * 614.12). For the source's own entry, "this" is the entering object and
 * "you" the player it enters under (CR 110.2a); choices and payments are
 * made before it enters (614.12a). Entry of other permanents is evaluated
 * for each of them by the static ability's controller. */
export function entering(this: Analyzer, value: Record<string, unknown>, path: string, context: Context): void {
  const what = value.what;
  const self = isRecord(what) && (what.ref === "self" || what.ref === "this");
  const scope: Context = self ? { ...context, phase: "continuous",
    you: this.node({ id: this.id(path, "entering-controller"), path, node: "runtime-player", scope: context.ability, role: "entering-controller", rules: ["110.2a", "614.12"] }) } : context;
  const bindings = Array.isArray(value.bindings) ? value.bindings as Sentence["bindings"] : undefined;
  this.definitions(bindings, child(path, "bindings"), scope);
  for (const [key, entry] of Object.entries(value)) {
    if (key === "static" || key === "bindings") continue;
    if (key === "effects") { this.sentences(entry as Sentence[], child(path, key), scope); continue; }
    this.walk(entry, child(path, key), key === "costs" ? { ...scope, phase: "payment" } : scope);
  }
  bindings?.forEach((binding, index) => this.walk(binding.value, child(child(child(path, "bindings"), index), "value"), scope));
}
export function simplePaymentAlternatives(this: Analyzer, cost: Extract<Cost, { cost: "choice" }>, path: string, context: Context): boolean {
  if (context.barrier || context.phase !== "payment" || cost.options.length < 2 ||
      !cost.options.every(option => option.length > 0 && option.every(part =>
        (part.cost === "mana" && !part.multiplier && !part.symbols.includes("X")) ||
        (part.cost === "pay-life" && part.amount.amount === "fixed")))) return false;
  this.walk(cost.options, child(path, "options"), context);
  const rules = ["118.1", "601.2b", "601.2h", "602.2b"];
  this.node({ id: this.id(path, "payment-alternatives"), path, node: "payment-alternatives", scope: context.ability,
    payer: context.you, options: cost.options, rules });
  this.engineFeature("intrinsic-payment", path, "Choose one alternative in the activation payment scope before paying it.", rules);
  return true;
}

export function attackPayment(this: Analyzer, effect: Extract<StaticEffect, { static: "combat-rule" }>, path: string, context: Context): boolean {
  const { rule } = effect;
  if (context.barrier || context.phase !== "continuous" || rule.rule !== "declaration-cost" ||
      rule.action !== "attack" || rule.scope !== "declaration" || rule.condition ||
      !isDeepStrictEqual(rule.who, { player: "controller", of: { ref: "them" } }) ||
      effect.what.ref !== "each" || rule.multiplier?.amount !== "count" ||
      !rule.costs.every(cost => cost.cost === "mana" && !cost.multiplier)) return false;
  const group = this.object(effect.what, child(path, "what"), context);
  if (!group || group.domain !== "object") return false;
  const at = child(path, "rule");
  const rules = ["508.1a", "508.1b", "508.1h", "508.1i", "508.1j"];
  const payer = this.node({ id: this.id(at, "declaring-player"), path: at, node: "runtime-player",
    scope: context.ability, role: "declaring-player", rules });
  this.bound(rule.who, child(at, "who"), payer, rules);
  this.bound({ ref: "them" }, `${at}/who/of`, group.id, rules);
  this.walk(rule.costs, child(at, "costs"), { ...context, you: payer, phase: "payment" });
  this.walk(rule.multiplier, child(at, "multiplier"), { ...context, actor: { id: payer, domain: "player", cardinality: "one" } });
  this.walk(rule.defenders, child(at, "defenders"), context);
  this.node({ id: this.id(at, "attack-payment"), path: at, node: "attack-payment", scope: context.ability, payer, eligible: group.id, rule, rules });
  this.engineFeature("attack-payment", at, "Evaluate the costs once for the proposed attack declaration and charge its declaring player.", rules);
  return true;
}

export function staticRule(this: Analyzer, rule: StaticRule, path: string, context: Context): boolean {
  if (context.barrier || context.phase !== "continuous" || !context.permanentSource) return false;
  let rules: string[];
  const first = this.result.references.length;
  if (rule.static === "prohibit") {
    const prohibition = rule.prohibition;
    if (prohibition.prohibition === "prevent-damage" && !prohibition.by && !prohibition.to) {
      rules = ["101.2", "615.12"];
    } else if (prohibition.prohibition === "activate" && prohibition.abilities.ref === "each" &&
        prohibition.abilities.filter.cls === "ability" && prohibition.abilities.filter.abilityKind === "activated" &&
        prohibition.abilities.filter.abilitySource?.ref === "each" && !prohibition.who && !prohibition.when) {
      rules = ["101.2", "602.1c", "611.3"];
    } else if (prohibition.prohibition === "cast" && prohibition.who.player === "each-opponent" &&
        prohibition.what.ref === "each" && prohibition.what.filter.cls === "spell" &&
        prohibition.when?.during === "turn" && prohibition.when.whose.player === "you") {
      rules = ["101.2", "102.3", "601.3", "611.3"];
    } else return false;
    this.walk(prohibition, child(path, "prohibition"), context);
  } else if (rule.static === "untap-during-step") {
    const owner = rule.step.owner;
    if (!isDeepStrictEqual(owner, { player: "filtered", base: { player: "each-player" }, filter: { otherThan: { player: "you" } } }) ||
        !["self", "this", "all", "each"].includes(rule.what.ref)) return false;
    rules = ["102.1", "502.3", "611.3a"];
    const at = `${path}/step/owner`;
    const others = this.node({ id: this.id(at, "other-players"), path: at, node: "derived-player",
      role: "other-players", scope: context.ability, of: context.you, rules });
    this.bound(owner, at, others, rules);
    this.object(rule.what, child(path, "what"), context);
  } else {
    if (rule.who.player !== "each-player" || rule.maximum.amount !== "fixed") return false;
    rules = ["502.3", "611.3a"];
    this.walkChildren(rule as unknown as Record<string, unknown>, path, context);
  }
  const operands = this.result.references.slice(first).flatMap(ref => ref.binding === "bound" ? [ref.target] : []);
  this.node({ id: this.id(path, "static-rule"), path, node: "static-rule", scope: context.ability, rule, operands, rules });
  this.engineFeature(rule.static === "prohibit" && rule.prohibition.prohibition === "prevent-damage" ? "damage-prevention" : "static-rule",
    path, "Apply the printed prohibition or untap rule to its bound subjects as the game action is evaluated.", rules);
  return true;
}

/** CR 401.5: the looked-at object is queried when the permission is used.
 * A newly exposed top card is hidden until the current action finishes. */
export function privateLibraryLook(this: Analyzer, permission: LookPermission, path: string, context: Context): boolean {
  const [card] = permission.what;
  if (context.barrier || context.phase !== "continuous" || !context.permanentSource ||
      permission.who.player !== "you" || permission.timing !== "any-time" || permission.what.length !== 1 ||
      permission.topLibraryChanges !== "after-casting-activation-or-special-action" ||
      card.ref !== "library-cards" || card.position !== "top" || card.count.amount !== "fixed" ||
      card.count.value !== 1 || card.of?.zone !== "library" || card.of.owner !== "your" ||
      Object.keys(card).some(key => !["ref", "position", "count", "of"].includes(key))) return false;
  const at = child(path, "permission");
  this.walk(permission, at, context);
  const rules = ["109.5", "401.5", "601.2i", "602.2b"];
  this.node({ id: this.id(path, "private-library-look"), path, node: "private-library-look", scope: context.ability,
    viewer: context.you, libraryOwner: context.you, refresh: "after-announcement-or-special-action", rules });
  this.engineFeature("private-library-look", path,
    "Allow only the bound viewer to inspect the current top card, after any in-progress casting, activation, or special action finishes.", rules);
  return true;
}
/** An ability an effect gives an object is that object's ability (CR 113.10,
 * 201.5a-b): its source is the recipient, its "you" the recipient's
 * controller (CR 301.5d, 303.4e). Each affected object has its own instance.
 * The granting effect's own decisions (a keyword choice, a protection
 * exception) stay in the granting scope. */
export function gained(this: Analyzer, grant: GainedAbility, path: string, context: Context): void {
  if (grant.gained === "this-ability") return;
  if (grant.gained === "abilities-of") {
    const barrier = "Abilities copied from other objects need those objects' abilities and their reevaluation bound (CR 113.10, 611.3a).";
    this.diagnostic("unsupported-scope", path, barrier, ["113.10", "201.5b", "611.3a"]);
    this.walkChildren(grant as unknown as Record<string, unknown>, path, { ...context, barrier });
    return;
  }
  const exception = grant.gained === "keyword" ? grant.protectionAttachmentException : undefined;
  if (exception?.exception === "already-attached") {
    const barrier = "The exempted attachments are a snapshot taken as the protection effect begins; that moment is not bound.";
    this.diagnostic("unsupported-scope", child(path, "protectionAttachmentException"), barrier, ["702.16n", "702.16p"]);
    this.walk(exception, child(path, "protectionAttachmentException"), { ...context, barrier });
  } else if (exception) this.walk(exception, child(path, "protectionAttachmentException"), context);
  if (grant.gained === "keyword-choice") {
    this.walk(grant.who, child(path, "who"), context);
    this.walk(grant.count, child(path, "count"), context);
  }
  const keywords = grant.gained === "keyword" ? [[grant.keyword, child(path, "keyword")] as const]
    : grant.gained === "keyword-choice" ? grant.options.map((option, index) => [option, child(child(path, "options"), index)] as const) : [];
  const recipient = context.recipient;
  const source = recipient && "candidate" in recipient && recipient.candidate.domain === "object"
    ? this.node({ id: this.id(path, "recipient"), path, node: "recipient", scope: context.ability, of: recipient.candidate.id,
        each: recipient.candidate.cardinality !== "one", rules: ["113.10", "201.5a"] })
    : recipient && "created" in recipient
      ? this.node({ id: this.id(path, "recipient"), path, node: "recipient", scope: context.ability, created: true,
          ...(recipient.emblemController ? { emblem: { owner: recipient.emblemController, controller: recipient.emblemController, zone: "command" } } : {}),
          rules: recipient.emblemController ? ["114.1", "114.2", "114.4"] : ["111.1", "113.10"] })
      : undefined;
  if (!source) {
    // A player's keyword, or a recipient that is not bound: only a keyword
    // without references in its parameters is free of obligations.
    if (grant.gained !== "quoted") { keywords.forEach(([keyword, at]) => this.otherObjectKeyword(keyword as unknown as Record<string, unknown>, at, context)); return; }
    const barrier = "The ability's recipient is not a bound object, so its source and controller are unknown (CR 113.10, 201.5a).";
    this.diagnostic("unsupported-scope", path, barrier, ["113.10", "201.5a"]);
    this.walk(grant.ability, child(path, "ability"), { ...context, barrier });
    return;
  }
  if (grant.gained === "quoted") {
    const at = child(path, "ability");
    const quoted = grant.ability;
    const attacks = quoted.kind === "triggered" && quoted.trigger.trigger === "attacks" &&
      isDeepStrictEqual(quoted.trigger.what, { ref: "this", noun: "creature" });
    const fights = attacks ? quoted.effects.flatMap((sentence, index) => sentence.steps.flatMap((effect, step) =>
      effect.effect === "fight" && isDeepStrictEqual(effect.a, { ref: "it" }) && isDeepStrictEqual(effect.b, { ref: "self" })
        ? [`${at}/effects/${index}/steps/${step}/b`] : [])) : [];
    // CR 201.5a: here the quoted clause spells out the granting object's
    // name, while "this creature" is the recipient of the quoted ability.
    const outer = fights.length && context.permanentSource && !context.sourceMoved && recipient && "candidate" in recipient
      ? { paths: fights, source: context.source } : undefined;
    this.ability(quoted, at, context.face, source, undefined, undefined, outer);
    return;
  }
  const ability = this.id(path, "ability"), you = this.id(path, "you");
  this.node({ id: ability, path, node: "ability", source, face: context.face, abilityKind: "keywords", you, rules: ["113", "113.10"] });
  this.node({ id: you, path, node: "runtime-player", scope: ability, role: "static-source-controller", rules: ["109.5", "301.5d", "303.4e"] });
  const granted: Context = { source, face: context.face, ability, you, phase: "continuous", objects: [], players: [], quantities: [] };
  keywords.forEach(([keyword, at]) => {
    // "Protection from the color of your choice": the qualities are fixed by
    // the granting effect as it applies (CR 608.2d, 608.2h), so they belong
    // to the granting scope, not to the recipient's ability.
    if (!keyword.from) { this.keyword(keyword, at, granted); return; }
    const { from, ...rest } = keyword;
    this.walk(from, child(at, "from"), context);
    this.keyword(rest as KeywordInstance, at, granted);
  });
  if (grant.gained === "keyword" && grant.costDefinition) this.walk(grant.costDefinition, child(path, "costDefinition"), { ...granted, phase: "payment" });
}
export function cascade(this: Analyzer, effect: Extract<StaticEffect, { static: "as-cascade" }>, path: string, context: Context): void {
  const choice = child(path, "choice");
  const controllerPath = child(path, "controller");
  const from = child(choice, "from");
  const controller = this.player(effect.controller, controllerPath, context);
  if (!controller) {
    this.unresolved(effect.choice.from, from, "unresolved-dependency",
      "The cascade occurrence requires the controller of the static ability.", ["109.5", "702.85b"]);
    return;
  }
  const occurrence = this.node({ id: this.id(path, "cascade"), path, node: "cascade-occurrence",
    scope: context.ability, controllerMatches: controller.id, rules: ["608.2c", "702.85b", "702.85c"] });
  const decider = this.node({ id: this.id(controllerPath, "cascade-controller"), path: controllerPath,
    node: "derived-player", scope: context.ability, role: "controller", of: occurrence, rules: ["608.2c", "702.85b"] });
  const pool = this.node({ id: this.id(from, "pool"), path: from, node: "cascade-exiled-pool", occurrence,
    rules: ["702.85a", "702.85b", "702.85c"] });
  const cards = this.bound(effect.choice.from, from, pool, ["702.85b"]);
  this.event(choice, context, "choose-and-put", [cards], { optional: effect.choice.optional, decider });
  this.engineFeature("cascade-resolution", path,
    "Offer the choice from this cascade's cards still in exile after exiling, before the cast decision or bottom placement.",
    ["608.2d", "702.85b"]);
}
export function castSacrifice(this: Analyzer, ability: Ability, abilities: Ability[] | undefined, path: string): Candidate | undefined {
  const additional = abilities?.flatMap((sibling, index) => sibling.kind === "additional-cost" ? [{ sibling, index }] : []);
  if (additional?.length !== 1 || containsEffect(ability, "sacrifice")) return;
  const [{ sibling, index }] = additional;
  const [cost] = sibling.costs;
  if (sibling.costs.length !== 1 || sibling.optional || sibling.variableConstraints?.length || sibling.costModifications?.length ||
      cost.cost !== "sacrifice" || cost.what.ref !== "filter" ||
      !isDeepStrictEqual(cost.what.filter, { types: ["creature"] }) || objectCardinality(cost.what) !== "one") return;
  const siblings = path.slice(0, path.lastIndexOf("/"));
  return this.at(`${siblings}/${index}/costs/0/what`);
}

export function exileCastingDiscount(this: Analyzer, ability: Extract<Ability, { kind: "additional-cost" }>, path: string, context: Context): boolean {
  if (!ability.optional || ability.variableConstraints?.length || ability.costs.length !== 1 ||
      ability.costModifications?.length !== 1) return false;
  const [cost] = ability.costs;
  const [discount] = ability.costModifications;
  if (cost.cost !== "exile" || cost.what.ref !== "filter" || cost.what.filter.cls !== "card" ||
      Object.keys(cost.what.filter).some(key => !["cls", "zone", "colors"].includes(key)) ||
      !cost.what.filter.colors?.length || !isDeepStrictEqual(cost.what.filter.zone, { zone: "hand", owner: "your" }) ||
      !isDeepStrictEqual(cost.what.count, { amount: "chosen", minimum: 0 }) ||
      discount.change.direction !== "reduce" || discount.change.symbols.length !== 1 ||
      !/^[1-9][0-9]*$/.test(discount.change.symbols[0]) || Object.keys(discount).length !== 2 ||
      !isDeepStrictEqual(discount.multiplier, { amount: "count", of: {
        cls: "card", relation: { relation: "event-result", event: "exile", scope: "this-way" },
      } })) return false;
  const at = `${path}/costs/0/what`;
  const rules = ["601.2b", "601.2f", "601.2h", "701.13a"];
  this.walk(cost.what.filter, child(at, "filter"), context);
  const selection = this.node({ id: this.id(at, "exile-commitment"), path: at, node: "casting-exile-commitment",
    scope: context.ability, caster: context.you, filter: cost.what.filter, minimum: 0,
    discountPerCard: Number(discount.change.symbols[0]), rules });
  this.bound(cost.what, at, selection, rules);
  this.engineFeature("casting-exile-discount", path,
    "Commit to a count of eligible cards for the additional cost, use that count for cost calculation, then pay the full committed exile cost.", rules);
  return true;
}

export function costModifications(this: Analyzer, modifications: CostModification[] | undefined, path: string, payment: Context): void {
  modifications?.forEach((modification, index) => {
    const at = child(path, index);
    const scoped = { ...payment };
    this.definitions(modification.bindings, child(at, "bindings"), scoped);
    this.walk(modification, at, scoped);
    if (scoped.x && this.result.references.some(reference => reference.binding === "bound" &&
        reference.target === scoped.x && reference.path.startsWith(`${at}/bindings/`))) {
      this.invalidNodes.set(scoped.x, "unresolved-dependency");
      this.diagnostic("unsupported-number-binding", child(at, "bindings"), "A payment definition cannot establish itself recursively.", ["107.3c"]);
    }
  });
}
export function ability(this: Analyzer, ability: Ability, path: string, face: string, source: string, cardAbilities?: Ability[], classLevel?: number,
  quotedFightSide?: Context["quotedFightSide"]): void {
  const id = this.id(path, "ability");
  const you = this.id(path, "you");
  const selfGraveyardReplacement = isSelfGraveyardReplacement(ability, cardAbilities);
  const sameTurnEntryCopy = cardAbilities !== undefined && this.permanentFaces.has(face) &&
    this.faceTypes.get(face)?.types?.includes("creature") && isSameTurnEntryCopy(ability);
  const entryCopy = sameTurnEntryCopy ? ability.effect :
    this.permanentFaces.has(face) && ability.kind === "static" && ability.effect.static === "entry-copy" && ordinaryEntryCopy(ability.effect)
      ? ability.effect : undefined;
  const role = ability.kind === "activated" || ability.kind === "loyalty" ? "activator"
    : ability.kind === "triggered" || ability.kind === "chapter" || ability.kind === "visit" || ability.kind === "case-solve" ? "trigger-controller"
    : ability.kind === "spell" ? "spell-controller" : ability.kind === "additional-cost" ? "prospective-caster"
    : selfGraveyardReplacement ? "replacement-source-controller-or-owner"
    : entryCopy ? "entering-controller" : "static-source-controller";
  this.node({ id, path, node: "ability", source, face, abilityKind: ability.kind, you, rules: ["113", "109.5"] });
  this.node({ id: you, path, node: "runtime-player", scope: id, role,
    rules: selfGraveyardReplacement ? ["108.4a", "109.5"] : entryCopy ? ["109.5", "110.2", "614.12"] : ["109.5"] });
  // An unqualified "other" excludes the permanent with the ability, including
  // an object that was given the ability; never the "source" of a spell.
  const recipientSource = this.result.nodes.find(node => node.id === source);
  const granted = recipientSource?.node === "recipient";
  const permanentSource = ability.kind !== "spell" && !(granted && recipientSource.emblem) &&
    (this.permanentFaces.has(face) || granted);
  const context: Context = { source, face, ability: id, you, permanentSource, staticAbility: ability.kind === "static",
    entryChoice: cardAbilities && permanentSource ? entryChoice(cardAbilities, path.slice(0, path.lastIndexOf("/"))) : undefined,
    resolutionChoice: resolutionChoice(ability, path), lexicalAbilityPath: path, classLevel, quotedFightSide,
    linkedExile: cardAbilities && permanentSource ? linkedExileOrigin(cardAbilities, path.slice(0, path.lastIndexOf("/"))) : undefined,
    spellAbility: ability.kind === "spell",
    // "This creature can't attack unless defending player controls an
    // Island", "This creature can't be blocked as long as ...": a rule about
    // the creature's own attack (CR 508.5).
    attackingSource: ability.kind === "static" && ability.effect.static === "cant" && isRecord(ability.effect.what)
      && (ability.effect.what.ref === "this" || ability.effect.what.ref === "self")
      && ["attack", "be-blocked"].includes(String((ability.effect as { action?: unknown }).action)),
    phase: ability.kind === "static" || ability.kind === "keywords" || ability.kind === "class-level-bar" ? "continuous" : "resolution", objects: [], players: [], quantities: [] };
  if ((ability.kind === "activated" && namesObject(ability.costs, false)) || (ability.kind === "static" && namesObject(ability.effect, true)))
    context.mentionedObject = true;
  // X a player announced (CR 107.3a): for an activated ability with X in
  // its cost (107.3k), a spell with X in its costs, or a permanent's own
  // entry from such a spell (107.3m). Text that defines X wins (107.3c); a
  // granted ability has no announced X (107.3j).
  if (!granted && !definesX(ability)) {
    const entering = (ability.kind === "triggered" && ability.trigger.trigger === "enters" && "what" in ability.trigger
        && isRecord(ability.trigger.what) && ["self", "this"].includes(String(ability.trigger.what.ref)))
      || (ability.kind === "static" && ["enters-with-counters", "as-enters", "entry-choice"].includes(String(ability.effect.static)));
    const announcement = (ability.kind === "activated" && costHasX(ability.costs)) || (ability.kind === "loyalty" && costHasX(ability.cost)) ? "activation" as const
      : ability.kind === "spell" && this.xFaces.has(face) ? "cast" as const
      : entering && this.xFaces.has(face) && this.permanentFaces.has(face) ? "entering-spell" as const : undefined;
    const constraints = ability.kind === "activated" ? ability.variableConstraints : undefined;
    if (announcement && constraints?.some(constraint => !("not" in constraint))) {
      this.diagnostic("unsupported-number-binding", child(path, "variableConstraints"),
        "An announcement constraint needs its own earlier bound value.", ["107.3a", "601.2b"]);
    } else if (announcement) context.announcedX = this.node({ id: this.id(path, "announced-x"), path, node: "announced-number", scope: id, chooser: you, announcement,
      ...(constraints?.length ? { constraints } : {}), rules: announcement === "activation" ? ["107.3a", "107.3k", "602.2b"] : announcement === "cast" ? ["107.3a", "601.2b"] : ["107.3m"] });
  }
  switch (ability.kind) {
    case "activated": case "loyalty": {
      const payment = { ...context, phase: "payment" as const };
      if (ability.kind === "activated" && ability.keyword)
        this.diagnostic("unsupported-scope", child(path, "keyword"),
          ability.keyword === "forecast"
            ? "Forecast requires the source's hand/owner/upkeep bindings, activation history, and reveal lifetime."
            : "Intrinsic activated-keyword rules require the source incarnation's activation, attack, or entry history and payment bindings.",
          ability.keyword === "forecast" ? ["702.57"] : ["702"]);
      const costs = child(path, ability.kind === "activated" ? "costs" : "cost");
      const solePayment = ability.kind === "activated" ? soleObjectPayment(ability.costs) : -1;
      if (ability.kind === "activated" && ability.costs[solePayment]?.cost === "sacrifice")
        payment.sourceExclusionPath = `${costs}/${solePayment}/what/filter`;
      this.walk(ability.kind === "activated" ? ability.costs : ability.cost, costs, payment);
      this.walk(ability.restriction, child(path, "restriction"), payment);
      this.walk(ability.permission, child(path, "permission"), payment);
      this.costModifications(ability.costModifications, child(path, "costModifications"), payment);
      context.sourceMoved = payment.sourceMoved;
      // Objects the cost selected are the effect's antecedents ("Sacrifice a
      // creature: You gain life equal to its toughness"); a sacrificed,
      // exiled, discarded or returned one is known by its last-known
      // information (CR 608.2h).
      // "Remove all charge counters from this artifact: It deals that much
      // damage": how much a cost paid is known to the effect (CR 601.2h, 602.2b).
      if (ability.kind === "activated") ability.costs.forEach((cost, index) => {
        if (!["pay-life", "remove-counters", "discard", "exile", "sacrifice"].includes(cost.cost)) return;
        const at = child(child(path, "costs"), index);
        const event = this.event(at, payment, `cost:${cost.cost}`, []);
        context.quantities = combine(context.quantities, [this.eventResult(at, event, "quantity", "one", "measured-quantity")]);
      });
      if (ability.kind === "activated") ability.costs.forEach((cost, index) => {
        if (!("what" in cost) || !isRecord(cost.what)) return;
        const selected = this.at(child(child(child(path, "costs"), index), "what"));
        if (!selected || selected.id === context.source || selected.domain !== "object") return;
        this.costObjects.add(selected.id);
        if (index === solePayment && selected.cardinality === "one") {
          if (cost.cost === "sacrifice" && !containsEffect(ability.effects, "sacrifice")) context.sacrificedCost = selected;
          else if (cost.cost === "exile" && selected.description?.cls === "card") {
            const at = `${costs}/${index}`;
            const event = this.id(at, "event");
            const paid = this.result.nodes.find(node => node.id === event);
            if (paid?.node === "event") paid.inputs.push(selected.id);
            const result = this.eventResult(at, event, "objects", "one", "zone-successors", { cls: "card" });
            context.exileResults = [{ event, result }];
          }
        }
        if (["sacrifice", "exile", "discard", "return", "move-zone"].includes(cost.cost)) context.departed = combine(context.departed ?? [], [selected]);
        else if (["tap-objects", "untap-objects", "exert", "reveal"].includes(cost.cost)) context.objects = combine(context.objects, [selected]);
      });
      const producer = ability.kind === "activated" && ability.effects[0]?.steps[0]?.effect === "play-card"
        && cardAbilities && permanentSource
        ? hideawayProducer(cardAbilities, ability) : undefined;
      if (producer !== undefined) {
        const siblings = path.slice(0, path.lastIndexOf("/"));
        this.hideawayPlay(`${path}/effects/0/steps/0`, this.id(`${siblings}/${producer.index}/keywords/0`, "pool"), context, producer.play);
        break;
      }
      this.sentences(ability.effects, child(path, "effects"), context);
      break;
    }
    case "triggered":
      if (this.attackDeclarationGuard(ability, path, context)) break;
      this.trigger(ability.trigger, child(path, "trigger"), context);
      if (this.entrantExchange(ability, path, context) || this.scanAgainstCastSpell(ability, path, context, cardAbilities) ||
          this.castFromSeededExile(ability, path, context, cardAbilities)) break;
      const kickers = this.kickers.get(context.face);
      if (!context.announcedX && kickers?.length === 1 && kickers[0].announcedX && !definesX(ability) &&
          ability.trigger.trigger === "enters" && (ability.trigger.what.ref === "self" || ability.trigger.what.ref === "this") &&
          ability.condition?.condition === "kicked" && ability.condition.what.ref === "it" && costHasX(ability.effects)) {
        const at = child(path, "condition");
        const rules = ["107.3m", "400.7d", "607.2", "702.33e"];
        context.enteringKickerX = this.node({ id: this.id(at, "entering-kicker-x"), path: at, node: "entering-kicker-x",
          scope: context.ability, source: context.source, kicker: kickers[0].id, announced: kickers[0].announcedX, rules });
        this.engineFeature("event-history", at, "Track the original spell's kicker payment and announced X through its own entry.", rules);
      }
      if (ability.trigger.trigger === "becomes-status" && ability.trigger.status === "monstrous" &&
          (ability.trigger.what.ref === "self" || ability.trigger.what.ref === "this") &&
          costHasX(ability.effects) && !definesX(ability)) {
        const at = child(path, "trigger");
        const event = this.id(at, "event");
        context.triggerX = this.eventResult(child(at, "monstrosity"), event, "quantity", "one", "measured-quantity").id;
        this.engineFeature("monstrosity-event", at, "Read X from the monstrosity action that made the triggering object monstrous.", ["701.37c"]);
      }
      context.interveningIf = true;
      this.namingCondition(ability.condition, child(path, "condition"), context);
      context.interveningIf = undefined;
      this.countedPermanentPool(ability, path, context);
      const linked = context.linkedExile;
      if (linked && linked.ability !== path && !context.objects.length && ability.condition?.condition === "count" &&
          isDeepStrictEqual(ability.condition.comparison, { op: "ge", value: { amount: "fixed", value: 1 } }) &&
          isDeepStrictEqual(Object.keys(ability.condition.of).sort(), ["cls", "relation"]) && ability.condition.of.cls === "card" &&
          ability.condition.of.relation?.relation === "linked-exile" &&
          (ability.condition.of.relation.source.ref === "self" || ability.condition.of.relation.source.ref === "this")) {
        const at = `${path}/condition`;
        const pool = this.id(linked.path, "linked-exile-pool");
        const rules = ["603.4", "607.2a", "608.2h"];
        this.node({ id: this.id(at, "linked-pool-condition"), path: at, node: "linked-pool-condition",
          scope: context.ability, pool, comparison: "nonempty", timing: "trigger-check-and-resolution", rules });
        context.countedLinkedPool = { id: pool, domain: "object", cardinality: "many", description: { cls: "card" } };
        context.objects = [context.countedLinkedPool];
        this.engineFeature("linked-exile-pool", at, "Check the current linked pool at both intervening-if checks.", rules);
      }
      if (context.stackPlacement && isDeepStrictEqual(ability.condition,
        { condition: "is-filter", what: { ref: "it" }, filter: { singleTarget: true } }))
        context.singleTargetStackGuard = true;
      this.sentences(ability.effects, child(path, "effects"), context);
      break;
    case "spell": {
      context.sacrificedCost = this.castSacrifice(ability, cardAbilities, path);
      const first = ability.effects[0]?.steps[0];
      if (cardAbilities?.length === 1 && first?.effect === "copy-stack-object") {
        const { abilityWord, ...paragraph } = ability;
        if (abilityWord === undefined && isDeepStrictEqual(paragraph, {
          kind: "spell", effects: [{ steps: [DIRECT_CREATURE_SPELL_COPY] }],
        })) context.directCreatureSpellCopy = first;
      }
      if (first?.effect === "shuffle" && cardAbilities?.length === 2 &&
          cardAbilities[1] === ability && isDeepStrictEqual(cardAbilities[0], { kind: "keywords", keywords: [{ keyword: "demonstrate" }] }) &&
          isRevealAndCast(ability)) {
        this.revealAndCast(ability.effects, child(path, "effects"), context);
        return;
      }
      const previous = this.spellParagraph?.face === face ? this.spellParagraph.context : undefined;
      if (previous) Object.assign(context, { objects: previous.objects, players: previous.players, quantities: previous.quantities,
        departed: previous.departed, sourceMoved: previous.sourceMoved, mentionedObject: previous.mentionedObject, sourceNamed: previous.sourceNamed,
        exileResults: previous.exileResults, uncertainExileOrigin: previous.uncertainExileOrigin,
        stackCopies: previous.stackCopies, previousEffect: previous.previousEffect, uncertainStackOrigin: previous.uncertainStackOrigin,
        scans: previous.scans, dieResults: previous.dieResults, uncertainDieOrigin: previous.uncertainDieOrigin, uncertainScanOrigin: previous.uncertainScanOrigin, uncertainRevealOrigin: previous.uncertainRevealOrigin,
        copiedCards: previous.copiedCards, uncertainCardCopyOrigin: previous.uncertainCardCopyOrigin,
        unexportedSecretChoices: previous.unexportedSecretChoices,
        measuredEvents: previous.measuredEvents, uncertainMeasureOrigin: previous.uncertainMeasureOrigin,
        declaredTargets: previous.declaredTargets, uncertainTargetOrigin: previous.uncertainTargetOrigin });
      this.sentences(ability.effects, child(path, "effects"), context);
      this.spellParagraph = { face, context };
      return;
    }
    case "chapter": this.sentences(ability.effects, child(path, "effects"), context); break;
    case "static":
      if (this.sharedExilePlay(ability, path, context, cardAbilities)) break;
      if (entryCopy) {
        this.bindEntryCopy(entryCopy, child(path, "effect"), context);
        break;
      }
      if (selfGraveyardReplacement) {
        this.selfGraveyardReplacement(`${path}/effect/replacement`, context);
        break;
      }
      if (ability.effect.static === "replacement") {
        const at = `${path}/effect/replacement`;
        if (this.quantityReplacement(ability.effect.replacement, at, context) ||
            this.keepUnspentMana(ability.effect.replacement, at, context) ||
            this.additionalStackCopy(ability.effect.replacement, at, context) ||
            this.exileReplacement(ability.effect.replacement, at, context)) break;
      }
      if (ability.effect.static === "play-permission" &&
          this.linkedExileLifeCost(ability.effect.permission, child(path, "effect"), context)) break;
      // "..., where X is ...": the static ability's own definition (CR 107.3c).
      if ("bindings" in ability.effect && Array.isArray(ability.effect.bindings)
        && !["entry-choice", "enters-with-counters", "enters-tapped", "as-enters"].includes(String(ability.effect.static)))
        this.definitions(ability.effect.bindings as Sentence["bindings"], child(child(path, "effect"), "bindings"), context);
      this.walk(ability.effect, child(path, "effect"), context);
      if (ability.effect.static === "replacement") this.diagnostic("unsupported-scope", path,
        "Replacement-event roles and captures need a separate scope.", ["614"]);
      break;
    case "keywords": ability.keywords.forEach((keyword, index) => this.keyword(keyword, child(child(path, "keywords"), index), context)); break;
    case "additional-cost": {
      const payment = { ...context, phase: "payment" as const };
      if (cardAbilities?.filter(entry => entry.kind === "additional-cost").length === 1 &&
          this.exileCastingDiscount(ability, path, payment)) break;
      // Additional costs and the spell paragraph use one announced X on
      // the stack. Do not share it with activated or triggered abilities.
      const spells = cardAbilities?.flatMap((entry, index) => entry.kind === "spell" ? [{ entry, index }] : []);
      if (!ability.optional && !ability.variableConstraints?.length &&
          costHasX(ability.costs) && spells?.length === 1 && !definesX(spells[0].entry)) {
        const spellPath = `${path.slice(0, path.lastIndexOf("/"))}/${spells[0].index}`;
        payment.announcedX = this.id(spellPath, "announced-x");
      }
      this.walk(ability.costs, child(path, "costs"), payment);
      this.costModifications(ability.costModifications, child(path, "costModifications"), payment);
      break;
    }
    case "exert-on-attack": {
      // The linked "When you do" ability is a separate reflexive trigger
      // whose "it" is the exerted object; its capture is not bound here.
      const barrier = "A linked reflexive exert trigger needs the exert cost's capture and its own ability scope bound.";
      this.diagnostic("unsupported-scope", path, barrier, ["701.43d", "607.2h"]);
      this.walk(ability.what, child(path, "what"), { ...context, phase: "payment", barrier });
      this.sentences(ability.reflexive ?? [], child(path, "reflexive"), { ...context, barrier });
      break;
    }
    case "linked-triggers": {
      const barrier = "Linked triggered abilities need the static ability's reveal events bound as their \"this way\".";
      this.diagnostic("unsupported-scope", path, barrier, ["603.11", "607.2a"]);
      this.walk(ability.static, child(path, "static"), { ...context, barrier });
      this.walk(ability.triggers, child(path, "triggers"), { ...context, barrier });
      break;
    }
    case "visit":
      this.diagnostic("unsupported-scope", path, "A visit ability needs its roll-to-visit event and the printing's lit-up numbers bound; neither is in the Oracle text.", ["701.52", "702.159a", "717.1"]);
      this.sentences(ability.effects, child(path, "effects"), context);
      break;
    case "sticker":
      this.diagnostic("unsupported-scope", path, "A sticker's ability belongs to the object it is put on; that object and the ticket payment are not bound.", ["123.3c", "123.7"]);
      if (ability.sticker.sticker === "ability")
        this.walk(ability.sticker.ability, child(path, "sticker/ability"), { ...context, barrier: "Sticker ability scope is unsupported." });
      break;
    case "max-speed":
      this.diagnostic("unsupported-scope", path, "The nested ability's source and controller scope have not been bound.", ["702.179"]);
      this.walk(ability.ability, child(path, "ability"), { ...context, barrier: "Nested ability scope is unsupported." });
      break;
    case "infinity":
      this.diagnostic("unsupported-scope", path, "The nested ability applies only while the permanent is harnessed; that designation history is not bound.", ["702.186b", "701.64b"]);
      this.walk(ability.ability, child(path, "ability"), { ...context, barrier: "Nested ability scope is unsupported." });
      break;
    case "class-level-bar": {
      const metadata = this.result.nodes.find(node => node.id === face);
      if (metadata?.node === "face" && /[—-] .*\bClass\b/.test(metadata.typeLine ?? "") &&
          Number.isSafeInteger(ability.level) && ability.level >= 2 && ability.costs.length > 0 &&
          ability.abilities.every(nested => nested.kind === "static" || nested.kind === "triggered" || nested.kind === "activated")) {
        const rules = ["602.2b", "716.2a", "716.2b", "716.2d"];
        const payer = this.node({ id: this.id(path, "upgrade-activator"), path, node: "runtime-player",
          scope: id, role: "activator", rules });
        this.walk(ability.costs, child(path, "costs"), { ...context, phase: "payment", you: payer });
        const abilities = ability.abilities.map((nested, index) => {
          const at = `${path}/abilities/${index}`;
          this.ability(nested, at, face, source, undefined, ability.level);
          return this.id(at, "ability");
        });
        this.node({ id: this.id(path, "class-level"), path, node: "class-level", scope: id, source, payer,
          level: ability.level, requiresLevel: ability.level - 1, timing: "sorcery", abilities, rules });
        this.engineFeature("class-level", path, "Track the source incarnation's noncopiable level and cumulative abilities, and pay for the next level only as a sorcery.", rules);
        break;
      }
      const barrier = "A Class level bar needs intrinsic upgrade activation/payment, designation history, and cumulative granted-ability scopes bound.";
      this.diagnostic("unsupported-scope", path, barrier, ["716.2", "716.3"]);
      this.walk(ability.costs, child(path, "costs"), { ...context, phase: "payment", barrier });
      this.walk(ability.abilities, child(path, "abilities"), { ...context, barrier });
      break;
    }
    case "anchor-words": {
      const barrier = "Anchor-word abilities need the linked as-enters choice and their own ability scopes bound.";
      this.diagnostic("unsupported-scope", path, barrier, ["614.12c", "607.2m"]);
      this.walk(ability.options, child(path, "options"), { ...context, barrier });
      break;
    }
    case "case-solve": case "case-solved": {
      const barrier = "Case designation history and the distinct solve, continuous, trigger-time, or activation gate scopes are not bound.";
      this.diagnostic("unsupported-scope", path, barrier, ["719.3", "702.169"]);
      if (ability.kind === "case-solve") this.walk(ability.condition, child(path, "condition"), { ...context, barrier });
      else this.walk(ability.ability, child(path, "ability"), { ...context, barrier });
      break;
    }
    default: {
      const unreachable: never = ability;
      throw new Error(`Unhandled ability variant: ${JSON.stringify(unreachable)}`);
    }
  }
  this.spellParagraph = undefined;
}
