// Single-template recognisers: linked exile pools and plays from exile.
import { isDeepStrictEqual } from "node:util";
import type { Ability, Effect, ObjectFilter, ObjectRef, Sentence } from "../../../ast.ts";
import { stepEventOperation } from "../../step-vocabulary.ts";
import type { BindingNode } from "../../types.ts";
import type { Analyzer } from "../analyzer.ts";
import { child, impliesPermanent, type Candidate, type Context } from "../context.ts";
import { hasExileInstruction } from "../replacement-shapes.ts";
import { HIDEAWAY_DAMAGE_PLAY, HIDEAWAY_PLAY, REVEAL_AND_CAST } from "../templates.ts";

export function hideawayPlay(this: Analyzer, path: string, pool: string, context: Context, play: typeof HIDEAWAY_PLAY | typeof HIDEAWAY_DAMAGE_PLAY): void {
  const at = child(path, "instruction");
  const { instruction } = play;
  const controller = this.bound(instruction.who, child(at, "who"), context.you, ["109.5"]);
  const cards = this.bound(instruction.what, child(at, "what"), pool, ["400.7", "607.2a", "607.3"]);
  this.walk(instruction.alternativeCost, child(at, "alternativeCost"), context);
  const conditionPath = child(at, "condition");
  this.walk(instruction.condition, conditionPath, context);
  const id = this.id(conditionPath, "condition");
  const condition = instruction.condition.condition === "compare"
    ? this.node({ id, path: conditionPath, node: "total-power-condition", scope: context.ability,
        creatures: this.at(`${conditionPath}/value/among`)!.id,
        comparison: instruction.condition.comparison, timing: "resolution", rules: ["608.2h"] })
    : this.node({ id, path: conditionPath, node: "damage-history-condition", scope: context.ability,
        opponentsOf: context.you, comparison: instruction.condition.comparison, timing: "resolution",
        during: "this-turn", quantifier: "any-one-opponent", rules: ["102.2", "120.3", "120.4b", "608.2i"] });
  this.event(path, context, HIDEAWAY_PLAY.effect, [cards, controller],
    { optional: true, decider: controller.id, conditional: true, condition });
  this.engineFeature("linked-exile-play", path,
    instruction.condition.condition === "compare"
      ? "During resolution, if the activator's creatures have total power at least ten, they may play each card still in this incarnation's linked exile pool. Apply land-play limits or cast without paying its mana cost, subject to legality."
      : "If one opponent received enough damage this turn, offer legal land plays and casts from the linked face-down exile pool.",
    ["113.7a", "305.2", "305.3", "406.3a", "607.2a", "607.3", "608.2g", "608.2h", "701.18b"]);
}
export function revealAndCast(this: Analyzer, effects: typeof REVEAL_AND_CAST, path: string, context: Context): void {
  const [shuffle, scan] = effects[0].steps;
  const [exile, bottom] = effects[1].steps;
  const [cast] = effects[2].steps;
  this.effect(shuffle, `${path}/0/steps/0`, context);

  const scanPath = `${path}/0/steps/1`;
  this.walkChildren(scan, scanPath, context);
  const controller = this.at(child(scanPath, "who"))!;
  const event = this.event(scanPath, context, scan.effect, [controller]);
  const pool = this.eventResult(scanPath, event, "objects", "many", "same-objects", { cls: "card" });
  const hit = this.eventResult(child(scanPath, "until"), event, "objects", "one", "same-objects", scan.until.matching, pool.id);
  context.objects = [hit];

  const exilePath = `${path}/1/steps/0`;
  this.effect(exile, exilePath, context);
  // "The rest" excludes the revealed library object, not its successor in exile.
  this.poolSelections.set(context.ability, { pool, selected: hit, path: exilePath });
  this.effect(bottom, `${path}/1/steps/1`, context);

  const castPath = `${path}/2/steps/0`;
  const instruction = child(castPath, "instruction");
  const successor = this.id(exilePath, "objects");
  this.walk(cast.instruction.who, child(instruction, "who"), context);
  const card = this.bound(cast.instruction.what, child(instruction, "what"), successor, ["400.7j", "608.2c"]);
  this.walk(cast.instruction.alternativeCost, child(instruction, "alternativeCost"), context);
  this.event(castPath, context, cast.effect, [card, controller], { optional: true, decider: controller.id, requiresResult: successor });
  this.engineFeature("reveal-and-cast", path,
    "Shuffle the controller's library, reveal through the first nonland or exhaustion, exile the match and put the other revealed cards on the bottom in random order. If the exile successor exists, the resolving spell's controller may cast that card during resolution without paying its mana cost if legal.",
    ["400.7j", "601.2", "608.2c", "608.2d", "608.2g", "609.3", "701.20", "701.24"]);
}
/** A scope marker is immaterial only here: granting permission for the
 * branch's sole actual choice has no effect if that choice did not occur.
 * CR 118.12, 608.2c–d and 611.2a. */
export function conditionalExilePermission(this: Analyzer, sentences: Sentence[], path: string, context: Context): boolean {
  if (sentences.length !== 3 || sentences[0].steps.length !== 1 || sentences[1].steps.length !== 2 ||
      sentences[2].steps.length !== 1 || Object.keys(sentences[0]).length !== 1 ||
      Object.keys(sentences[1]).length !== 3 || Object.keys(sentences[2]).length !== 2 ||
      sentences[1].conditionKind !== "if" || !isDeepStrictEqual(sentences[1].condition, { condition: "you-do" }) ||
      sentences[1].otherwise || sentences[1].afterConditional || sentences[2].afterConditional !== "scope-unresolved" ||
      context.exileResults?.length || context.uncertainExileOrigin) return false;
  const [discard] = sentences[0].steps;
  const [exile, choose] = sentences[1].steps;
  const [grant] = sentences[2].steps;
  const you = { player: "you" } as const;
  if (discard.effect !== "discard" || discard.random ||
      !isDeepStrictEqual({ ...discard, random: undefined }, { effect: "discard", who: you, optional: true, random: undefined,
        what: { ref: "filter", filter: { cls: "card" }, count: { amount: "fixed", value: 1 } } }) ||
      !isDeepStrictEqual(exile, { effect: "exile", what: {
        ref: "library-cards", position: "top", count: { amount: "fixed", value: 2 }, of: { zone: "library", owner: "your" },
      } }) || !isDeepStrictEqual(choose, { effect: "choose-object", who: you,
        what: { ref: "from-pool", pool: { ref: "them" }, count: { amount: "fixed", value: 1 } } }) ||
      !isDeepStrictEqual(grant, { effect: "grant-play-permission", duration: { duration: "this-turn" },
        permission: { who: you, action: "play", what: { ref: "that", filter: { cls: "card" } } },
      })) return false;
  const decision = `${path}/0/steps/0`;
  this.effect(discard, decision, context);
  const guard = `${path}/1/condition`;
  const rules = ["118.11", "118.12", "400.7j", "608.2c", "608.2d", "611.2a"];
  this.node({ id: this.id(guard, "decision-link"), path: guard, node: "decision-link", scope: context.ability,
    condition: "you-do", decision, rules });
  const branch = { ...this.branch(context), conditional: true };
  this.effect(exile, `${path}/1/steps/0`, branch);
  const select = `${path}/1/steps/1`;
  this.effect(choose, select, branch);
  const chosen = branch.directCardChoice;
  const at = `${path}/2/steps/0`;
  if (!chosen || chosen.path !== select) {
    this.diagnostic("unsupported-scope", at, "The play permission requires the conditional branch's actual chosen exile object.", rules);
    return true;
  }
  this.effect(grant, at, branch);
  this.node({ id: this.id(at, "exile-play-lifetime"), path: at, node: "exile-play-lifetime", scope: context.ability,
    exile: chosen.result.id, player: context.you, duration: "end-of-turn", requiresDecision: decision, rules });
  this.engineFeature("linked-exile-play", at,
    "If the discard was paid and an exiled card was chosen, grant normal play permission for that exact exile object this turn.", rules);
  this.blockBranchCaptures(context, [branch]);
  context.objects = [];
  context.quantities = [];
  return true;
}

/** CR 202.3, 400.7j and 608.2c–d. Each optional player chooses from
 * their own hand, and the grant sees only this pair of actual exiles. */
export function pairedHandExiles(this: Analyzer, sentences: Sentence[], path: string, context: Context): boolean {
  if (!context.spellAbility || context.uncertainExileOrigin || context.exileResults?.length ||
      sentences.length !== 3 || sentences.some(sentence => Object.keys(sentence).length !== 1) ||
      sentences[0].steps.length !== 2 || sentences[1].steps.length !== 2 || sentences[2].steps.length !== 1) return false;
  const [drawSelf, exileSelf] = sentences[0].steps;
  const [drawOther, exileOther] = sentences[1].steps;
  const [grant] = sentences[2].steps;
  const you = { player: "you" } as const;
  const opponent = { player: "that-player" } as const;
  const two = { amount: "fixed", value: 2 } as const;
  const counters = [{ counter: { type: "time" }, count: { amount: "attribute", of: { ref: "it" }, attribute: "mana-value" } }];
  const hand = (owner: "your" | typeof opponent) => ({ ref: "filter", filter: { nonTypes: ["land"], cls: "card", zone: { zone: "hand", owner } },
    count: { amount: "fixed", value: 1 } });
  const performed = { ref: "related", number: "plural", filter: { cls: "card" },
    relation: { relation: "event-result", event: "exile", scope: "this-way" } } as const;
  if (!isDeepStrictEqual(drawSelf, { effect: "draw", who: you, amount: two }) ||
      drawOther.effect !== "draw" || drawOther.who.player !== "target-opponent" || !drawOther.who.selectionId ||
      !isDeepStrictEqual(drawOther, { effect: "draw", who: { player: "target-opponent", selectionId: drawOther.who.selectionId }, amount: two }) ||
      !isDeepStrictEqual(exileSelf, { effect: "exile", what: hand("your"), counters, optional: true }) ||
      !isDeepStrictEqual(exileOther, { effect: "exile", what: hand(opponent), counters, optional: true, who: opponent, decidedBy: opponent }) ||
      !isDeepStrictEqual(grant, { effect: "gain-abilities", what: {
        ref: "each", filter: { cls: "card", withoutAbility: ["suspend"], fromAmong: performed },
      }, abilities: [{ gained: "keyword", keyword: { keyword: "suspend" } }] })) return false;
  const captures: Candidate[] = [];
  for (const [index, pair] of [[drawSelf, exileSelf], [drawOther, exileOther]].entries()) {
    const draw = `${path}/${index}/steps/0`;
    const at = `${path}/${index}/steps/1`;
    const scope = { ...this.branch(context), exileCounterFromSelected: at };
    this.effect(pair[0], draw, scope);
    this.effect(pair[1], at, scope);
    const actual = scope.exileResults?.length === 1 ? scope.exileResults[0] : undefined;
    if (actual?.event === this.id(at, "event")) captures.push(actual.result);
  }
  const rules = ["118.12", "202.3", "400.7j", "608.2c", "608.2d", "702.62b"];
  const at = `${path}/2/steps/0`;
  if (captures.length !== 2) {
    this.diagnostic("unsupported-scope", at, "The two hand exiles must establish independent actual results.", rules);
    return true;
  }
  const pool = this.node({ id: this.id(at, "two-hand-exiles"), path: at, node: "result-union", scope: context.ability,
    from: captures.map(candidate => candidate.id), cardinality: "many", rules });
  this.effect(grant, at, { ...context, objects: [], quantities: [],
    boundOperand: { path: `${at}/what/filter/fromAmong`, candidate: { id: pool, domain: "object", cardinality: "many",
      description: { cls: "card" } }, rules } });
  this.engineFeature("exile-and-search-results", at,
    "Record the two independent optional hand exile outcomes with their own chosen-card mana values. Give suspend only to the resulting cards without it.", rules);
  context.objects = [];
  context.quantities = [];
  context.mentionedObject = true;
  context.uncertainExileOrigin = true;
  return true;
}

/** A marked sentence can still have an explicit, earlier reveal pool.
 * Here the intervening condition grants a keyword only to the exile
 * successor and cannot affect that pool. CR 400.7j, 608.2c and 701.20. */
export function independentRevealRemainder(this: Analyzer, sentence: Sentence, prior: Sentence | undefined, priorPath: string, context: Context): boolean {
  const original = { ref: "related", number: "plural", filter: { cls: "card" },
    relation: { relation: "event-result", event: "reveal", scope: "contextual" } };
  const [scan] = context.scans ?? [];
  if (!scan || scan.operation !== "reveal" || context.scans?.length !== 1 || context.uncertainScanOrigin || context.uncertainRevealOrigin ||
      context.uncertainExileOrigin || context.exileResults?.length !== 1 || Object.keys(sentence).length !== 2 ||
      !isDeepStrictEqual(sentence.steps, [{ effect: "move-zone", what: { ref: "rest", of: original },
        to: { zone: "library", owner: "your", position: "bottom" }, order: "random",
      }]) || !isDeepStrictEqual(prior, {
        condition: { condition: "not", of: { condition: "has-keyword", what: { ref: "it" }, keyword: "suspend" } },
        conditionKind: "if", steps: [{ effect: "gain-abilities", what: { ref: "it" }, abilities: [{ gained: "keyword", keyword: { keyword: "suspend" } }] }],
      })) return false;
  const exile = context.exileResults[0];
  const source = this.result.nodes.find(node => node.id === exile.event);
  return this.at(`${priorPath}/steps/0/what`)?.id === exile.result.id &&
    source?.node === "event" && source.operation === "exile" && source.inputs.includes(scan.matches.id);
}

export function makeExileLedger(this: Analyzer, path: string, context: Context, writers: { event: string; result: string }[], rules: string[]): string {
  return this.node({ id: this.id(path, "linked-exile-ledger"), path, node: "linked-exile-ledger",
    scope: context.ability, source: context.source, writers, membership: "still-in-exile", identity: "source-incarnation",
    cardinality: "many", rules });
}

/** A ledger reader can name one exact group-seeding ability and its own
 * writer. Unrelated printed exile operations require a stronger linkage. */
export function linkedSeedPath(this: Analyzer, ability: Ability, abilities: Ability[], path: string, kind: "artifact" | "enchantment"): string | undefined {
  const self = { ref: "this", noun: kind };
  const enters = { trigger: "enters", what: self };
  const source = kind === "artifact" ? enters : {
    trigger: "any-of", events: [enters, { trigger: "loses-game", who: { player: "an-opponent" } }],
  };
  const group = kind === "artifact" ? { who: { player: "each-player" } } : {};
  const exile = { effect: "exile", what: { ref: "library-cards", position: "top", count: { amount: "fixed",
    value: kind === "artifact" ? 3 : 1 }, of: { zone: "library", owner: kind === "artifact" ? "their" : "each-player" } }, ...group };
  const producers = abilities.flatMap((candidate, index) => hasExileInstruction(candidate) ? [{ candidate, index }] : []);
  const seed = producers.find(({ candidate }) => candidate !== ability && candidate.kind === "triggered" &&
    !candidate.condition && isDeepStrictEqual(candidate.trigger, source) &&
    isDeepStrictEqual(candidate.effects, [{ steps: [exile] }]));
  if (producers.length !== 2 || !seed || !producers.some(entry => entry.candidate === ability)) return;
  const siblings = path.slice(0, path.lastIndexOf("/"));
  return `${siblings}/${seed.index}/effects/0/steps/0`;
}

export function seedLedgerWriter(this: Analyzer, path: string, rules: string[]): { event: string; result: string } {
  const event = this.id(path, "event");
  const cards = this.eventResult(child(path, "linked-cards"), event, "objects", "many", "zone-successors", { cls: "card" });
  this.engineFeature("linked-exile-pool", path,
    "Capture the actual cards exiled from each participating library as source-incarnation ledger inputs.", rules);
  return { event, result: cards.id };
}

/** CR 118.11–12 and 607.1c, 607.2a. The ability's immediate cast is
 * gated by the caster's mandatory action. Its actual exile output is
 * tracked separately for ledger insertion and exclusion. */
export function castFromSeededExile(this: Analyzer, ability: Extract<Ability, { kind: "triggered" }>, path: string, context: Context,
  abilities?: Ability[]): boolean {
  const { trigger, effects } = ability;
  const cast = context.triggerObject;
  const caster = context.castActors?.caster;
  const them = { player: "they" } as const;
  const definite = { player: "the-player" } as const;
  const source = { ref: "this", noun: "artifact" } as const;
  const other = { ref: "related", number: "plural", filter: { other: true, cls: "card" },
    relation: { relation: "linked-exile", source } } as const;
  if (context.barrier || !context.permanentSource || !abilities || ability.condition || cast?.event !== "cast" || !caster ||
      !isDeepStrictEqual(trigger, { trigger: "cast", who: { player: "a-player" }, what: { cls: "spell" },
        from: { zone: "hand", owner: "their" } }) || effects.length !== 2 ||
      !isDeepStrictEqual(effects[0], { steps: [{ effect: "exile", what: { ref: "it" }, who: { player: "that-player" } }] }) ||
      !isDeepStrictEqual(effects[1], { condition: { condition: "preceding-action", who: definite, outcome: "performed" },
        conditionKind: "if", steps: [{ effect: "play-card", optional: true, decidedBy: them, instruction: {
          who: them, action: "cast", what: { ref: "filter", filter: { cls: "spell" }, count: { amount: "fixed", value: 1 } },
          fromAmong: other, alternativeCost: { cost: "without-mana-cost" },
        } }],
      })) return false;
  const seed = this.linkedSeedPath(ability, abilities, path, "artifact");
  if (!seed) return false;
  const rules = ["107.3b", "118.9", "118.11", "118.12", "400.7j", "607.1c", "607.2a", "607.4", "607.5", "608.2g"];
  const at = `${path}/effects/0/steps/0`;
  this.effect(effects[0].steps[0], at, { ...context, actor: caster, boundOperand: {
    path: child(at, "what"), candidate: cast.candidate, rules,
  } });
  const paid = this.id(at, "event");
  const exiled = this.id(at, "objects");
  const condition = `${path}/effects/1/condition`;
  this.bound(definite, child(condition, "who"), caster.id, rules);
  const play = `${path}/effects/1/steps/0`;
  const from = `${play}/instruction/fromAmong`;
  const ledger = this.makeExileLedger(from, context, [this.seedLedgerWriter(seed, rules), { event: paid, result: exiled }], rules);
  this.walk(source, `${from}/relation/source`, context);
  const remaining = this.node({ id: this.id(from, "remainder"), path: from, node: "remainder", scope: context.ability,
    pool: ledger, excluding: exiled, rules });
  this.bound({ ref: "it" }, `${from}/filter/other`, exiled, rules);
  this.effect(effects[1].steps[0], play, { ...context, actor: caster, conditional: true, boundOperand: {
    path: from, candidate: { id: remaining, cardinality: "many", domain: "object", description: { cls: "card" } }, rules,
  } });
  this.node({ id: this.id(condition, "cast-after-exile-payment"), path: condition, node: "cast-after-exile-payment",
    scope: context.ability, caster: caster.id, spell: cast.candidate.id, payment: paid, actualExile: exiled,
    ledger, pool: remaining, cast: this.id(play, "event"), check: "mandatory-exile-payment-started", rules });
  this.engineFeature("linked-exile-play", play,
    "Only the triggering caster who starts paying the mandatory exile may cast one card from the linked pool during resolution. Exclude only the actual new exile card, if any.", rules);
  return true;
}

/** CR 603.11 and 607.2h, with normal play timing and one committed
 * use per player, source incarnation and turn across both alternatives.
 * https://magic.wizards.com/en/news/feature/adventures-forgotten-realms-release-notes-2021-07-09 */
export function sharedExilePlay(this: Analyzer, ability: Extract<Ability, { kind: "static" }>, path: string, context: Context, abilities?: Ability[]): boolean {
  const effect = ability.effect;
  if (context.barrier || !context.permanentSource || !abilities || effect.static !== "play-permission-choice") return false;
  const active = { player: "active-player" } as const;
  const source = { ref: "this", noun: "enchantment" } as const;
  const pool = { ref: "related", number: "plural", filter: { cls: "card" }, relation: { relation: "linked-exile", source } } as const;
  const count = { amount: "fixed", value: 1 } as const;
  const exile = { effect: "exile", what: { ref: "library-cards", position: "top", count,
    of: { zone: "library", owner: "their" } } } as const;
  const rider = (event: "play" | "cast") => ({ rider: "use-trigger", event, effects: [{ steps: [exile] }] });
  if (!isDeepStrictEqual(effect, { static: "play-permission-choice",
        useLimit: { maximum: 1, period: "turn", during: "active-player" }, permissions: [
          { who: active, action: "play", what: { ref: "filter", filter: { types: ["land"] }, count }, fromAmong: pool,
            riders: [rider("play")] },
          { who: active, action: "cast", what: { ref: "filter", filter: { cls: "spell" }, count }, fromAmong: pool,
            manaPayment: "any-color", riders: [rider("cast")] },
        ],
      })) return false;
  const seed = this.linkedSeedPath(ability, abilities, path, "enchantment");
  if (!seed) return false;
  const at = child(path, "effect");
  const rules = ["305.1", "305.2", "400.7", "601.2i", "603.7g", "603.9", "603.11", "607.2a", "607.2h", "607.5",
    "609.4b", "611.3", "712.11b", "712.11c", "712.12", "733.1", "800.4"];
  const actor = this.player(active, `${at}/useLimit/during`, context);
  if (!actor) return true;
  const writers = [this.seedLedgerWriter(seed, rules)];
  const alternatives: Extract<BindingNode, { node: "shared-exile-play" }>["alternatives"] = [];
  const ledgerId = this.id(at, "linked-exile-ledger");
  for (const [index, permission] of effect.permissions.entries()) {
    const option = `${at}/permissions/${index}`;
    const from = child(option, "fromAmong");
    this.bound(active, child(option, "who"), actor.id, rules);
    this.walk(permission.what, child(option, "what"), { ...context, actor });
    this.bound(pool, from, ledgerId, rules);
    this.walk(source, `${from}/relation/source`, context);
    this.node({ id: this.id(child(from, "relation"), "linked-exile-filter"), path: child(from, "relation"),
      node: "linked-exile-filter", scope: context.ability, pool: ledgerId, rules });
    const used = this.event(option, context, `permission-used:${permission.action}`, [actor]);
    const user = this.eventResult(option, used, "player", "one", "same-objects");
    const riderPath = `${option}/riders/0`;
    const you = this.id(riderPath, "you");
    const refill = this.nestedAbility(riderPath, { ...context, you }, "delayed-triggered", rules);
    this.node({ id: you, path: riderPath, node: "runtime-player", scope: refill.ability, role: "trigger-controller", rules });
    const top = `${riderPath}/effects/0/steps/0`;
    this.effect(exile, top, { ...refill, actor: user, objects: [], players: [], quantities: [] });
    writers.push({ event: this.id(top, "event"), result: this.id(top, "objects") });
    alternatives.push({ action: permission.action === "play" ? "play-land" : "cast-spell", use: used, player: user.id,
      refill: refill.ability, ...(permission.manaPayment ? { mana: "any-color" as const } : {}) });
  }
  this.makeExileLedger(at, context, writers, rules);
  this.node({ id: this.id(at, "shared-exile-play"), path: at, node: "shared-exile-play", scope: context.ability,
    ledger: ledgerId, player: actor.id, alternatives, maximum: 1,
    period: "actual-turn-per-player-and-source-incarnation", commit: "completed-use-of-this-permission",
    refillController: "source-controller-at-use", candidate: "proposed-face-for-action", rules });
  this.engineFeature("linked-exile-play", at,
    "Share one normal-timing land play or spell cast from this source's exile ledger during the active player's turn. A completed use spends the permission and triggers the refill for that exact user.", rules);
  return true;
}

/** CR 608.2h: the scan compares to the triggering spell as it
 * last existed, even if exiling it failed. Only the actual match can be cast. */
export function scanAgainstCastSpell(this: Analyzer, ability: Extract<Ability, { kind: "triggered" }>, path: string, context: Context,
  siblings?: Ability[]): boolean {
  const { trigger, effects } = ability;
  const cast = context.triggerObject;
  const caster = context.castActors?.caster;
  const player = { player: "that-player" } as const;
  const it = { ref: "it" } as const;
  const match = { ref: "that", filter: { cls: "card" } } as const;
  const source = { ref: "this", noun: "enchantment" } as const;
  const linked = { relation: "linked-exile", source } as const;
  if (context.barrier || ability.condition || !context.permanentSource || cast?.event !== "cast" || !caster ||
      siblings?.filter(hasExileInstruction).length !== 1 ||
      !isDeepStrictEqual(trigger, { trigger: "cast", who: { player: "a-player" }, what: { cls: "spell" },
        from: { zone: "hand", owner: "their" } }) ||
      effects.length !== 3 || effects.some(sentence => Object.keys(sentence).length !== 1) ||
      effects[0].steps.length !== 2 || effects[1].steps.length !== 1 || effects[2].steps.length !== 1) return false;
  const [exile, scan] = effects[0].steps;
  const [play] = effects[1].steps;
  const [clean] = effects[2].steps;
  if (!isDeepStrictEqual(exile, { effect: "exile", what: it, who: player }) || scan.effect !== "scan-library" ||
      !isDeepStrictEqual(scan, { effect: "scan-library", who: player, operation: "exile", library: { zone: "library", owner: "their" },
        until: { count: { amount: "fixed", value: 1 }, matching: { cls: "card", characteristicRelations: [
          { relation: "shared-value", characteristic: "card-type", with: { operand: "object", what: it } },
        ] } },
      }) || !isDeepStrictEqual(play, { effect: "play-card", optional: true, decidedBy: player,
        instruction: { who: player, action: "cast", what: match, alternativeCost: { cost: "without-mana-cost" } },
      }) || clean.effect !== "move-zone" || !isDeepStrictEqual(clean, { effect: "move-zone",
        what: { ref: "all", filter: { cls: "card", relation: linked } }, to: { zone: "library", owner: "their", position: "bottom" },
        order: "random", who: { player: "they" },
      })) return false;

  const rules = ["400.3", "400.7j", "607.1c", "607.2a", "607.5", "608.2g", "608.2h", "609.3", "701.13"];
  const at = `${path}/effects/0/steps/0`;
  this.effect(exile, at, { ...context, actor: caster, boundOperand: { path: child(at, "what"), candidate: cast.candidate, rules } });
  const exileEvent = this.id(at, "event");
  const first = this.id(at, "objects");
  const scanPath = `${path}/effects/0/steps/1`;
  const scanScope: Context = { ...context, actor: caster, boundOperand: {
    path: `${scanPath}/until/matching/characteristicRelations/0/with/what`, candidate: cast.candidate, rules,
  } };
  if (!this.libraryScan(scan, scanPath, scanScope) || scanScope.scans?.length !== 1) {
    this.diagnostic("unsupported-scope", scanPath, "The scan must produce a distinct full exile group and a singleton match.", rules);
    return true;
  }
  const capture = scanScope.scans[0];
  const playPath = `${path}/effects/1/steps/0`;
  this.effect(play, playPath, { ...context, actor: caster,
    boundOperand: { path: `${playPath}/instruction/what`, candidate: capture.matches, rules } });
  const cleanPath = `${path}/effects/2/steps/0`;
  const ledger = this.makeExileLedger(cleanPath, context, [{ event: exileEvent, result: first },
    { event: capture.event, result: capture.pool.id }], rules);
  this.walk(source, `${cleanPath}/what/filter/relation/source`, context);
  this.node({ id: this.id(`${cleanPath}/what/filter/relation`, "linked-exile-filter"),
    path: `${cleanPath}/what/filter/relation`, node: "linked-exile-filter", scope: context.ability, pool: ledger, rules });
  this.effect(clean, cleanPath, { ...context, actor: caster, boundOperand: {
    path: child(cleanPath, "what"), candidate: { id: ledger, domain: "object", cardinality: "many", description: { cls: "card" } }, rules,
  } });
  this.node({ id: this.id(cleanPath, "linked-scan-cleanup"), path: cleanPath, node: "linked-scan-cleanup",
    scope: context.ability, ledger, caster: caster.id, spell: cast.candidate.id, match: capture.matches.id,
    cast: this.id(playPath, "event"), cleanup: this.id(cleanPath, "event"), destination: "owners-libraries-random",
    rules });
  this.engineFeature("linked-exile-pool", cleanPath,
    "Keep both exile origins in the same source incarnation's ledger. Compare the scan to the original spell's stack characteristics, offer only the actual singleton match, and bottom the remaining linked cards in their owners' libraries.", rules);
  return true;
}

/** CR 608.2d–h and 701.13. Scan each library independently and offer
 * only the complete actual exile pool minus the one overridden choice. */
export function scansWithOneExcludedCard(this: Analyzer, sentences: Sentence[], path: string, context: Context): boolean {
  if (sentences.length !== 3 || sentences.some(sentence => Object.keys(sentence).length !== 1 || sentence.steps.length !== 1)) return false;
  const [[scan], [choice], [play]] = sentences.map(sentence => sentence.steps);
  const each = { player: "each-player" } as const;
  const opponent = { player: "an-opponent" } as const;
  const you = { player: "you" } as const;
  const predicate = { nonTypes: ["land"], cls: "card" } satisfies ObjectFilter;
  const relation = { relation: "event-result", event: "exile", scope: "this-way" } as const;
  const one = { amount: "fixed", value: 1 } as const;
  const selector = { ref: "filter", filter: { ...predicate, relation }, count: one } satisfies ObjectRef;
  const eligible = { ref: "related", number: "plural", filter: { other: true, cls: "card" }, relation } as const;
  if (scan.effect !== "scan-library" || !isDeepStrictEqual(scan, { effect: "scan-library", who: each, operation: "exile",
        library: { zone: "library", owner: "their" }, until: { count: one, matching: predicate } }) ||
      !isDeepStrictEqual(choice, { effect: "choose-object", who: opponent, what: selector }) ||
      play.effect !== "play-card" || !isDeepStrictEqual(play, { effect: "play-card", optional: true,
        instruction: { who: you, action: "cast", what: { ref: "filter", filter: { cls: "spell" },
          count: { amount: "fixed", value: 2 }, upTo: true }, fromAmong: eligible, alternativeCost: { cost: "without-mana-cost" } },
      })) return false;
  const rules = ["101.4", "400.7j", "608.2d", "608.2f", "608.2g", "608.2h", "609.3", "701.13"];
  const at = `${path}/0/steps/0`;
  const players = this.player(each, child(at, "who"), context);
  if (!players) return true;
  const member = this.node({ id: this.id(at, "library-member"), path: at, node: "instruction-member",
    scope: context.ability, group: players.id, domain: "player", rules });
  const owner: Candidate = { id: member, domain: "player", cardinality: "one" };
  this.walk(scan.library, child(at, "library"), { ...context, actor: owner });
  const scanEvent = this.event(at, context, stepEventOperation("scan-library"), [players]);
  const pool = this.eventResult(at, scanEvent, "objects", "many", "zone-successors", { cls: "card" });
  const matches = this.eventResult(child(at, "until"), scanEvent, "objects", "many", "zone-successors", predicate, pool.id);
  this.node({ id: this.id(at, "grouped-library-scan"), path: at, node: "grouped-library-scan", scope: context.ability,
    event: scanEvent, group: players.id, member, pool: pool.id, matches: matches.id,
    matching: predicate, maximumMatchesPerLibrary: 1, rules });
  const choicePath = `${path}/1/steps/0`;
  const choose = this.player(opponent, child(choicePath, "who"), context);
  if (!choose) return true;
  const potential = this.node({ id: this.id(choicePath, "eligible-exile-block"), path: child(choicePath, "what"),
    node: "filtered-result-group", scope: context.ability, pool: pool.id, filter: predicate, cardinality: "many", rules });
  this.bound(selector, child(choicePath, "what"), potential, rules);
  const candidates: Candidate = { id: potential, domain: "object", cardinality: "many", description: predicate };
  const choiceEvent = this.event(choicePath, context, "choose-object", [choose, candidates]);
  const excluded = this.eventResult(choicePath, choiceEvent, "objects", "one", "same-objects", predicate, potential);
  const playPath = `${path}/2/steps/0`;
  const available = `${playPath}/instruction/fromAmong`;
  const remainder = this.node({ id: this.id(available, "remainder"), path: available, node: "remainder",
    scope: context.ability, pool: pool.id, excluding: excluded.id, rules });
  this.bound({ ref: "it" }, `${available}/filter/other`, excluded.id, rules);
  this.effect(play, playPath, { ...context, boundOperand: { path: available, candidate: {
    id: remainder, domain: "object", cardinality: "many", description: { cls: "card" },
  }, rules } });
  this.node({ id: this.id(playPath, "excluded-scan-cast"), path: playPath, node: "excluded-scan-cast", scope: context.ability,
    chooser: context.you, opponent: choose.id, exclude: excluded.id, pool: remainder, cast: this.id(playPath, "event"),
    maximum: 2, order: "one-legal-cast-at-a-time", rules });
  this.engineFeature("library-scan", at,
    "Scan each player's library separately and retain the full actual exile set. The ability's controller picks the opponent who removes one eligible card from the casting choices.", rules);
  this.engineFeature("sequential-cast", playPath,
    "Allow up to two legal casts one at a time from the complete exile pool other than the chosen card.", rules);
  context.objects = [];
  context.players = [];
  context.quantities = [];
  context.uncertainExileOrigin = true;
  context.uncertainScanOrigin = true;
  return true;
}

/** CR 400.7j and 611.2a. A permission following an exact exile
 * of one card uses that incarnation, including when other participants
 * of the ability were battlefield permanents. */
export function immediateExilePlay(this: Analyzer, effect: Effect, path: string, context: Context, precedingEffect?: string): Context["boundOperand"] {
  if (context.barrier || context.decider || context.conditional || context.uncertainExileOrigin ||
      effect.effect !== "grant-play-permission" || Object.keys(effect).length !== 3 ||
      !isDeepStrictEqual({ ...effect.permission, what: undefined }, { who: { player: "you" }, action: "play", what: undefined }) ||
      !(effect.permission.what.ref === "it" || isDeepStrictEqual(effect.permission.what, { ref: "that", filter: { cls: "card" } })) ||
      !["your-next-end-step", "end-of-turn", "this-turn"].includes(effect.duration.duration) ||
      context.exileResults?.length !== 1 || !precedingEffect || context.precedingSentence?.steps.length !== 1) return;
  const [previous] = context.precedingSentence.steps;
  const [exile] = context.exileResults;
  if (previous.effect !== "exile" || !isDeepStrictEqual({ ...previous, faceDown: undefined }, { effect: "exile", faceDown: undefined, what: {
        ref: "library-cards", position: "top", count: { amount: "fixed", value: 1 }, of: { zone: "library", owner: "your" },
      } }) || exile.event !== this.id(precedingEffect, "event") || exile.result.cardinality !== "one" ||
      (!exile.faceDown && (effect.permission.what.ref !== "it" || effect.duration.duration === "this-turn")) ||
      context.objects.some(candidate => candidate.id !== exile.result.id &&
        (!candidate.description || !impliesPermanent(candidate.description)))) return;
  const rules = ["400.7", "400.7j", "305.1", "601.3", "611.2a", ...(exile.faceDown ? ["406.3a"] : [])];
  this.node({ id: this.id(path, "exile-play-lifetime"), path, node: "exile-play-lifetime", scope: context.ability,
    exile: exile.result.id, player: context.you,
    duration: effect.duration.duration === "your-next-end-step" ? "your-next-end-step" : "end-of-turn",
    ...(exile.faceDown ? { visibility: "turn-face-up-before-announcement-unless-cast-face-down" as const } : {}), rules });
  this.engineFeature("linked-exile-play", path,
    "Allow the player to play that exact exile card with normal timing and costs up to the printed deadline.", rules);
  return { path: `${path}/permission/what`, candidate: exile.result, rules };
}

export function lookAtFaceDownExile(this: Analyzer, effect: Extract<Effect, { effect: "grant-look-permission" }>, path: string, context: Context): boolean {
  const { permission, duration } = effect;
  if (context.barrier || context.decider || context.phase !== "resolution" || effect.optional || effect.decidedBy ||
      effect.forEach || effect.forEachVote || permission.who.player !== "you" || permission.timing !== "any-time" ||
      context.uncertainExileOrigin || context.exileResults?.length !== 1 || !context.exileResults[0].faceDown ||
      !isDeepStrictEqual(permission.what, [{ ref: "that", filter: { cls: "card" } }])) return false;
  const at = child(path, "permission");
  this.walk(permission, at, context);
  this.walk(duration, child(path, "duration"), context);
  const card = this.at(`${at}/what/0`);
  if (!card) return true;
  if (card.id !== context.exileResults[0].result.id || card.cardinality !== "one") {
    this.diagnostic("unsupported-scope", path, "Private access requires the exact face-down exile successor.", ["400.7j", "406.3"]);
    return true;
  }
  const rules = ["400.7j", "406.3", "406.3a", "611.2a"];
  const event = this.event(path, context, effect.effect, [card], context.conditional ? { conditional: true } : {});
  this.node({ id: this.id(path, "face-down-exile-look"), path, node: "face-down-exile-look", scope: context.ability,
    viewer: context.you, card: card.id, duration, event, continuedUntil: "leaves-exile-or-shuffled", rules });
  this.engineFeature("private-exile-look", path, "Only the bound player may inspect the card. Retain access while it stays in exile unshuffled.", rules);
  return true;
}
