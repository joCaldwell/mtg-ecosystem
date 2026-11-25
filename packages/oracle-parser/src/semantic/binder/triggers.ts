// Trigger conditions and event patterns.
import { isDeepStrictEqual } from "node:util";
import type { Ability, Condition, Effect, ObjectFilter, Trigger } from "../../ast.ts";
import { uniqueStackTriggerEvent } from "../../parser/stack-copy.ts";
import type { Analyzer } from "./analyzer.ts";
import { child, combine, type Candidate, type Context } from "./context.ts";
import { isRecord, namesObject, stackClasses } from "./predicates.ts";
import { replacementObject, replacementPlayer } from "./replacement-shapes.ts";

/** CR 601.2i, 603.7 and 614.1c. The next actual cast selects one
 * stack incarnation. Its resolving battlefield entry owns the counter. */
export function nextCreatureEntryCounter(this: Analyzer, effect: Extract<Effect, { effect: "create-delayed-trigger" }>, path: string, context: Context): boolean {
  const { trigger, effects, ...header } = effect;
  const object = { ref: "trigger-event-object", event: "cast" } as const;
  const cast = { trigger: "cast", who: { player: "you" }, what: { types: ["creature"], cls: "spell" } } as const;
  if (context.barrier || context.decider || context.phase !== "resolution" || trigger.trigger !== "cast" ||
      !isDeepStrictEqual(trigger, cast) ||
      !isDeepStrictEqual(header, { effect: "create-delayed-trigger", occurrence: "next", duration: { duration: "this-turn" },
        nextOnly: true, targets: "delayed-ability" }) ||
      !isDeepStrictEqual(effects, [{ steps: [{ effect: "create-replacement", replacement: {
        event: { event: "move-zone", from: { zone: "stack" }, to: { zone: "battlefield" }, resolving: true, what: object },
        occurrence: "next", replace: { replacement: "entry-modifiers", modifiers: { counters: [
          { count: { amount: "fixed", value: 1 }, counter: { type: "+1/+1" }, additional: true },
        ] } },
      } }] }])) return false;
  const rules = ["400.7", "601.2i", "603.7b", "608.3", "614.1c", "614.12"];
  const created = this.event(path, context, "create-delayed-trigger", [], context.conditional ? { conditional: true } : {});
  const delayed: Context = { ...this.nestedAbility(path, context, "delayed-triggered", rules),
    objects: [], players: [], quantities: [], triggerObject: undefined };
  this.trigger(trigger, child(path, "trigger"), delayed);
  const spell = delayed.triggerObject;
  const at = `${path}/effects/0/steps/0`;
  if (spell?.event !== "cast") {
    this.diagnostic("unsupported-scope", at, "The entering counter requires the delayed ability's exact completed spell cast.", rules);
    return true;
  }
  const subject = this.bound(object, `${at}/replacement/event/what`, spell.candidate.id, rules);
  const modifier = this.event(at, delayed, "create-replacement", [subject]);
  this.node({ id: this.id(path, "next-creature-entry-counter"), path, node: "next-creature-entry-counter",
    scope: context.ability, created, trigger: delayed.ability, spell: spell.candidate.id, modifier,
    window: "this-turn", move: "specific-stack-incarnation-resolving-to-battlefield", counter: "+1/+1", count: 1, rules });
  this.engineFeature("zone-replacement", path,
    "Watch the next completed creature cast this turn. The delayed trigger adds one counter only to the battlefield entry of that same resolving spell.", rules);
  return true;
}

export function delayedTrigger(this: Analyzer, effect: Extract<Effect, { effect: "create-delayed-trigger" }>, path: string, context: Context): void {
  const delayed = this.nestedAbility(path, context, "delayed-triggered", ["603.7"]);
  const enclosing = context.targetScope ?? context.ability;
  this.trigger(effect.trigger, child(path, "trigger"), effect.triggerTargets === "enclosing-ability" ? { ...delayed, targetScope: enclosing } : delayed);
  const body: Context = effect.targets === "enclosing-ability" ? { ...delayed, targetScope: enclosing } : delayed;
  this.walk(effect.condition, child(path, "condition"), body);
  this.sentences(effect.effects, child(path, "effects"), body);
}
/** "When you do, …" (CR 603.12): triggers on an action of this resolution and
 * otherwise follows the delayed-trigger rules. "When you do" names the one
 * optional instruction just decided. */
export function reflexiveTrigger(this: Analyzer, effect: Extract<Effect, { effect: "create-reflexive-trigger" }>, path: string, context: Context): void {
  const event = effect.event;
  if (event.event === "preceding-action") {
    const decisions = context.decisions?.length ? context.decisions : context.previousDecisions ?? [];
    this.walk(event.who, child(child(path, "event"), "who"), context);
    if (decisions.length === 1) this.node({ id: this.id(child(path, "event"), "decision-link"), path: child(path, "event"), node: "decision-link",
      scope: context.ability, condition: event.performed ? "you-do" : "you-dont", decision: decisions[0], rules: ["603.12", "608.2d"] });
    else this.diagnostic("unsupported-reference", child(path, "event"), "Payment/performance conditions need an explicit preceding-action link.", ["603.12"]);
  } else if (event.event === "repeated-payment" || event.event === "damage-prevented") {
    const barrier = "This reflexive trigger needs the repeated payment or prevention it watches bound.";
    this.diagnostic("unsupported-scope", child(path, "event"), barrier, ["603.12a", "615.5"]);
    this.walk(event, child(path, "event"), { ...context, barrier });
  } else this.walk(event, child(path, "event"), context);
  const reflexive = this.nestedAbility(path, context, "reflexive-triggered", ["603.12"]);
  this.sentences(effect.effects, child(path, "effects"), reflexive);
}
export function trigger(this: Analyzer, trigger: Trigger, path: string, context: Context): void {
  this.triggerPattern(trigger, path, context);
  // After the trigger, "another" may mean the object it matched.
  // The source as the trigger's subject ("When this creature enters") is not another object.
  if (namesObject(trigger, false)) context.mentionedObject = true;
  // A cast or activation trigger is about a spell or ability; a bare "it"
  // means that, unless the trigger names this object too ("a spell that
  // targets this creature, put a +1/+1 counter on it").
  if (context.triggerObject && !/"ref":"(self|this)"/.test(JSON.stringify(trigger))) context.spellPronoun = true;
}
export function counterChange(this: Analyzer, trigger: Extract<Trigger, { trigger: "counter-change" }>, path: string, context: Context,
  gate?: { path: string; condition: Extract<Condition, { condition: "is-filter" }> }): boolean {
  const { on, match, by } = trigger;
  const recipients = on.recipient === "any-of" ? on.recipients : [on];
  if (context.barrier || !("recipients" in match) || match.recipients !== "each" ||
      (by && by.player !== "you") || !recipients.every(recipient => recipient.recipient === "object"
        ? replacementObject(recipient.what) || recipient.what.ref === "this" || recipient.what.ref === "self"
        : replacementPlayer(recipient.who))) return false;
  // CR 122.6a: the actor is independent of the recipient. Each matching
  // recipient gets its own occurrence; the count is of the actual change.
  const scope: Context = { ...context, phase: "trigger-match", eventPredicate: true };
  this.walk(trigger, path, scope);
  const candidates = recipients.flatMap((recipient, index) => {
    const base = on.recipient === "any-of" ? `${path}/on/recipients/${index}` : child(path, "on");
    const candidate = this.at(child(base, recipient.recipient === "object" ? "what" : "who"));
    return candidate ? [candidate] : [];
  });
  const actor = by ? this.at(child(path, "by")) : undefined;
  if (candidates.length !== recipients.length || (by && !actor)) return false;
  const event = this.event(path, context, "trigger:counter-change", [...candidates, ...(actor ? [actor] : [])]);
  const rules = ["122.2", "122.6", "122.6a", "603.2c", "603.10", "700.1"];
  this.node({ id: this.id(path, "counter-change"), path, node: "counter-change-match", scope: context.ability,
    event, pattern: trigger, recipients: candidates.map(candidate => candidate.id), ...(actor ? { actor: actor.id } : {}),
    ...(gate ? { gate: { zone: "exile" as const, timing: "matching-event" as const } } : {}), rules });
  this.engineFeature("counter-change-event", path,
    "Match actual counter changes by kind, actor and recipient. Partition each event by recipient and apply the printed counter grouping.", rules);
  context.quantities = [this.eventResult(child(path, "count"), event, "quantity", "one", "measured-quantity")];
  const [candidate] = candidates;
  if (on.recipient === "object") {
    const result = this.eventResult(path, event, "objects", "one", "same-objects", candidate.description);
    context.objects = [result];
    if (candidate.id === context.source) this.sourceAliases.add(result.id);
    if (gate) this.bound(gate.condition.what, child(gate.path, "what"), result.id, ["122.2", "603.2c", "603.10"]);
  } else if (on.recipient === "player" && candidate.id !== context.you) {
    context.players = [this.eventResult(path, event, "player", "one", "same-objects")];
  } else if (on.recipient === "any-of") {
    // A mixed event does not make an object or player available in every outcome.
    context.mentionedObject = true;
  }
  return true;
}

function printedTrigger(analyzer: Analyzer, path: string, context: Context): Extract<Ability, { kind: "triggered" }> | undefined {
  const root = context.lexicalAbilityPath;
  if (context.barrier || !root || path !== `${root}/trigger` || context.ability !== analyzer.id(root, "ability")) return;
  const ability = analyzer.abilityAsts.get(root) as Ability | undefined;
  if (ability?.kind === "triggered") return ability;
}

// CR 603.2, 603.2c and 603.6a: both alternatives provide the same singular
// entrant role. The entrant deals the damage even if it later leaves.
// https://magic.wizards.com/en/news/feature/iconic-masters-release-notes-2017-09-11
function enteringDamageSource(analyzer: Analyzer, trigger: Extract<Trigger, { trigger: "any-of" }>,
  path: string, context: Context, branches: Context[]): Candidate | undefined {
  const ability = printedTrigger(analyzer, path, context);
  const damage = ability?.effects[0]?.steps[0];
  const second = trigger.events[1];
  if (damage?.effect !== "damage" || second?.trigger !== "enters" || second.what.ref !== "filter" ||
      second.what.filter.subtypes?.length !== 1) return;
  const { subtypes } = second.what.filter;
  if (!isDeepStrictEqual(trigger, { trigger: "any-of", events: [
    { trigger: "enters", what: { ref: "this", noun: "creature" } },
    { trigger: "enters", what: { ref: "filter", filter: { other: true, subtypes, control: "you" },
      count: { amount: "fixed", value: 1 } } },
  ] }) || !isDeepStrictEqual(ability?.effects, [{
    steps: [{ effect: "damage", source: { ref: "it" }, amount: { amount: "x" }, to: damage.to }],
    bindings: [{ variable: "x", value: { amount: "count", of: { subtypes, control: "you" } } }],
  }]) || damage.to.to !== "any-target" || !context.permanentSource ||
      !branches.every(branch => branch.objects.length === 1 && branch.objects[0].domain === "object" &&
        branch.objects[0].cardinality === "one" && branch.players.length === 0)) return;
  const id = analyzer.node({ id: analyzer.id(path, "entering-damage-source"), path, node: "alternatives",
    scope: context.ability, of: branches.map(branch => branch.objects[0].id), rules: ["603.2", "603.2c", "603.6a"] });
  return { id, cardinality: "one", domain: "object" };
}

// CR 508.3d and 603.2c: count the qualifying attackers in the one
// declaration, even if some are gone by the time the ability resolves.
function qualifyingAttackCount(analyzer: Analyzer, trigger: Extract<Trigger, { trigger: "player-attacks" }>,
  path: string, context: Context): boolean {
  const { with: filter } = trigger;
  const ability = printedTrigger(analyzer, path, context);
  const first = ability?.effects[0]?.steps[0];
  const among = first?.effect === "mill" || first?.effect === "draw" || first?.effect === "gain-life" ? first.amount
    : first?.effect === "create-token" ? first.count : undefined;
  if (!filter || !isDeepStrictEqual(among, { amount: "that-much" }) ||
      !(isDeepStrictEqual(filter, { types: ["creature"] }) ||
        isDeepStrictEqual(filter, { types: ["creature"], supertypes: ["legendary"] }) ||
        (filter.subtypes?.length === 1 && isDeepStrictEqual(filter, { subtypes: filter.subtypes }))) ||
      !isDeepStrictEqual(trigger, { trigger: "player-attacks", who: { player: "you" }, per: "declaration",
        with: filter, count: { op: "ge", value: { amount: "fixed", value: 1 } } })) return false;
  const actor = analyzer.player(trigger.who, child(path, "who"), { ...context, phase: "trigger-match" });
  if (!actor) return false;
  const rules = ["508.1", "508.3d", "603.2c"];
  const event = analyzer.event(path, context, "trigger:player-attacks", [actor]);
  analyzer.node({ id: analyzer.id(path, "attack-declaration"), path, node: "attack-declaration", scope: context.ability,
    event, actor: actor.id, filter, count: trigger.count, rules });
  context.quantities = [analyzer.eventResult(child(path, "count"), event, "quantity", "one", "measured-quantity")];
  analyzer.engineFeature("attack-declaration", path, "Count the matching attackers in the declaration that triggered this ability.", rules);
  return true;
}

export function triggerPattern(this: Analyzer, trigger: Trigger, path: string, context: Context): void {
  if (trigger.trigger === "player-attacks" && qualifyingAttackCount(this, trigger, path, context)) return;
  if (trigger.trigger === "phase" && trigger.firstEachTurn && !context.barrier) {
    const rules = ["303.4b", "500.11", "503.1", "603.2c", "614.10a"];
    const enchanted = this.player(trigger.whose, child(path, "whose"), { ...context, phase: "trigger-match" });
    if (!enchanted) return;
    const event = this.event(path, context, "trigger:phase", [enchanted]);
    const occurrence = this.node({ id: this.id(path, "first-upkeep-start"), path, node: "first-upkeep-start",
      scope: context.ability, event, enchantedPlayer: enchanted.id, occurrence: "first-actual-upkeep-of-turn",
      attachmentAt: "trigger-match", rules });
    const player = this.eventResult(path, event, "player", "one", "same-objects");
    context.firstUpkeep = { scope: context.ability, occurrence, player };
    context.players = [player];
    this.engineFeature("phase-scheduling", path,
      "Match only the first upkeep that starts in the turn and capture its step and the matched enchanted player at trigger time.", rules);
    return;
  }
  if (trigger.trigger === "player-attacks" && !context.barrier && trigger.per === "declaration" &&
      trigger.who.player === "you" && trigger.whom &&
      (trigger.whom.player === "a-player" || trigger.whom.player === "the-initiative-holder") &&
      !trigger.whomPermanents && !trigger.whomGrouping && !trigger.totalPower &&
      (!trigger.count || trigger.count.value.amount === "fixed")) {
    const match: Context = { ...context, phase: "trigger-match", eventPredicate: true };
    this.walkChildren(trigger as unknown as Record<string, unknown>, path, match);
    const actor = this.at(child(path, "who"));
    const defender = this.at(child(path, "whom"));
    if (actor && defender && defender.cardinality === "one") {
      const rules = ["508.1k", "508.3d", "603.2c", "726.1"];
      const event = this.event(path, context, "trigger:player-attacks", [actor, defender]);
      this.node({ id: this.id(path, "attack-declaration"), path, node: "attack-declaration", scope: context.ability,
        event, actor: actor.id, defender: defender.id, ...(trigger.with ? { filter: trigger.with } : {}),
        ...(trigger.count ? { count: trigger.count } : {}), rules });
      this.engineFeature("attack-declaration", path, "Match the one qualifying declaration, including its stated defender and attacker predicate.", rules);
      context.players = [this.eventResult(path, event, "player", "one", "same-objects")];
      return;
    }
  }
  if (trigger.trigger === "is-attacked" && !context.barrier) {
    const who = this.player(trigger.who, child(path, "who"), { ...context, phase: "trigger-match", eventPredicate: true });
    if (who?.cardinality === "one") {
      const event = this.event(path, context, "trigger:is-attacked", [who]);
      const player = this.eventResult(path, event, "player", "one", "same-objects");
      context.attackedTriggerPlayer = player;
      context.players = [player];
      return;
    }
  }
  if (trigger.trigger === "counter-change") {
    if (this.counterChange(trigger, path, context)) return;
    const barrier = "Counter-change triggers need per-counter/per-recipient/group occurrence, placing/removing actor, and event quantity/recipient captures bound.";
    this.diagnostic("unsupported-scope", path, barrier, ["122.2", "122.6", "122.7", "603.2c"]);
    this.walk(trigger, path, { ...context, phase: "trigger-match", barrier });
    return;
  }
  if (trigger.trigger === "event-gated") {
    const { event, gate } = trigger;
    if (event.trigger === "counter-change" && event.direction === "remove" && event.on.recipient === "object" &&
        isDeepStrictEqual(event.on.what, { ref: "this", noun: "card" }) && gate.gate === "condition" &&
        gate.condition.condition === "is-filter" &&
        isDeepStrictEqual(gate.condition, { condition: "is-filter", what: { ref: "it" }, filter: { zone: { zone: "exile" } } }) &&
        this.counterChange(event, child(path, "event"), context, { path: `${path}/gate/condition`, condition: gate.condition }))
      return;
    const barrier = "Trigger event-time conditions need the matching event's participant, source snapshot, and branch-local gate bindings.";
    this.diagnostic("unsupported-scope", path, barrier, ["603.1", "603.4", "603.10"]);
    this.walk(trigger, path, { ...context, phase: "trigger-match", barrier });
    return;
  }
  if (trigger.trigger === "becomes-class-level") {
    if (!context.barrier && context.classLevel === trigger.level &&
        (trigger.what.ref === "self" || trigger.what.ref === "this")) {
      const what = this.object(trigger.what, child(path, "what"), { ...context, phase: "trigger-match" });
      if (what) {
        this.event(path, context, "trigger:becomes-class-level", [what]);
        return;
      }
    }
    const barrier = "A Class-level trigger needs the source incarnation's designation transition and granted-ability availability bound.";
    this.diagnostic("unsupported-scope", path, barrier, ["716.2", "603.2"]);
    this.walk(trigger, path, { ...context, barrier });
    return;
  }
  if (trigger.trigger === "roll-dice") {
    const barrier = "Dice triggers need per-roll/group occurrence, ignored-result exclusion, natural/final stage, and result capture scopes.";
    this.diagnostic("unsupported-scope", path, barrier, ["706.2", "706.6", "706.7"]);
    this.walk(trigger, path, { ...context, barrier });
    return;
  }
  // A trigger pattern denotes the object/player of the matching occurrence,
  // not every object satisfying its filter at resolution (CR 603.2c).
  if (trigger.trigger === "any-of") {
    // One ability triggers on whichever printed event occurs (CR 603.2).
    // Each alternative binds on its own; what the effect may refer to is
    // what every alternative establishes alike ("Whenever this creature
    // attacks or blocks, ... it"). If any alternative moves the source, it
    // may have left its zone (CR 603.10a).
    const branches = trigger.events.map((event, index) => {
      const branch: Context = { ...context, objects: [], players: [], quantities: [], departed: [] };
      this.triggerPattern(event, child(child(path, "events"), index), branch);
      return { branch, subject: "what" in event && event.what ? this.at(child(child(child(path, "events"), index), "what")) : undefined,
        who: "who" in event && event.who ? this.at(child(child(child(path, "events"), index), "who")) : undefined };
    });
    context.sourceMoved = context.sourceMoved || branches.some(({ branch }) => branch.sourceMoved);
    const [first] = branches;
    const publicDeparture = trigger.events.every(event => event.trigger === "dies") &&
      branches.every(({ branch }) => branch.departed?.length === 1 && branch.departed[0].cardinality === "one" &&
        branch.objects.length === 1 && branch.objects[0].triggerSuccessor);
    const sourceEntryAndExit = trigger.events.length === 2 &&
      trigger.events.some(event => event.trigger === "enters") && trigger.events.some(event => event.trigger === "leaves") &&
      branches.every(({ subject }) => subject?.id === context.source);
    if (!context.barrier && (publicDeparture || sourceEntryAndExit)) {
      context.zoneAlternatives = branches.flatMap(({ branch }, index) => {
        const kind = trigger.events[index].trigger;
        if (kind !== "dies" && kind !== "leaves" && kind !== "enters") return [];
        const event = this.id(`${path}/events/${index}`, "event");
        return [{ kind, event, ...(kind === "dies" ? { previous: branch.departed![0].id, successor: branch.objects[0].id } : {}) }];
      });
    }
    const sourceTypeLine = this.result.nodes.find(node => node.id === context.face);
    const equipmentEntry = trigger.events.length === 2 && trigger.events.every(event => event.trigger === "enters") &&
      trigger.events.some(event => event.trigger === "enters" && (event.what.ref === "self" || event.what.ref === "this")) &&
      trigger.events.some(event => event.trigger === "enters" && event.what.ref === "filter" &&
        isDeepStrictEqual(event.what.filter.subtypes, ["Equipment"])) &&
      sourceTypeLine?.node === "face" && /[—-]\s.*\bEquipment\b/.test(sourceTypeLine.typeLine ?? "") &&
      branches.every(({ branch }) => branch.objects.length === 1 && branch.objects[0].cardinality === "one");
    // An alternative with another participant ("blocks or becomes blocked
    // by a creature") makes "that creature" a different object per event;
    // that needs a discriminated binding, so nothing is established.
    // "Blocks or becomes blocked by a creature": each alternative has one
    // other creature, described alike; "that creature" is the one the event
    // that occurred established.
    const others = branches.map(({ branch }) => branch.objects.filter(candidate => !this.sourceAliases.has(candidate.id)));
    if (!context.barrier && branches.length > 1 && branches.every(({ branch }) => branch.sourceSuccessor?.cardinality === "one")) {
      // Each disjunct moves this same source to a public zone. The
      // resulting object depends on which trigger event actually occurred.
      const rules = ["400.7e", "603.2", "603.6c"];
      const id = this.node({ id: this.id(path, "successors"), path, node: "alternatives", scope: context.ability,
        of: branches.map(({ branch }) => branch.sourceSuccessor!.id), rules });
      const successor: Candidate = { id, domain: "object", cardinality: "one", description: { cls: "card" },
        successorOf: context.source, triggerSuccessor: true };
      context.sourceSuccessor = successor;
      context.objects = [successor];
    } else if (!context.barrier && equipmentEntry && !context.sourceMoved) {
      const rules = ["201.5", "301.5", "603.2c"];
      const id = this.node({ id: this.id(path, "entering-equipment"), path, node: "alternatives", scope: context.ability,
        of: branches.map(({ branch }) => branch.objects[0].id), rules });
      context.objects = [{ id, domain: "object", cardinality: "one", description: { subtypes: ["Equipment"] } }];
    } else if (!context.sourceMoved && branches.length > 1 && others.every(list => list.length === 1 && list[0].cardinality === "one")
      && others.every(list => JSON.stringify(list[0].description) === JSON.stringify(others[0][0].description))) {
      const id = this.node({ id: this.id(path, "alternatives"), path, node: "alternatives", scope: context.ability, of: others.map(list => list[0].id), rules: ["603.2"] });
      context.objects = [{ id, cardinality: "one", domain: "object", description: others[0][0].description }];
      context.mentionedObject = true;
    } else if (branches.some(({ branch }) => branch.objects.length > 1)) context.mentionedObject = true;
    else if (!context.sourceMoved && first.subject && branches.every(({ subject }) => subject?.id === first.subject!.id && subject.cardinality === "one"))
      context.objects = [first.subject];
    if (first.who && branches.every(({ who }) => who?.id === first.who!.id) && first.who.id !== context.you)
      context.players = [first.who];
    const entrant = enteringDamageSource(this, trigger, path, context, branches.map(({ branch }) => branch));
    if (entrant) {
      context.objects = [entrant];
      context.mentionedObject = true;
    }
    return;
  }
  if (trigger.trigger === "phase-in-turn" && context.delayedSchedule && !context.barrier &&
      isDeepStrictEqual(trigger.turn, { turn: "next" })) {
    const rules = ["500.7", "603.7b", "614.10a"];
    this.node({ id: this.id(path, "scheduled-phase"), path, node: "scheduled-phase",
      scope: context.ability, phase: trigger.phase, turn: "next", rules });
    this.engineFeature("phase-scheduling", path, "Anchor the delayed trigger to the stated phase of the next turn.", rules);
    return;
  }
  const match = { ...context, phase: "trigger-match" as const };
  if (!context.barrier && trigger.trigger === "cast" && trigger.from?.owner === "their" &&
      (trigger.who?.player === "a-player" || trigger.who?.player === "an-opponent")) {
    // The origin is the casting player's zone at the matched occurrence.
    // Another trigger or a later pronoun cannot supply this role (CR 603.2c).
    match.actor = this.player(trigger.who, child(path, "who"), { ...match, eventPredicate: true });
    for (const [key, value] of Object.entries(trigger)) if (key !== "who") this.walk(value, child(path, key), match);
  } else this.walk(trigger, path, match);
  const objects: Candidate[] = [];
  if ("what" in trigger && trigger.what && "ref" in trigger.what) {
    const what = this.at(child(path, "what"));
    if (what) objects.push(what);
  }
  const who = "who" in trigger ? this.at(child(path, "who")) : undefined;
  const recipient = "to" in trigger && trigger.to && trigger.to !== "any" ? this.at(child(path, "to")) : undefined;
  const event = this.event(path, context, `trigger:${trigger.trigger}`, [...objects, ...(who ? [who] : [])]);
  const stackEvent = uniqueStackTriggerEvent(trigger);
  const eventObject = objects.length ? this.eventResult(path, event, "objects",
    trigger.grouping === "one-or-more" ? "many" : "one", "same-objects", objects[0].description)
    : stackEvent ? this.eventResult(path, event, "objects", "one", "same-objects",
      stackEvent === "cast" ? { cls: "spell" } : { cls: "ability", abilityKind: "activated", manaAbility: false }) : undefined;
  // Preserve the older ordinary-pronoun policy. A filter-only cast trigger
  // establishes this explicit event role, not a generic latest-object alias.
  if (objects.length && eventObject) context.objects = [eventObject];
  // A trigger whose subject is the source or another definite description
  // ("equipped creature") names that object explicitly when it means it.
  if (eventObject && "what" in trigger && trigger.what && "ref" in trigger.what
    && ["self", "this", "equipped", "enchanted", "fortified"].includes(trigger.what.ref)) this.sourceAliases.add(eventObject.id);
  if (stackEvent && eventObject) context.triggerObject = { event: stackEvent, candidate: eventObject };
  if (trigger.trigger === "leaves" && (trigger.what.ref === "self" || trigger.what.ref === "this") &&
      eventObject?.cardinality === "one" && objects[0]?.id === context.source) context.ownLeaveEvent = event;
  if (trigger.trigger === "put-into-zone" && trigger.to.zone === "stack" &&
      eventObject?.cardinality === "one" && eventObject.description && stackClasses(eventObject.description))
    context.stackPlacement = eventObject;
  if (["attacks", "attacks-and-isnt-blocked", "becomes-blocked"].includes(trigger.trigger) && eventObject?.cardinality === "one") context.attacker = eventObject;
  if (recipient) {
    if (recipient.domain === "player") context.players = combine(context.players, [recipient]);
    else context.objects = combine(context.objects, [recipient]);
  }
  // "Blocks a creature", "becomes blocked by a creature", "becomes unattached
  // from a permanent": the other object of the event.
  for (const key of ["by", "blocking", "from"] as const) {
    const other = key in trigger && isRecord((trigger as Record<string, unknown>)[key]) && "ref" in ((trigger as Record<string, unknown>)[key] as object)
      ? this.at(child(path, key)) : undefined;
    if (other?.domain === "object" && other.cardinality === "one") context.objects = combine(context.objects, [other]);
  }
  const related = !who && eventObject ? this.relationPlayer(eventObject, path, context) : undefined;
  if (related) context.players = combine(context.players, [related]);
  // "That player" never means the player the text calls "you".
  if (who && who.id !== context.you) context.players = [this.eventResult(path, event, "player", "one", "same-objects")];
  if (trigger.trigger === "cast" && trigger.what && !("ref" in trigger.what) && who?.cardinality === "one" && stackEvent === "cast")
    context.castActors = { caster: who.id === context.you ? who : context.players[0], filter: trigger.what };
  // "deals damage to a player" (one occurrence per damaged player), "at the
  // beginning of each player's upkeep": "that player" is the event's player.
  else if ((trigger.trigger === "deals-damage" && trigger.to === "any" && !trigger.recipients)
    || (trigger.trigger === "phase" && (trigger.whose === "each-player" || trigger.whose === "each-opponent")))
    context.players = [this.eventResult(path, event, "player", "one", "same-objects")];
  if ((trigger.trigger === "taps-for-mana" && who?.id !== context.you &&
      (trigger.who?.player === "a-player" || trigger.who?.player === "an-opponent")) ||
      (trigger.trigger === "phase" && trigger.phase === "draw-step" && trigger.whose === "each-player")) {
    if (context.players.length === 1 && context.players[0].cardinality === "one") context.definiteEventPlayer = context.players[0];
  }
  // "Becomes the target of a spell or ability ... counter that spell": the
  // spell or ability that targeted it.
  if (trigger.trigger === "becomes-target" && "of" in trigger && isRecord(trigger.of) && !("ref" in trigger.of))
    context.objects = combine(context.objects, [{ ...this.eventResult(child(path, "of"), event, "objects", "one", "same-objects", trigger.of as ObjectFilter), qualifiedOnly: true }]);
  // "Put into an opponent's graveyard", "returned to a player's hand": that
  // player is the card's owner (CR 400.3, 404.2).
  if (eventObject && eventObject.cardinality === "one" && "to" in trigger && isRecord(trigger.to) && "owner" in trigger.to && trigger.to.owner !== "your"
    && (trigger.to.owner === "an-opponent" || (isRecord(trigger.to.owner) && trigger.to.owner.player === "a-player"))) {
    const id = this.id(child(path, "to"), "owner-of-described");
    this.node({ id, path: child(path, "to"), node: "derived-player", scope: context.ability, role: "owner", of: eventObject.id, rules: ["108.3", "400.3"] });
    this.weakPlayers.add(id);
    context.players = combine(context.players, [{ id, cardinality: "one", domain: "player" }]);
  }
  // "At the beginning of enchanted player's upkeep, that player ...", "a
  // creature enchanted player controls": the player the trigger names.
  for (const at of [child(path, "whose"), child(child(child(path, "what"), "filter"), "controlledBy")]) {
    const named = this.at(at);
    if (named?.domain === "player" && named.cardinality === "one" && named.id !== context.you) context.players = combine(context.players, [named]);
  }
  if (["deals-damage", "is-dealt-damage", "player-dealt-damage", "gains-life", "loses-life", "pays-life"].includes(trigger.trigger)) {
    context.quantities = [this.eventResult(path, event, "quantity", "one", "measured-quantity")];
    if (trigger.trigger === "gains-life" && who?.id === context.you)
      context.lifeGain = { player: who.id, quantity: context.quantities[0].id };
  }
  // "Whenever one or more Dragons you control attack, draw that many cards":
  // how many objects the event had.
  else if ("grouping" in trigger && trigger.grouping === "one-or-more" && eventObject)
    context.quantities = [this.eventResult(child(path, "count"), event, "quantity", "one", "measured-quantity")];
  const publicDeparture = trigger.trigger === "put-into-zone" &&
    isDeepStrictEqual(trigger.from, { zone: "battlefield" }) && trigger.to.zone === "exile";
  if (publicDeparture || ["dies", "leaves", "leaves-zone", "put-into-graveyard", "enters-or-leaves", "enters-or-dies"].includes(trigger.trigger)) {
    if (objects.some(object => object.id === context.source)) context.sourceMoved = true;
    // The moved object is a new object (CR 400.7). The ability can find the
    // object it became in the public zone it first went to (CR 400.7e,
    // 603.6c): "When this creature dies, return it to its owner's hand".
    // Its last-known information answers "if it was a Human" or "its
    // power" (CR 603.10a, 608.2h).
    context.objects = [];
    if (eventObject) {
      context.departed = combine(context.departed ?? [], [eventObject]);
      if (trigger.trigger !== "enters-or-leaves" && trigger.trigger !== "enters-or-dies") {
        const id = this.node({ id: this.id(path, "successor"), path, node: "event-result", event, result: "objects",
          cardinality: eventObject.cardinality, availability: "if-produced", identity: "zone-successors",
          description: { ...(eventObject.description ?? {}), cls: "card" }, rules: ["400.7e", "603.6c"] });
        const successor: Candidate = { id, cardinality: eventObject.cardinality, domain: "object",
          description: { ...(eventObject.description ?? {}), cls: "card" }, successorOf: eventObject.id, triggerSuccessor: true };
        context.objects = [successor];
        if (context.sourceMoved && objects.some(object => object.id === context.source)) context.sourceSuccessor = successor;
      }
    }
  }
}
