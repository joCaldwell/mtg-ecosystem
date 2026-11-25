// Instruction-level binding: effects, sentences and their structured forms.
import { isDeepStrictEqual } from "node:util";
import type { Effect, ObjectFilter, Sentence } from "../../ast.ts";
import { stepEventOperation } from "../step-vocabulary.ts";
import type { BindingCardinality, BindingNode } from "../types.ts";
import type { Analyzer } from "./analyzer.ts";
import { announcedDivision, choiceInSentences, resolutionCount } from "./choices.ts";
import { ENTRY_CHOICE_RULES, cardCount, child, combine, objectCardinality, unambiguousPluralRecipient, type Candidate, type Context } from "./context.ts";
import { coloredSearchSlots, containsEffect, echoes, isRecord, stackClasses, type StackInstruction } from "./predicates.ts";

/** CR 700.2a-c: modes are chosen as the spell or ability is put on the stack,
 * and a mode's targets exist only if it was chosen. Each mode binds in its own
 * branch; a later instruction cannot rely on a result of an unchosen mode. */
export function modal(this: Analyzer, effect: Extract<Effect, { effect: "modal" }>, path: string, context: Context, weights?: number[]): void {
  const count = effect.count;
  const announcement = { ...context, announcementOperand: true };
  if ("variable" in count) this.walk({ amount: count.variable === "x" ? "x" : "that-much" }, child(path, "count"), announcement);
  this.walk(effect.conditionalCount, child(path, "conditionalCount"), announcement);
  this.walk(effect.chooser, child(path, "chooser"), announcement);
  let sourceMoved = context.sourceMoved;
  const branches: Context[] = [];
  effect.options.forEach((option, index) => {
    const at = child(child(path, "options"), index);
    const branch = this.branch(context);
    if (weights) {
      branch.resolutionChoice = choiceInSentences(option.effects, child(at, "effects"));
      branch.weightedMode = this.id(path, "weighted-modal");
    }
    this.walk(option.additionalCost, child(at, "additionalCost"), { ...branch, phase: "payment" });
    this.sentences(option.effects, child(at, "effects"), branch);
    sourceMoved ||= branch.sourceMoved;
    branches.push(branch);
  });
  if (weights && "weightBudget" in count) {
    const rules = ["601.2b", "700.2a", "700.2c", "700.2d", "700.2i"];
    this.node({ id: this.id(path, "weighted-modal"), path, node: "weighted-modal",
      scope: context.ability, chooser: context.you, maximum: count.weightBudget, weights,
      repeat: Boolean(effect.allowRepeat), timing: "stack-announcement", order: "printed-mode-order", rules });
    this.engineFeature("weighted-modal-selection", path,
      "Choose mode occurrences within the pawprint budget and resolve each occurrence with independent choices and results.", rules);
  }
  context.sourceMoved = sourceMoved;
  this.blockBranchCaptures(context, branches);
  context.objects = []; context.players = []; context.quantities = [];
}
/** "Flip a coin. If you win the flip, [A]. If you lose the flip, [B]" (CR
 * 705): one flip, and the outcome's instructions as exclusive branches.
 * What a branch produces exists only if that outcome happened; a target
 * named in one outcome is still the ability's target in the next ("If you
 * lose the flip, it gets -0/-2"). */
export function flipCoin(this: Analyzer, effect: Extract<Effect, { effect: "flip-coin" }>, path: string, context: Context): void {
  this.walk(effect.who, child(path, "who"), context);
  this.walk(effect.count, child(path, "count"), context);
  const flipper = this.at(child(path, "who"));
  this.event(path, context, "flip-coin", flipper ? [flipper] : [], context.conditional ? { conditional: true } : {});
  let announced: Candidate[] = [];
  let sourceMoved = context.sourceMoved;
  const branches: Context[] = [];
  (effect.outcomes ?? []).forEach((outcome, index) => {
    const branch: Context = { ...this.branch(context), conditional: true };
    branch.objects = combine(branch.objects, announced);
    this.sentences(outcome.effects, child(child(child(path, "outcomes"), index), "effects"), branch);
    announced = combine(announced, branch.objects.filter(candidate => this.announced(candidate)));
    sourceMoved ||= branch.sourceMoved;
    branches.push(branch);
  });
  this.blockBranchCaptures(context, branches);
  context.sourceMoved = sourceMoved;
  context.objects = combine(context.objects, announced);
}
/** "Roll a d20" with a results table (CR 706.3): one roll, then the row its
 * result falls in, as exclusive branches like a coin flip's outcomes.
 * Targets in any row are the ability's own (CR 706.3b). */
export function dieTable(this: Analyzer, effect: Extract<Effect, { effect: "roll-dice" }>, path: string, context: Context): void {
  for (const key of ["who", "die", "count", "adjustments"] as const) this.walk(effect[key], child(path, key), context);
  const roller = this.at(child(path, "who"));
  this.event(path, context, "roll-dice", roller ? [roller] : [], context.conditional ? { conditional: true } : {});
  let announced: Candidate[] = [];
  let sourceMoved = context.sourceMoved;
  const branches: Context[] = [];
  effect.table!.rows.forEach((row, index) => {
    const at = child(child(child(path, "table"), "rows"), index);
    const branch: Context = { ...this.branch(context), conditional: true };
    branch.objects = combine(branch.objects, announced);
    this.walk(row.range, child(at, "range"), branch);
    this.sentences(row.effects, child(at, "effects"), branch);
    announced = combine(announced, branch.objects.filter(candidate => this.announced(candidate)));
    sourceMoved ||= branch.sourceMoved;
    branches.push(branch);
  });
  this.blockBranchCaptures(context, branches);
  context.sourceMoved = sourceMoved;
  context.objects = combine(context.objects, announced);
}
/** CR 608.2d: exactly one listed instruction is chosen while the effect is
 * applied. Options bind like modes; shared references are the same nodes. */
export function choice(this: Analyzer, effect: Extract<Effect, { effect: "choice" }>, path: string, context: Context): void {
  this.walk(effect.who, child(path, "who"), context);
  const chooser = this.at(child(path, "who"));
  const decider = chooser?.id;
  // The choice is the step's own event; an optional choice records its
  // decision event before it branches (CR 608.2d).
  if (!effect.optional) this.event(path, context, stepEventOperation(effect.effect), chooser ? [chooser] : [], {
    ...(context.decider ? { optional: true as const, decider: context.decider } : {}),
    ...(context.conditional ? { conditional: true as const } : {}) });
  let sourceMoved = context.sourceMoved;
  const branches: Context[] = [];
  effect.options.forEach((option, index) => {
    const branch = { ...this.branch(context), ...(decider ? { decider } : {}) };
    this.effect(option, child(child(path, "options"), index), branch);
    sourceMoved ||= branch.sourceMoved;
    branches.push(branch);
  });
  this.blockBranchCaptures(context, branches);
  context.sourceMoved = sourceMoved;
  context.objects = []; context.players = []; context.quantities = [];
}
/** Simultaneous instructions are one event: none sees another's results,
 * and all of their results are available afterwards. */
export function simultaneous(this: Analyzer, effect: Extract<Effect, { effect: "simultaneous" }>, path: string, context: Context): void {
  const start = this.branch(context);
  const after = this.branch(context);
  const branches: Context[] = [];
  effect.steps.forEach((step, index) => {
    const branch = this.branch(start);
    this.effect(step, child(child(path, "steps"), index), branch);
    branches.push(branch);
    after.objects = combine(after.objects, branch.objects);
    after.players = combine(after.players, branch.players);
    after.quantities = combine(after.quantities, branch.quantities);
    after.sourceMoved ||= branch.sourceMoved;
    after.departed = combine(after.departed ?? [], branch.departed ?? []);
  });
  this.blockBranchCaptures(context, branches);
  Object.assign(context, { objects: after.objects, players: after.players, quantities: after.quantities, sourceMoved: after.sourceMoved, departed: after.departed });
}
/** "[Do A.] If [condition], [do B] instead" (CR 614.15): one of the two
 * happens. Each binds in its own branch from what came before; the
 * condition may refer to the original instruction's objects. Results of
 * either branch are not antecedents afterwards. */
export function selfReplacement(this: Analyzer, effect: Extract<Effect, { effect: "self-replacement" }>, path: string, context: Context): void {
  const run = (instructions: Effect | Effect[] | { sequence: Sentence[] }, at: string): Context => {
    const branch = this.branch(context);
    if (Array.isArray(instructions)) instructions.forEach((instruction, index) => this.effect(instruction, child(at, index), branch));
    else if ("effect" in instructions) this.effect(instructions, at, branch);
    else this.sentences(instructions.sequence, child(at, "sequence"), branch);
    return branch;
  };
  const original = run(effect.original, child(path, "original"));
  // The condition decides whether the original instruction is replaced, so
  // it is checked before that instruction happens (CR 614.15): it sees the
  // original's participants, not what the instruction did to them.
  const participants = [...original.objects, ...(original.departed ?? [])]
    .filter(candidate => this.result.nodes.find(node => node.id === candidate.id)?.node !== "event-result");
  this.walk(effect.condition, child(path, "condition"), { ...this.branch(context), objects: combine(context.objects, participants), players: original.players });
  // The replacement's pronoun in a role the original instruction fills means
  // that participant: "Target creature gets +2/+2. If it's a Human, instead
  // it gets +3/+3"; "~ deals 2 damage to any target. If ..., it deals 3
  // damage instead."
  if ("effect" in effect.original && Array.isArray(effect.instead)) {
    const pronoun = (value: unknown) => isRecord(value) && (value.ref === "it" || value.ref === "them" || value.ref === "that");
    for (const [index, instruction] of effect.instead.entries()) {
      const at = child(child(path, "instead"), index);
      const instead = instruction as unknown as Record<string, unknown>, replaced = effect.original as unknown as Record<string, unknown>;
      for (const key of ["what", "source", "on", "from"]) {
        if (!pronoun(instead[key]) || !isRecord(replaced[key])) continue;
        const participant = this.at(child(child(path, "original"), key));
        if (participant) this.roleAntecedents.set(child(at, key), participant);
      }
      if (isRecord(instead.to) && pronoun(instead.to.ref) && isRecord(replaced.to) && isRecord(replaced.to.ref)) {
        const participant = this.at(child(child(child(path, "original"), "to"), "ref"));
        if (participant) this.roleAntecedents.set(child(child(at, "to"), "ref"), participant);
      }
    }
  }
  const instead = run(effect.instead, child(path, "instead"));
  this.blockBranchCaptures(context, [original, instead]);
  context.sourceMoved = context.sourceMoved || original.sourceMoved || instead.sourceMoved;
  context.mentionedObject = context.mentionedObject || original.mentionedObject || instead.mentionedObject;
  context.objects = []; context.players = []; context.quantities = [];
}
/** "[Player] pays [cost]" as an instruction (CR 118.12): the payer is that
 * player. Paying it is optional when printed with "may". */
export function payCost(this: Analyzer, effect: Extract<Effect, { effect: "pay-cost" }>, path: string, context: Context): void {
  if (effect.optional) this.rememberDecision(path, context);
  this.walk(effect.who, child(path, "who"), context);
  const payer = this.at(child(path, "who"));
  const decider = effect.optional ? (effect.decidedBy ? (this.walk(effect.decidedBy, child(path, "decidedBy"), context), this.at(child(path, "decidedBy"))?.id) : payer?.id) : undefined;
  if (!payer || payer.domain !== "player" || payer.cardinality !== "one") {
    const barrier = "The payment needs one bound paying player.";
    this.diagnostic("unsupported-scope", path, barrier, ["118.12"]);
    this.walk(effect.costs, child(path, "costs"), { ...context, barrier });
    return;
  }
  const boundPayment = effect.optional && !effect.decidedBy && !context.decider && !context.x && !context.announcedX &&
    context.lifeGain?.player === payer.id && !context.conditional &&
    context.lexicalAbilityPath !== undefined && path === `${context.lexicalAbilityPath}/effects/0/steps/0` &&
    isDeepStrictEqual(effect.costs, [{ cost: "mana", symbols: ["X"] }]) &&
    isDeepStrictEqual(effect.variableConstraints, [{ variable: "x", maximum: {
      amount: "event-quantity", event: "life-change", change: "gained", who: { player: "you" }, scope: "contextual",
    } }]);
  const payment: Context = { ...context, you: payer.id, phase: "payment" };
  if (boundPayment && context.lifeGain) {
    const at = child(path, "variableConstraints");
    const rules = ["107.3f", "118.12", "119.9", "603.2c", "608.2d", "608.2g"];
    const id = this.node({ id: this.id(at, "life-bounded-x"), path: at, node: "bounded-payment-amount",
      scope: context.ability, payment: this.id(path, "event"), payer: payer.id, minimum: 0, maximum: context.lifeGain.quantity, rules });
    this.bound({ player: "you" }, `${at}/0/maximum/who`, payer.id, rules);
    context.resolutionPaymentX = { path, id };
    payment.x = id;
    this.engineFeature("bounded-payment-amount", at,
      "Choose X at the resolution payment, bounded by the actual life gain that triggered this ability.", rules);
  } else if (effect.variableConstraints?.length) {
    this.walk(effect.variableConstraints, child(path, "variableConstraints"), payment);
  }
  this.walk(effect.costs, child(path, "costs"), payment);
  const event = this.event(path, context, "pay-cost", [payer], {
    ...(effect.optional ? { optional: true as const, ...(decider ? { decider } : {}) } : {}), ...(context.conditional ? { conditional: true as const } : {}) });
  if (effect.optional) context.decisions?.push(path);
  // "You may pay life equal to its power. If you do, put that many +1/+1 counters on it."
  if (effect.costs.length === 1 && (effect.costs[0].cost === "pay-life" || effect.costs[0].cost === "remove-counters"))
    context.quantities = combine(context.quantities, [this.eventResult(path, event, "quantity", "one", "measured-quantity")]);
}
/** "You may [A] and [B]": one decision covers the instructions, which are
 * then performed in order (CR 608.2c-d). Their results exist only if the
 * player chose to perform them. */
export function optionalGroup(this: Analyzer, effect: Extract<Effect, { effect: "optional-group" }>, path: string, context: Context): void {
  this.rememberDecision(path, context);
  this.walk(effect.who, child(path, "who"), context);
  const decider = this.at(child(path, "who"))?.id;
  const group: Context = { ...context, decider: decider ?? "unresolved" };
  effect.steps.forEach((step, index) => this.effect(step, child(child(path, "steps"), index), group));
  this.blockBranchCaptures(context, [group]);
  Object.assign(context, { objects: group.objects, players: group.players, quantities: group.quantities, sourceMoved: group.sourceMoved, sourceSuccessor: group.sourceSuccessor, x: group.x });
  if (decider === context.you) context.decisions?.push(path);
}
export function definitions(this: Analyzer, bindings: Sentence["bindings"], path: string, context: Context): void {
  if (!bindings) return;
  for (const [index, binding] of bindings.entries()) {
    const at = child(path, index);
    if (context.x) {
      this.diagnostic("duplicate-number-definition", at, "X already has a definition in this scope; shadowing has not been established.", ["107.3i", "107.3k"]);
      continue;
    }
    context.x = this.node({ id: this.id(at, "number"), path: at, node: "defined-number", scope: context.ability,
      definition: binding.value, rules: ["107.3c"] });
  }
}
export function sentences(this: Analyzer, sentences: Sentence[], path: string, context: Context): void {
  if (!context.barrier && !context.previousEffect && !context.decider && !context.conditional && context.phase === "resolution" &&
      (this.copyThenReturnOriginal(sentences, path, context) || this.copyAndReselect(sentences, path, context) ||
        this.copiesForOpponents(sentences, path, context) ||
        this.grantOnStackCopies(sentences, path, context) || this.conditionalExilePermission(sentences, path, context) ||
        this.pairedHandExiles(sentences, path, context) || this.matchingSacrifice(sentences, path, context) ||
        this.fightCreatedTokens(sentences, path, context) || this.searchesForExiledCreatures(sentences, path, context) ||
        this.searchesForOpponentOffers(sentences, path, context) || this.scansWithOneExcludedCard(sentences, path, context))) return;
  let previousDecisions: string[] = [];
  let precedingAction: { event: string; action: "sacrifice" | "discard" } | undefined;
  let precedingSentence: Context["precedingSentence"];
  for (const [index, sentence] of sentences.entries()) {
    const at = child(path, index);
    context.precedingSentence = precedingSentence;
    const sourceOnly = context.permanentSource && !context.mentionedObject && !context.sourceMoved && this.onlySource(context);
    const decisions: string[] = [];
    context.decisions = decisions;
    context.previousDecisions = previousDecisions;
    this.definitions(sentence.bindings, child(at, "bindings"), context);
    if (this.opponentDiscardOrLife(sentence, at, context) || this.replacedSearchPartition(sentence, at, context)) {
      context.previousEffect = undefined;
      previousDecisions = [];
      precedingAction = undefined;
      precedingSentence = undefined;
      continue;
    }
    // "If you do" / "If you don't" refers to the one optional instruction of
    // the preceding sentence (CR 608.2c, 608.2d).
    const condition = sentence.condition;
    const decisionPath = previousDecisions.length === 1 ? previousDecisions[0] : undefined;
    const decisionEvent = decisionPath ? this.result.nodes.find(node => node.id === this.id(decisionPath, "event")) : undefined;
    const previousPayment = condition?.condition === "preceding-action" && condition.outcome === "not-performed" &&
      condition.who.player === "they" ? decisionEvent : undefined;
    const paymentCondition = previousPayment?.node === "event" && previousPayment.operation === "pay-cost" &&
      previousPayment.optional && previousPayment.scope === context.ability && previousPayment.decider &&
      this.player({ player: "they" }, `${at}/condition/who`, context)?.id === previousPayment.decider;
    const yourDecision = decisionEvent?.node !== "event" || decisionEvent.decider === context.you;
    const linked = condition && (paymentCondition ||
      (yourDecision && (condition.condition === "you-do" || condition.condition === "you-dont"))) ? decisionPath : undefined;
    const declines = condition?.condition === "you-dont" || paymentCondition;
    const paid = !linked && condition?.condition === "you-do" ? precedingAction : undefined;
    if (paid) {
      const atCondition = child(at, "condition");
      const rules = ["118.11", "118.12", "608.2c", paid.action === "sacrifice" ? "701.21a" : "701.9a"];
      this.node({ id: this.id(atCondition, "action-cost-condition"), path: atCondition, node: "action-cost-condition",
        scope: context.ability, ...paid, rules });
      this.engineFeature("action-cost-condition", atCondition, "Check whether the preceding action was carried out as the cost of this instruction.", rules);
    }
    if (linked && condition) this.node({ id: this.id(child(at, "condition"), "decision-link"), path: child(at, "condition"), node: "decision-link",
      scope: context.ability, condition: declines ? "you-dont" : "you-do", decision: linked, rules: ["608.2c", "608.2d"] });
    // An "unless" clause follows its instruction and may refer to that
    // instruction's objects ("Counter target spell unless its controller
    // pays {3}"), so it binds after the steps.
    const trailing = sentence.conditionKind === "unless" || sentence.conditionPosition === "after";
    context.linkedPath = linked;
    const suspended = context.explicitExiledCast && context.explicitExiledCast.path === context.previousEffect &&
      isDeepStrictEqual(condition, { condition: "not", of: { condition: "has-keyword", what: { ref: "it" }, keyword: "suspend" } }) &&
      !sentence.otherwise && !sentence.afterConditional && sentence.steps.length === 1 &&
      sentence.steps[0].effect === "gain-abilities" && isDeepStrictEqual(sentence.steps[0].what, { ref: "it" }) &&
      isDeepStrictEqual(sentence.steps[0].abilities, [{ gained: "keyword", keyword: { keyword: "suspend" } }])
      ? context.explicitExiledCast : undefined;
    if (!trailing && !paymentCondition) this.walk(condition, child(at, "condition"), suspended
      ? { ...context, boundOperand: { path: `${at}/condition/of/what`, candidate: suspended.result, rules: ["400.7j", "608.2c", "702.62"] } }
      : linked || paid ? { ...context, linkedDecision: true } : context);
    if (sentence.afterConditional &&
        !this.independentRevealRemainder(sentence, sentences[index - 1], child(path, index - 1), context))
      this.diagnostic("unsupported-scope", at, "This sentence refers back to a conditional sentence; whether that condition governs it is not decided.", ["608.2c"]);
    if (condition || sentence.otherwise || sentence.afterConditional) {
      context.previousEffect = undefined;
    }
    if (condition || sentence.otherwise) {
      // The steps bind against what came before; what they establish exists
      // only if the condition held, so it is not an antecedent afterwards.
      // "If you discard a land card this way, put it ...": the objects that
      // instruction acted on.
      const link = this.result.nodes.find(node => node.node === "event-link" && node.path.startsWith(`${child(at, "condition")}/`));
      const linkedResult = link?.node === "event-link" ? this.result.nodes.find(node => node.node === "event-result" && node.event === link.event && node.result === "objects") : undefined;
      const conditionSubject = !trailing && condition && "what" in condition && isRecord(condition.what)
        && ["it", "that", "this", "self", "target", "equipped", "enchanted"].includes(String(condition.what.ref))
        ? this.at(child(child(at, "condition"), "what"))
        : linkedResult?.node === "event-result" ? { id: linkedResult.id, cardinality: linkedResult.cardinality, domain: "object" as const, description: linkedResult.description } : undefined;
      const branches = [sentence.steps, sentence.otherwise?.steps ?? []].map((steps, branchIndex) => {
        const declinedBranch = declines ? branchIndex === 0 : branchIndex === 1;
        const branch: Context = { ...this.branch(context), conditional: true, decisions,
          performedRepeat: linked && !declinedBranch && branchIndex === 0 && !sentence.otherwise && !sentence.afterConditional &&
            sentence.conditionKind === "if" && steps.length === 2 && steps[0].effect === "draw" && steps[1].effect === "repeat-process"
            ? { decision: linked, path: `${at}/steps/1` } : undefined,
          ...(suspended && branchIndex === 0 ? { previousEffect: suspended.path, explicitExiledCast: suspended } : {}),
          players: combine(context.players, this.targetPlayersUnder(child(at, "condition"), context)),
          ...(linked && !declinedBranch && context.resolutionPaymentX?.path === linked && !context.x
            ? { x: context.resolutionPaymentX.id } : {}),
          // "If you don't, ...": what the declined instruction would have produced does not exist.
          ...(linked && declinedBranch ? this.beforeDecision.get(linked) : {}),
          ...(conditionSubject?.domain === "object" ? { conditionSubject: { candidate: conditionSubject,
            before: [...context.objects, ...(context.departed ?? [])].map(candidate => candidate.id) } } : {}) };
        const stepsPath = branchIndex === 0 ? child(at, "steps") : child(child(at, "otherwise"), "steps");
        const unattach = precedingSentence?.steps.length === 1 ? precedingSentence.steps[0] : undefined;
        const orderedTap = branchIndex === 0 && linked && !declines && !sentence.otherwise && !sentence.afterConditional &&
          decisionEvent?.node === "event" && decisionEvent.operation === "unattach" && decisionEvent.optional &&
          unattach?.effect === "unattach" && unattach.from && "ref" in unattach.from && unattach.from.ref === "filter" &&
          isDeepStrictEqual(unattach.from.filter, { types: ["creature"], control: "you" }) &&
          steps.length === 2 && steps[0].effect === "tap" && Object.keys(steps[0]).length === 2 &&
          isDeepStrictEqual(steps[0].what, { ref: "that", filter: { types: ["creature"] } }) &&
          steps[1].effect === "gain-abilities" && isDeepStrictEqual(steps[1].what, { ref: "it" });
        steps.forEach((effect, stepIndex) => {
          const atStep = child(stepsPath, stepIndex);
          const tapped = orderedTap && stepIndex === 1 ? this.at(`${stepsPath}/0/what`) : undefined;
          const receiver = tapped && linked && this.at(`${linked}/from`)?.id === tapped.id
            ? { path: child(atStep, "what"), candidate: tapped, rules: ["118.12", "608.2c", "609.3", "701.3d"] } : undefined;
          this.effect(effect, atStep, receiver ? { ...branch, boundOperand: receiver } : branch);
        });
        return branch;
      });
      // A trailing "if" or "unless" is checked as its instruction would be
      // performed, before that instruction acts (CR 608.2c): it sees the
      // instruction's participants, not what the instruction produced.
      if (trailing) {
        const steps = branches[0];
        const participants = [...steps.objects, ...(steps.departed ?? [])]
          .filter(candidate => this.result.nodes.find(node => node.id === candidate.id)?.node !== "event-result");
        const checked: Context = { ...this.branch(context), objects: combine(context.objects, participants), players: steps.players,
          departed: steps.departed, conditional: true, decisions };
        this.walk(condition, child(at, "condition"), checked);
        steps.players = combine(steps.players, checked.players);
      }
      // Targets were announced with the spell or ability (CR 601.2c), and an
      // "unless" clause's payer and participants exist whether or not it is
      // paid; only what the instructions produce depends on the condition.
      const lasting = (candidate: Candidate) => {
        const node = this.result.nodes.find(entry => entry.id === candidate.id);
        return node?.node === "selection" ? trailing || node.timing === "stack-announcement"
          : node?.node === "derived-player" ? trailing : false;
      };
      for (const branch of branches) {
        context.objects = combine(context.objects, branch.objects.filter(lasting));
        context.players = combine(context.players, branch.players.filter(lasting));
      }
      this.blockBranchCaptures(context, branches);
      context.sourceMoved = context.sourceMoved || branches.some(branch => branch.sourceMoved);
      context.mentionedObject = context.mentionedObject || branches.some(branch => branch.mentionedObject);
    } else {
      for (const [stepIndex, effect] of sentence.steps.entries())
        this.effect(effect, child(child(at, "steps"), stepIndex), context);
    }
    previousDecisions = decisions;
    precedingSentence = !condition && !sentence.otherwise && !sentence.afterConditional ? { path: at, steps: sentence.steps } : undefined;
    const [step] = sentence.steps;
    const actionPath = `${at}/steps/0`;
    precedingAction = !condition && !sentence.otherwise && !sentence.afterConditional &&
      sentence.steps.length === 1 && (step.effect === "sacrifice" || step.effect === "discard") && !step.optional && !step.forEach && !step.forEachVote &&
      step.who.player === "you" && (step.effect === "discard" || !step.chosenBy) &&
      step.what !== "hand" && step.what.ref === "filter" && objectCardinality(step.what) === "one" && this.at(child(actionPath, "what"))
      ? { event: this.id(actionPath, "event"), action: step.effect } : undefined;
    context.linkedPath = undefined;
    // A trailing definition may refer to a target established earlier in
    // the same sentence. Traverse it after the subjects have been bound.
    sentence.bindings?.forEach((binding, bindingIndex) => {
      const value = child(child(child(at, "bindings"), bindingIndex), "value");
      if (sourceOnly && sentence.steps.length === 1 && step.effect === "exile" && step.what.ref === "library-cards" &&
          step.what.count.amount === "x" && binding.value.amount === "distinct-count" &&
          binding.value.characteristic === "card-type" && binding.value.among.ref === "each" &&
          isDeepStrictEqual(binding.value.among.filter, { other: true, nonTypes: ["land"], cls: "permanent", control: "you" })) {
        this.walk(binding.value, value, { ...context, sourceExclusionPath: `${value}/among/filter` });
        this.engineFeature("characteristic-predicate", value, "Count each card type represented among the qualifying permanents at resolution.", ["205.2", "608.2h"]);
      } else this.walk(binding.value, value, context);
    });
    for (const reference of this.result.references) {
      if (reference.binding === "bound" && reference.target === context.x &&
          reference.path.startsWith(`${child(at, "bindings")}/`)) {
        this.diagnostic("unsupported-number-binding", reference.path, "A number definition cannot establish itself recursively.", ["107.3c"]);
        this.invalidNodes.set(reference.target, "unresolved-dependency");
      }
    }
  }
}
export function quantifiedInstruction(this: Analyzer, effect: Extract<Effect, { effect: "for-each" }>, path: string, context: Context,
  precedingEffect?: string): boolean {
  if (context.barrier || context.decider || context.phase !== "resolution" || effect.optional || effect.forEach || effect.forEachVote ||
      effect.effects.length === 0 || Object.keys(effect.effects[0]).length !== 1) return false;
  const [first, second] = effect.effects[0].steps;
  const { each } = effect;
  const target = first?.effect === "exile-until" && first.what.length === 1 ? first.what[0]
    : first?.effect === "tap" || first?.effect === "destroy" ? first.what : undefined;
  const targetByPlayer = effect.effects.length === 1 && "player" in each && (each.player === "each-player" || each.player === "each-opponent") &&
    effect.effects[0].steps.length === 1 && target?.ref === "target" && target.upTo && objectCardinality(target) === "one" &&
    target.filter.control === "that-player" && !first?.optional && !first?.decidedBy && !first?.forEach && !first?.forEachVote;
  const tokenCopy = effect.effects.length === 1 && "ref" in each && each.ref === "each" && each.filter.cls === "token" &&
    effect.effects[0].steps.length === 1 && first?.effect === "create-token" &&
    "copyOf" in first.token && first.token.copyOf.ref === "that";
  const exileCopy = effect.effects.length === 1 && "ref" in each && each.ref === "each" && each.filter.cls === "card" &&
    each.filter.relation?.relation === "event-result" && each.filter.relation.event === "exile" &&
    each.filter.relation.scope === "this-way" && !context.exileResults?.[0]?.faceDown &&
    Object.keys(each.filter).every(key => ["cls", "relation", "types", "nonTypes", "supertypes"].includes(key)) &&
    effect.effects[0].steps.length === 2 &&
    first?.effect === "copy-card" && second?.effect === "play-card" && second.instruction.what.ref === "copy-result" &&
    context.exileResults?.length === 1 && !context.uncertainExileOrigin;
  const onExiledCards = effect.effects.length === 2 && !context.exileResults?.[0]?.faceDown && "ref" in each && each.ref === "each" &&
    isDeepStrictEqual(each.filter, { nonTypes: ["land"], cls: "card", relation: { relation: "event-result", event: "exile", scope: "this-way" } }) &&
    effect.effects[0].steps.length === 1 && first?.effect === "put-counters" &&
    isDeepStrictEqual(first.on, { ref: "that", filter: { cls: "card" } }) &&
    isDeepStrictEqual(first.count, { amount: "attribute", of: { ref: "it" }, attribute: "mana-value" }) &&
    !first.optional && !first.forEach && !first.forEachVote && !first.decidedBy &&
    effect.effects[1].steps.length === 1 && effect.effects[1].steps[0].effect === "gain-abilities" &&
    isDeepStrictEqual(effect.effects[1].condition,
      { condition: "not", of: { condition: "has-keyword", what: { ref: "it" }, keyword: "suspend" } }) &&
    effect.effects[1].conditionKind === "if" && !effect.effects[1].otherwise && !effect.effects[1].afterConditional &&
    context.exileResults?.length === 1 && !context.uncertainExileOrigin;
  const destroyed = precedingEffect ? this.result.nodes.find(node => node.id === this.id(precedingEffect, "event")) : undefined;
  const destroyLoop = effect.effects.length === 1 && destroyed?.node === "event" && destroyed.operation === "destroy" &&
    destroyed.scope === context.ability && !destroyed.conditional &&
    isDeepStrictEqual(each, { ref: "each", filter: { cls: "permanent",
      relation: { relation: "zone-change", to: { zone: "graveyard", owner: "any" }, scope: "this-way" } } }) &&
    effect.effects[0].steps.length === 1 && first?.effect === "create-token" &&
    isDeepStrictEqual(first.who, { player: "controller", of: { ref: "it" } }) &&
    first.count.amount === "fixed" && !first.optional && !first.forEach && !first.forEachVote && !first.decidedBy;
  const linkedOwners = effect.effects.length === 1 && context.linkedExile && !context.sourceMoved &&
    "ref" in each && each.ref === "each" && each.filter.relation?.relation === "linked-exile" &&
    (each.filter.relation.source.ref === "this" || each.filter.relation.source.ref === "self") &&
    isDeepStrictEqual(Object.keys(each.filter).sort(), ["cls", "relation"]) && each.filter.cls === "card" &&
    effect.effects[0].steps.length === 1 && isDeepStrictEqual(first, {
      effect: "move-zone", who: { player: "owner", of: { ref: "it" } }, what: { ref: "it" },
      to: { zone: "library", owner: "their", position: "bottom" },
    });
  const counterChoices = effect.effects.length === 1 && "ref" in each && each.ref === "each" &&
    each.filter.cls === "permanent" && !each.filter.zone &&
    Object.keys(each.filter).every(key => ["cls", "nonSubtypes"].includes(key)) &&
    effect.effects[0].steps.length === 1 && isDeepStrictEqual(first, {
      effect: "put-counters-of", on: { ref: "it" },
      counters: { of: { ref: "it" }, select: "one-existing-counter-kind", placement: "optional",
        phaseOrder: "choose-all-then-place" },
    });
  if (!targetByPlayer && !tokenCopy && !exileCopy && !onExiledCards && !destroyLoop && !linkedOwners && !counterChoices) return false;
  const where = child(path, "each");
  let domain: Candidate | undefined;
  const exile = exileCopy || onExiledCards ? context.exileResults?.[0] : undefined;
  if (destroyLoop && destroyed) {
    const rules = ["400.7", "608.2c", "608.2h", "701.8a"];
    const id = this.node({ id: this.id(where, "destroyed-permanents"), path: where, node: "destroyed-permanents",
      scope: context.ability, event: destroyed.id, destination: "graveyard", information: "before-move", cardinality: "many", rules });
    this.bound(each, where, id, rules);
    domain = { id, domain: "object", cardinality: "many", description: { cls: "permanent" } };
  } else if (exile && "ref" in each && each.ref === "each" && (onExiledCards || Object.keys(each.filter).length > 2)) {
    const rules = ["400.7j", "608.2c", "608.2f"];
    const { relation, ...filter } = each.filter;
    const id = this.node({ id: this.id(where, "eligible-exile-members"), path: where, node: "filtered-result-group",
      scope: context.ability, pool: exile.result.id, filter, cardinality: "many", rules });
    this.bound(each, where, id, rules);
    domain = { id, domain: "object", cardinality: "many", description: filter };
  } else if (exile) {
    this.bound(each, where, exile.result.id, ["400.7j", "608.2c"]);
    domain = exile.result;
  } else {
    this.walk(each, where, context);
    domain = this.at(where);
  }
  if (!domain || domain.domain === "object-or-player" || domain.domain === "number") return false;
  const rules = targetByPlayer ? ["115.3", "601.2c", "603.3d", "608.2f"]
    : linkedOwners ? ["101.4", "401.4", "607.2a", "608.2e", "608.2f", "610.3"]
    : counterChoices ? ["122.1", "608.2c", "608.2d", "608.2f"] : ["608.2c", "608.2f", "611.2c"];
  const memberId = this.node({ id: this.id(where, "instruction-member"), path: where, node: "instruction-member",
    scope: context.ability, group: domain.id, domain: domain.domain, rules });
  const member: Candidate = { id: memberId, cardinality: "one", domain: domain.domain, description: domain.description };
  this.node({ id: this.id(path, "group-instruction"), path, node: "group-instruction", scope: context.ability,
    execution: { kind: "for-each", domain: domain.id, member: member.id, membership: targetByPlayer ? "announcement" : "resolution",
      processing: effect.processing, ...(linkedOwners ? { ordering: "each-owner-private-simultaneous" as const } : {}) }, rules });
  const branch = this.branch(context);
  if (targetByPlayer) {
    branch.actor = member;
    branch.announcementMember = member.id;
  } else {
    branch.objects = [member];
    branch.departed = [];
    branch.mentionedObject = true;
    if (exile && exileCopy) branch.exileResults = [{ event: exile.event, result: member }];
  }
  this.sentences(effect.effects, child(path, "effects"), branch);
  if (counterChoices) {
    const at = `${path}/effects/0/steps/0`;
    this.node({ id: this.id(at, "existing-counter-kind"), path: at, node: "existing-counter-kind", scope: context.ability,
      chooser: context.you, member: member.id, event: this.id(at, "event"), count: 1,
      placement: "optional-for-each-member", ordering: "choose-all-then-place", rules });
    this.engineFeature("existing-counter-kind", at,
      "Choose an existing counter kind on each member with a counter, then independently decide whether to add one of that kind to each of those members.", rules);
  }
  this.blockBranchCaptures(context, [branch]);
  // No per-member selection or effect result is exported as a single
  // antecedent after the loop.
  context.objects = [];
  if (targetByPlayer && first?.effect === "tap" && target) {
    const selection = this.at(`${path}/effects/0/steps/0/what`);
    const groupRules = ["601.2c", "603.3d", "608.2b", "608.2f"];
    if (selection) {
      const pool = this.node({ id: this.id(path, "eligible-target-group"), path, node: "eligible-target-group",
        scope: context.ability, template: selection.id, member: member.id, cardinality: "many", rules: groupRules });
      context.objects = [{ id: pool, domain: "object", cardinality: "many", description: target.filter }];
    }
  }
  context.quantities = [];
  context.mentionedObject = true;
  this.engineFeature("group-instruction", path,
    "Remember which members were eligible at the stated time and process each printed instruction across those members.", rules);
  return true;
}

export function libraryScan(this: Analyzer, effect: Extract<Effect, { effect: "scan-library" }>, path: string, context: Context): boolean {
  const { library, until, operation } = effect;
  if (context.barrier || context.decider || context.phase !== "resolution" || effect.optional || effect.forEach || effect.forEachVote ||
      library.zone !== "library" || (library.owner !== "your" && library.owner !== "their") ||
      until.count.amount !== "fixed" || !Number.isSafeInteger(until.count.value) || until.count.value < 1) return false;
  this.walk(effect.who, child(path, "who"), context);
  const actor = this.at(child(path, "who"));
  if (actor?.domain !== "player" || actor.cardinality !== "one") return false;
  this.walk(library, child(path, "library"), { ...context, actor });
  const owner = this.at(`${path}/library/owner`);
  if (!owner || owner.id !== actor.id) {
    this.diagnostic("unsupported-scope", path, "A scan must bind the library of the player performing it.", ["608.2c"]);
    return true;
  }
  const rules = ["608.2c", "609.3", operation === "exile" ? "701.13" : "701.20"];
  const match = `${path}/until/matching`;
  const subject = this.node({ id: this.id(match, "scan-candidate"), path: match, node: "characteristic-subject",
    scope: context.ability, role: "evaluated-candidate", timing: "resolution", rules });
  this.walk(until.matching, match, { ...context, characteristicSubject: subject, actor });
  const event = this.event(path, context, stepEventOperation(effect.effect), [actor, owner],
    context.conditional ? { conditional: true } : {});
  const identity = operation === "exile" ? "zone-successors" as const : "same-objects" as const;
  const pool = this.eventResult(path, event, "objects", "many", identity, { cls: "card" });
  const matches = this.eventResult(child(path, "until"), event, "objects",
    until.count.value === 1 ? "one" : "many", identity, { cls: "card" }, pool.id);
  this.node({ id: this.id(path, "scan"), path, node: "library-scan", scope: context.ability, actor: actor.id,
    owner: owner.id, operation, filter: until.matching, count: until.count.value, pool: pool.id, matches: matches.id, event, rules });
  this.engineFeature("library-scan", path,
    "Process the library in order until the requested number of matches or exhaustion. Keep the complete pool distinct from the matches.", rules);
  context.scans = [...(context.scans ?? []), { event, operation, pool, matches }];
  if (operation === "exile") context.uncertainExileOrigin = true;
  context.objects = combine(context.objects, [matches]);
  context.mentionedObject = true;
  if (actor.id !== context.you) context.players = combine(context.players, [actor]);
  return true;
}

export function copyCard(this: Analyzer, effect: Extract<Effect, { effect: "copy-card" }>, path: string, context: Context): boolean {
  if (context.barrier || context.decider || context.phase !== "resolution" || effect.forEach || effect.forEachVote ||
      effect.optional || effect.decidedBy || effect.who.player !== "you" ||
      effect.count.amount !== "fixed" || !Number.isSafeInteger(effect.count.value) || effect.count.value < 1 ||
      context.exileResults?.length !== 1 || context.uncertainExileOrigin || context.exileResults[0].faceDown ||
      (effect.what.ref !== "it" && effect.what.ref !== "that" && effect.what.ref !== "related")) return false;
  const card = this.object(effect.what, child(path, "what"), context);
  if (!card || card.id !== context.exileResults[0].result.id || card.cardinality !== "one") {
    this.diagnostic("unsupported-scope", path, "The copied card must be the one explicitly retained in exile.", ["707.12"]);
    return true;
  }
  const chooser = this.player(effect.who, child(path, "who"), context)!;
  const event = this.event(path, context, effect.effect, [card, chooser],
    context.conditional ? { conditional: true } : {});
  const result = this.eventResult(path, event, "objects", effect.count.value === 1 ? "one" : "many", "new-objects");
  const rules = ["112.2a", "608.2g", "704.5e", "707.12", "707.12a"];
  this.node({ id: this.id(path, "card-copy"), path, node: "card-copy", scope: context.ability,
    event, card: card.id, chooser: chooser.id, count: effect.count.value, result: result.id, rules });
  this.engineFeature("card-copy", path, "Create temporary copies in the original card's zone and offer any printed immediate casting choices.", rules);
  context.copiedCards = [...(context.copiedCards ?? []), result];
  context.objects = combine(context.objects, [result]);
  return true;
}

/** CR 115.7a–f, 707.10: copies retain the original choices and source.
 * Selecting new targets during copy creation is tied to the copy event. */
export function stackInstruction(this: Analyzer, effect: StackInstruction, path: string, context: Context, precedingEffect?: string): boolean {
  if (this.specializedStackInstruction(effect, path, context)) return true;
  if (context.barrier || context.phase !== "resolution" || effect.forEach || effect.forEachVote ||
      effect.decidedBy || effect.who.player !== "you" || effect === context.directCreatureSpellCopy) return false;
  const whatPath = child(path, "what");
  const whoPath = child(path, "who");
  const subject = effect.what;
  if (effect.effect === "choose-new-targets" && effect.timing === "copy-creation") {
    const [copy] = context.stackCopies ?? [];
    const previous = context.precedingSentence;
    // "Return ... then choose new targets for the copy" still specifies
    // an option at copy creation (CR 707.10c). The return's identity is
    // validated independently and doesn't supply a new copy.
    const afterReturn = previous?.steps.length === 2 && copy?.path === `${previous.path}/steps/0` &&
      precedingEffect === `${previous.path}/steps/1` && previous.steps[0].effect === "copy-stack-object" &&
      previous.steps[1].effect === "return-to-hand" && previous.steps[1].to === "owner" &&
      Object.keys(previous.steps[1]).length === 3;
    if (context.stackCopies?.length !== 1 || (copy.path !== precedingEffect && !afterReturn) ||
        context.uncertainStackOrigin || copy.inlineRetarget || copy.chooser !== context.you ||
        context.conditional || context.decider || !effect.optional || subject.ref !== "copy-result" ||
        copy.result.cardinality !== (subject.number === "singular" ? "one" : "many")) return false;
    const rules = ["115.7d", "115.7e", "115.7f", "707.10c"];
    const chooser = this.player(effect.who, whoPath, context);
    this.bound(subject, whatPath, copy.result.id, rules);
    this.event(path, context, stepEventOperation(effect.effect), [copy.result, ...(chooser ? [chooser] : [])],
      { optional: true, decider: copy.chooser });
    this.node({ id: this.id(path, "stack-instruction"), path, node: "stack-instruction", scope: context.ability,
      subject: copy.result.id, chooser: copy.chooser,
      action: { kind: "choose-new-targets", timing: "copy-creation", copyEvent: copy.event }, rules });
    context.stackCopies = [{ ...copy, inlineRetarget: true }];
    return true;
  }

  if (effect.effect === "change-text") {
    if (subject.ref !== "target" || objectCardinality(subject) !== "one" ||
        !effect.replacements.length || effect.optional || context.decider) return false;
    const candidate = this.object(subject, whatPath, context);
    if (!candidate) return false;
    const chooser = this.player(effect.who, whoPath, context);
    const rules = ["608.2d", "611.2a", "612.1", "612.2", "613.1c"];
    this.event(path, context, stepEventOperation(effect.effect), [candidate, ...(chooser ? [chooser] : [])],
      context.conditional ? { conditional: true } : {});
    this.node({ id: this.id(path, "text-choice"), path, node: "text-choice", scope: context.ability,
      subject: candidate.id, chooser: context.you, replacements: effect.replacements, rules });
    this.engineFeature("text-change", path, "Choose the printed word replacement and apply it only where the word has the chosen rules meaning.", rules);
    context.objects = combine(context.objects, [candidate]);
    return true;
  }

  const stack = subject.ref === "target" ? stackClasses(subject.filter)
    : subject.ref === "this" && subject.noun === "spell" && context.spellAbility ? "spell"
    : subject.ref === "trigger-event-object" && context.triggerObject?.event === subject.event
      ? subject.event === "cast" ? "spell" : "ability" : undefined;
  if (!stack || objectCardinality(subject) !== "one") return false;
  if (effect.effect === "change-targets" && effect.scope === "each") return false;
  if (effect.effect === "change-targets" && (effect.to || effect.newTarget || effect.reselection || effect.also)) return false;
  if (effect.effect === "copy-stack-object" && (effect.copyExceptions || effect.fixedTargets ||
      (effect.copyGrant && (stack !== "spell" || effect.count.amount !== "fixed" || effect.count.value !== 1 ||
        !effect.copyGrant.abilities.every(grant => grant.gained === "keyword" || grant.gained === "quoted"))) ||
      effect.count.amount !== "fixed" || !Number.isSafeInteger(effect.count.value) || effect.count.value < 1 ||
      (effect.retarget && (effect.retarget.who.player !== "you" || !effect.retarget.optional)))) return false;

  const candidate = this.object(subject, whatPath, context);
  if (!candidate) return false;
  const chooser = this.player(effect.who, whoPath, context)!;
  if (effect.optional) {
    this.rememberDecision(path, context);
    context.decisions?.push(path);
  }
  context.objects = combine(context.objects, [candidate]);
  const at = this.id(path, "stack-instruction");
  const scope = context.ability;
  const event = this.event(path, context, effect.effect, [candidate, chooser], {
    ...(effect.optional || context.decider ? { optional: true as const, decider: context.decider ?? chooser.id } : {}),
    ...(context.conditional ? { conditional: true as const } : {}),
  });
  if (effect.effect === "copy-stack-object" && effect.count.amount === "fixed") {
    const rules = ["707.10", "707.10b", "707.10c", "707.10f"];
    const count = effect.count.value;
    if (effect.retarget) this.player(effect.retarget.who, `${path}/retarget/who`, context);
    const result = this.eventResult(path, event, "objects", count === 1 ? "one" : "many", "new-objects",
      stack === "spell-or-ability" ? undefined : { cls: stack });
    this.node({ id: at, path, node: "stack-instruction", scope, subject: candidate.id, chooser: chooser.id,
      action: { kind: "copy", event, count, retarget: Boolean(effect.retarget) }, rules });
    if (effect.copyGrant) {
      const grantPath = child(path, "copyGrant");
      const grantRules = ["400.7a", "608.3f", "611.2c", "707.10f"];
      const successor = this.node({ id: this.id(grantPath, "copy-grant"), path: grantPath, node: "permanent-spell-copy-grant",
        scope, copy: result.id, applies: "permanent-spell-and-resulting-token", cardinality: "one", rules: grantRules });
      const grantContext: Context = { ...context, recipient: { candidate: { id: successor, domain: "object", cardinality: "one" } } };
      effect.copyGrant.abilities.forEach((grant, index) => this.gained(grant, `${grantPath}/abilities/${index}`, grantContext));
      this.engineFeature("permanent-spell-copy", grantPath, "Apply the printed separate grant to the permanent spell copy and the token it becomes.", grantRules);
    }
    // The copy is not the original. A later bare pronoun with both in
    // scope is ambiguous; an explicit copy-result has its own capture.
    context.objects = combine(context.objects, [result]);
    context.quantities = combine(context.quantities,
      [this.eventResult(child(path, "count"), event, "quantity", "one", "measured-quantity")]);
    context.stackCopies = [...(context.stackCopies ?? []), { path, event, result, chooser: chooser.id, inlineRetarget: Boolean(effect.retarget) }];
    this.engineFeature("stack-instruction", path, "Create copies on the stack with inherited choices and the original ability source. Apply printed retargeting during creation.", rules);
  } else if (effect.effect === "change-targets" || effect.effect === "choose-new-targets") {
    const changeScope = effect.effect === "change-targets" ? effect.scope : undefined;
    if (changeScope === "each") return false;
    const rules = [changeScope === undefined ? "115.7d"
      : changeScope === "all" ? "115.7a" : changeScope === "one" ? "115.7b" : "115.7c", "115.7e", "115.7f"];
    this.node({ id: at, path, node: "stack-instruction", scope, subject: candidate.id, chooser: chooser.id,
      action: changeScope === undefined ? { kind: "choose-new-targets", timing: "resolution" }
        : { kind: "change-targets", scope: changeScope }, rules });
    this.engineFeature("stack-instruction", path, "Apply the printed target-change procedure to the bound stack object, preserving modes and division.", rules);
  }
  return true;
}
export function insertUpkeep(this: Analyzer, effect: Extract<Effect, { effect: "add-steps" }>, path: string, context: Context): void {
  const rules = ["500.2", "500.9", "503.1", "805.8"];
  const matched = context.firstUpkeep;
  if (context.barrier || context.phase !== "resolution" || matched?.scope !== context.ability) {
    this.unresolved(effect.who, child(path, "who"), "unsupported-scope",
      "An added upkeep after this step requires the matched upkeep occurrence and player.", rules);
    return;
  }
  this.walk(effect.count, child(path, "count"), context);
  const player = this.bound(effect.who, child(path, "who"), matched.player.id, ["113.7a", "603.2c"]);
  const event = this.event(path, context, effect.effect, [player], context.conditional ? { conditional: true } : {});
  this.node({ id: this.id(path, "upkeep-insertion"), path, node: "upkeep-insertion", scope: context.ability,
    event, player: player.id, after: matched.occurrence, count: effect.count, placement: "same-phase", order: "most-recent-first", rules });
  this.engineFeature("phase-scheduling", path,
    "Insert the additional upkeep after the captured step in the same phase, newest insertion first.", rules);
}

export function secretChoiceDamage(this: Analyzer, effect: Extract<Effect, { effect: "secret-choice-damage" }>, path: string, context: Context): void {
  const rules = ["101.1", "101.4", "102.2", "109.5", "608.2c", "608.2d", "608.2f", "608.2h", "701.38c"];
  if (context.barrier || context.phase !== "resolution") {
    this.unresolved(effect.who, child(path, "who"), "unsupported-scope",
      "Private choices and their shared reveal require a single resolution scope.", rules);
    return;
  }
  const choosers = this.player(effect.who, child(path, "who"), context);
  if (!choosers) return;
  const outcomePath = (index: number) => child(child(path, "outcomes"), index);
  const outcomeId = (index: number) => this.id(outcomePath(index), "secret-choice-outcome");
  const sources = effect.outcomes.map((outcome, index) =>
    this.object(outcome.source, child(outcomePath(index), "source"), { ...this.branch(context), lastKnown: true }));
  if (!sources.every(source => source !== undefined)) return;
  const event = this.event(path, context, stepEventOperation(effect.effect), [choosers], context.conditional ? { conditional: true } : {});
  const revealed = this.node({ id: this.id(path, "secret-choice-reveal"), path, node: "secret-choice-reveal",
    scope: context.ability, event, choosers: choosers.id, controller: context.you, options: effect.choices,
    collection: "private-apnap", count: "one-per-chooser", reveal: "simultaneous", rules });
  for (const [index, outcome] of effect.outcomes.entries()) {
    const at = outcomePath(index);
    const recipients = outcome.when === "unanimous" ? choosers : {
      id: this.node({ id: this.id(at, "secret-choice-subset"), path: child(at, "recipients"),
        node: "secret-choice-subset", scope: context.ability, revealed, chose: outcome.recipients.chose,
        cardinality: "many", rules }),
      cardinality: "many" as const, domain: "player" as const,
    };
    // Preserve printed precedence even if there are no remaining choosers
    // and both universal predicates would otherwise be vacuously true.
    const when = outcome.when === "unanimous" ? {
      kind: "unanimous" as const, choice: outcome.choice, ...(index === 1 ? { unless: outcomeId(0) } : {}),
    }
      : { kind: "otherwise" as const, unless: [outcomeId(0), outcomeId(1)] as [string, string] };
    const condition = this.node({ id: outcomeId(index), path: at, node: "secret-choice-outcome", scope: context.ability,
      revealed, recipients: recipients.id, when,
      damage: { amount: outcome.amount.value, timing: "simultaneous", sourceInformation: "current-or-last-known" }, rules });
    this.event(at, context, "damage", [sources[index], recipients], { conditional: true, condition, requiresResult: revealed });
  }
  this.engineFeature("multiplayer-choice", path,
    "Collect one private choice from each opponent, reveal them together, and deal damage to the recipients of the single matching outcome.", rules);
  context.unexportedSecretChoices = true;
}

export function effect(this: Analyzer, effect: Effect, path: string, context: Context): void {
  const precedingEffect = context.previousEffect;
  context.previousEffect = path;
  const lookChoice = context.lookedCardChoice?.path === precedingEffect ? context.lookedCardChoice : undefined;
  context.lookedCardChoice = undefined;
  const cardChoice = context.directCardChoice?.path === precedingEffect ? context.directCardChoice : undefined;
  context.directCardChoice = undefined;
  const exportedCast = context.explicitExiledCast?.path === precedingEffect ? context.explicitExiledCast : undefined;
  context.explicitExiledCast = undefined;
  if (effect.effect === "add-steps") return this.insertUpkeep(effect, path, context);
  if (effect.effect === "secret-choice-damage") return this.secretChoiceDamage(effect, path, context);
  if (effect.effect === "choose-object") context.uncertainTargetOrigin = true;
  const graveyardReturn = this.graveyardReturnScope(effect, path, context);
  if (effect.effect === "reveal") context.uncertainRevealOrigin = true;
  if (this.distinctAction(effect, path, context)) {
    // A targeting rule records its permission; the step is its event.
    if (effect.effect === "targeting-as-though")
      this.event(path, context, stepEventOperation(effect.effect), [{ id: context.you, domain: "player", cardinality: "one" }],
        context.conditional ? { conditional: true } : {});
    return;
  }
  if (this.relativePlayerAction(effect, path, context)) return;
  if (this.afterMultiplayerChoices(effect, path, context, precedingEffect)) return;
  if (effect.effect === "choose-object" && this.multiplayerChoices(effect, path, context)) return;
  if (this.targetedPlayerAction(effect, path, context)) return;
  if (effect.effect === "grant-look-permission" && this.lookAtFaceDownExile(effect, path, context)) return;
  if (effect.effect === "create-replacement" &&
      (this.delayedExileReturn(effect, path, context) || this.moveToExileRule(effect, path, context, precedingEffect))) return;
  if (effect.effect === "repeat-process" && this.repeatHandPlacement(effect, path, context, precedingEffect)) {
    this.event(path, context, stepEventOperation(effect.effect), [], context.conditional ? { conditional: true } : {});
    return;
  }
  if (effect.effect === "add-mana" && this.commanderManaRider(effect, path, context)) return;
  if (effect.effect === "roll-dice") {
    if (this.numericRoll(effect, path, context)) return;
    context.uncertainDieOrigin = true;
  } else if (effect.effect === "roll-again") context.uncertainDieOrigin = true;
  if (effect.effect === "attach" && this.randomAuraAttachment(effect, path, context)) return;
  if (effect.effect === "damage" && this.damageByEachSource(effect, path, context)) return;
  if (effect.effect === "draw" && (this.historicalDamageDraw(effect, path, context) || this.groupedDraw(effect, path, context))) return;
  if (effect.effect === "for-each" && this.quantifiedInstruction(effect, path, context, precedingEffect)) return;
  if (effect.effect === "scan-library" && this.libraryScan(effect, path, context)) return;
  if (effect.effect === "copy-card" && (this.copyLinkedCards(effect, path, context) || this.copyCard(effect, path, context))) return;
  if (effect.effect === "play-card" && this.castLinkedCopies(effect, path, context, precedingEffect)) return;
  if ((effect.effect === "copy-stack-object" || effect.effect === "change-targets" ||
      effect.effect === "choose-new-targets" || effect.effect === "change-text") &&
      this.stackInstruction(effect, path, context, precedingEffect)) return;
  if (effect.effect === "grant-cast-keyword") {
    const caster = this.player(effect.caster, child(path, "caster"), context);
    const from = child(path, "from");
    this.walk(effect.from, from, context);
    const owner = this.at(child(from, "owner"));
    const what = child(path, "what");
    if (!caster || !owner) {
      this.unresolved(effect.what, what, "unresolved-dependency",
        "The recipient spells require a bound caster and hand owner.", ["109.5", "400.1", "601.2a"]);
      return;
    }
    const grant = this.node({ id: this.id(path, "grant"), path, node: "cast-keyword-grant",
      scope: context.ability, created: "resolution", duration: effect.duration.duration, keyword: effect.keyword,
      rules: ["113.7a", "514.2", "603.3a", "611.2a", "611.2c", "611.2f"] });
    const spells = this.node({ id: this.id(what, "spells"), path: what, node: "future-cast-spells",
      grant, caster: caster.id, from: { zone: effect.from.zone, owner: owner.id }, timing: effect.timing,
      cardinality: "many", rules: ["601.2a", "601.2i", "611.2f"] });
    this.bound(effect.what, what, spells, ["601.2a", "611.2f", "702.85a"]);
    this.event(path, context, stepEventOperation(effect.effect), [caster], context.conditional ? { conditional: true } : {});
    this.engineFeature("cast-keyword-grant", path,
      "Keep one source-independent grant per resolution and expire it during cleanup. Apply each grant to that controller's later completed casts from hand before collecting cast triggers.",
      ["109.5", "113.7a", "514.2", "601.2i", "603.3a", "611.2a", "611.2f", "702.85a", "702.85c"]);
    return;
  }
  if (effect.effect === "modify-characteristics") {
    // "Target land becomes a 3/3 creature": the engine applies the change in
    // the layer system with the affected set fixed as it begins (CR 611.2c,
    // 613). Its references bind as anywhere else, and what it changes is
    // still an antecedent ("It's still a land").
    this.engineFeature("characteristic-layers", path, "Characteristic changes need the engine's layer system, with the affected set and values fixed when the effect begins.", ["611.2c", "611.2d", "613.6", "604.3a"]);
    this.walk(effect, path, context);
    const changed = this.at(child(path, "what"));
    if (changed?.domain === "object") context.objects = combine(context.objects, [changed]);
    return;
  }
  if (effect.effect === "gain-abilities" && "recipient" in effect.what && !unambiguousPluralRecipient(context)) {
    this.walk(effect, path, { ...context, barrier: "An unresolved ability recipient cannot establish an object or player antecedent." });
    context.objects = []; context.players = []; context.quantities = [];
    return;
  }
  if (effect.effect === "for-each") {
    const barrier = "Quantified instructions require member references, original-ability target multiplicity, membership timing, and aggregated result bindings.";
    this.diagnostic("unsupported-scope", path, barrier, ["608.2e", "608.2f", "101.4", "115.3"]);
    this.walk(effect, path, { ...context, barrier });
    context.uncertainExileOrigin ||= containsEffect(effect, "exile") || containsEffect(effect, "exile-until");
    context.uncertainStackOrigin ||= containsEffect(effect, "copy-stack-object");
    context.uncertainRevealOrigin ||= containsEffect(effect, "reveal");
    context.uncertainDieOrigin ||= containsEffect(effect, "roll-dice") || containsEffect(effect, "roll-again");
    context.uncertainTargetOrigin ||= containsEffect(effect, "choose-object") || containsEffect(effect, "declare-target");
    context.objects = []; context.players = []; context.quantities = [];
    return;
  }
  if ("who" in effect && effect.who?.player === "group") {
    const barrier = "Grouped actions preserve participant occurrences and shared-event multiplicity; result identities need explicit per-role aggregation.";
    this.diagnostic("unsupported-scope", path, barrier, ["101.4", "121.2c", "608.2e", "608.2f"]);
    this.walk(effect, path, { ...context, barrier });
    context.uncertainExileOrigin ||= containsEffect(effect, "exile") || containsEffect(effect, "exile-until");
    context.uncertainStackOrigin ||= containsEffect(effect, "copy-stack-object");
    context.uncertainRevealOrigin ||= containsEffect(effect, "reveal");
    context.uncertainDieOrigin ||= containsEffect(effect, "roll-dice") || containsEffect(effect, "roll-again");
    context.uncertainTargetOrigin ||= containsEffect(effect, "choose-object") || containsEffect(effect, "declare-target");
    context.objects = []; context.players = []; context.quantities = [];
    return;
  }
  const plain = !effect.optional && !effect.forEach && !effect.forEachVote;
  if (plain && effect.effect === "look-top-may-reveal-for-mana") {
    this.libraryTopMana(effect, path, context);
    // Its parts carry their own events; the step's event exists once
    // they are all bound (the engine feature marks that).
    const controller = this.at(child(path, "who"));
    if (controller && this.result.engineFeatures.some(need => need.path === path && need.feature === effect.effect))
      this.event(path, context, stepEventOperation(effect.effect), [controller], context.conditional ? { conditional: true } : {});
    return;
  }
  const weights = plain && effect.effect === "modal" && !context.barrier && !context.conditional && !context.decider &&
    context.spellAbility && "weightBudget" in effect.count && Number.isSafeInteger(effect.count.weightBudget) &&
    effect.count.weightBudget > 0 && !effect.conditionalCount && !effect.random && !effect.chooser && !effect.keyword &&
    effect.options.length > 0 && effect.options.every(option => !option.additionalCost && option.weight !== undefined &&
      Number.isSafeInteger(option.weight) && option.weight > 0)
    ? effect.options.map(option => option.weight!) : undefined;
  if (plain && effect.effect === "modal" && !effect.unchosen && !effect.distinctTargetPlayers &&
      ("weightBudget" in effect.count ? weights !== undefined
        : !effect.allowRepeat || ("exactly" in effect.count && !effect.conditionalCount && !effect.random && !effect.chooser && !effect.keyword))) {
    if (effect.allowRepeat) this.engineFeature("repeatable-modal-selection", path,
      "Each chosen occurrence needs its own announced targets and resolves in printed mode order.", ["700.2", "601.2b", "601.2c", "608.2c"]);
    this.modal(effect, path, context, weights);
    return;
  }
  if (effect.effect === "choice" && (plain || (!context.barrier && effect.optional && !effect.decidedBy &&
      !effect.forEach && !effect.forEachVote && effect.who.player === "you"))) {
    if (effect.optional) {
      this.rememberDecision(path, context);
      context.decisions?.push(path);
      this.event(path, context, stepEventOperation(effect.effect), [{ id: context.you, domain: "player", cardinality: "one" }],
        { optional: true, decider: context.you, ...(context.conditional ? { conditional: true as const } : {}) });
    }
    this.choice(effect, path, context);
    return;
  }
  if (plain && effect.effect === "flip-coin" && !effect.until && effect.outcomes?.length && effect.count.amount === "fixed" && effect.count.value === 1) {
    this.flipCoin(effect, path, context); return;
  }
  if (plain && effect.effect === "roll-dice" && effect.table && !effect.ignore && effect.count.amount === "fixed" && effect.count.value === 1
    && !JSON.stringify(effect.table).includes('"effect":"roll-again"')) {
    this.dieTable(effect, path, context); return;
  }
  if (plain && effect.effect === "simultaneous") { this.simultaneous(effect, path, context); return; }
  if (plain && effect.effect === "optional-group") {
    if (!this.controllerSearch(effect, path, context, precedingEffect)) this.optionalGroup(effect, path, context);
    return;
  }
  if (effect.effect === "pay-cost" && !effect.times && !effect.forEach && !effect.forEachVote) { this.payCost(effect, path, context); return; }
  if (plain && effect.effect === "create-delayed-trigger") {
    if (!this.nextCreatureEntryCounter(effect, path, context)) this.delayedTrigger(effect, path, context);
    return;
  }
  if (plain && effect.effect === "create-reflexive-trigger") { this.reflexiveTrigger(effect, path, context); return; }
  if (plain && effect.effect === "self-replacement" && !effect.replacementChoice) {
    if (!this.choiceOfCopyCount(effect, path, context)) this.selfReplacement(effect, path, context);
    return;
  }
  if (["optional-group", "choice", "villainous-choice", "simultaneous", "modal", "create-delayed-trigger", "create-reflexive-trigger", "create-replacement", "self-replacement", "pay-cost", "roll-dice", "roll-again", "flip-coin"].includes(effect.effect) || effect.forEachVote) {
    const barrier = "Nested, optional, simultaneous, or delayed instruction scopes need explicit capture and merge rules.";
    this.diagnostic("unsupported-scope", path, barrier, ["603.7", "608.2c"]);
    this.walk(effect, path, { ...context, barrier });
    context.uncertainExileOrigin ||= containsEffect(effect, "exile") || containsEffect(effect, "exile-until");
    context.uncertainStackOrigin ||= containsEffect(effect, "copy-stack-object");
    context.uncertainRevealOrigin ||= containsEffect(effect, "reveal");
    context.uncertainDieOrigin ||= containsEffect(effect, "roll-dice") || containsEffect(effect, "roll-again");
    context.uncertainTargetOrigin ||= containsEffect(effect, "choose-object") || containsEffect(effect, "declare-target");
    context.objects = []; context.players = []; context.quantities = [];
    return;
  }
  if (effect.optional) this.rememberDecision(path, context);
  const castPool = !context.barrier && !context.decider && !context.uncertainExileOrigin &&
    !context.exileResults?.[0]?.faceDown &&
    context.exileResults?.length === 1 && precedingEffect &&
    context.exileResults[0].event === this.id(precedingEffect, "event") ? context.exileResults[0].result : undefined;
  const immediateCasts = effect.effect === "play-card" && effect.optional && !effect.forEach && !effect.forEachVote &&
    !effect.decidedBy && castPool?.cardinality === "many" &&
    effect.instruction.who.player === "you" && effect.instruction.action === "cast" &&
    effect.instruction.what.ref === "filter" && isDeepStrictEqual(effect.instruction.what.filter, { cls: "spell" }) &&
    isDeepStrictEqual(effect.instruction.what.count, { amount: "chosen", minimum: 0 }) &&
    isDeepStrictEqual(effect.instruction.alternativeCost, { cost: "without-mana-cost" }) &&
    isDeepStrictEqual(effect.instruction.fromAmong, { ref: "that", filter: { cls: "card" }, plural: true });
  const sharedLandSearch = plain && !context.barrier && !context.decider && context.phase === "resolution" &&
    effect.effect === "search" && effect.who.player === "you" &&
    isDeepStrictEqual(effect.zones, { selection: "all", of: [{ zone: "library", owner: "your" }] }) &&
    effect.for.ref === "filter" && effect.for.upTo && effect.for.count.amount === "fixed" &&
    effect.for.count.value === 2 && isDeepStrictEqual(effect.for.filter, { supertypes: ["basic"], types: ["land"], cls: "card" }) &&
    isDeepStrictEqual(effect.for.groupConstraints, [{ constraint: "shared-value", characteristic: "land-type" }]);
  const slots = plain && !context.barrier && context.phase === "resolution" ? coloredSearchSlots(effect) : undefined;
  if (sharedLandSearch && effect.effect === "search" && effect.for.ref === "filter") {
    this.walk(effect.who, child(path, "who"), context);
    this.walk(effect.zones, child(path, "zones"), context);
    const selection = child(path, "for");
    const rules = ["205.3i", "608.2d", "701.23b"];
    const event = this.id(path, "event");
    const query = this.node({ id: this.id(selection, "shared-land-type"), path: selection, node: "shared-land-type-search",
      scope: context.ability, chooser: context.you, library: context.you, event,
      filter: effect.for.filter, maximum: 2, characteristic: "land-type", rules });
    this.bound(effect.for, selection, query, rules);
    this.engineFeature("shared-land-type-search", path,
      "Choose zero through two qualifying cards, applying the same land type constraint to any pair that is found.", rules);
  } else if (slots && effect.effect === "search") {
    const selection = child(path, "for");
    this.walk(effect.who, child(path, "who"), context);
    this.walk(effect.zones, child(path, "zones"), context);
    const allocation = this.node({ node: "distinct-library-search", id: this.id(selection, "allocation"), path: selection,
      scope: context.ability, chooser: context.you, libraryOwner: context.you, slots, allocation: "distinct-objects",
      perSlot: "zero-or-one", cardinality: "many", rules: ["608.2d", "701.23b"] });
    this.bound(effect.for, selection, allocation, ["608.2d", "701.23b"]);
  } else {
    const count = immediateCasts ? { path: `${path}/instruction/what`, operation: "cast-from-exile" as const }
      : resolutionCount(effect, path);
    const suspension = effect.effect === "gain-abilities" && exportedCast && context.conditional &&
      isDeepStrictEqual(effect.what, { ref: "it" }) &&
      isDeepStrictEqual(effect.abilities, [{ gained: "keyword", keyword: { keyword: "suspend" } }])
      ? { path: child(path, "what"), candidate: exportedCast.result, rules: ["400.7j", "608.2c", "702.62"] } : undefined;
    let remainder: Context["boundOperand"];
    if (lookChoice && effect.effect === "move-zone" && plain && !context.barrier && !context.decider &&
        isDeepStrictEqual(effect.what, { ref: "rest" }) && effect.order === "random" &&
        isDeepStrictEqual(effect.to, { zone: "library", position: "bottom", owner: "your" })) {
      const at = child(path, "what");
      const rules = ["608.2c", "608.2d", "609.3"];
      const id = this.node({ id: this.id(at, "look-remainder"), path: at, node: "remainder", scope: context.ability,
        pool: lookChoice.pool.id, excluding: lookChoice.excluding, rules });
      remainder = { path: at, candidate: { id, domain: "object", cardinality: "many" }, rules };
      this.engineFeature("optional-look-remainder", path, "Use all remaining looked-at cards. When the choice was declined, use the full pool.", rules);
    }
    const sacrificed = context.spellAbility && precedingEffect && context.sacrificedCost && effect.effect === "gain-life" &&
      effect.who.player === "you" && isDeepStrictEqual(effect.amount, { amount: "attribute", attribute: "toughness", of: { ref: "it" } }) &&
      this.at(`${precedingEffect}/amount/of`)?.id === context.sacrificedCost.id &&
      context.objects.every(candidate => candidate.description?.cls === "card" || candidate.id === context.sacrificedCost?.id) &&
      !context.departed?.length &&
      this.result.nodes.some(node => node.id === this.id(precedingEffect, "event") && node.node === "event" && node.operation === "draw")
      ? { path: `${path}/amount/of`, candidate: context.sacrificedCost, rules: ["601.2h", "608.2h", "701.21a", "707.10"] } : undefined;
    if (sacrificed) this.engineFeature("event-history", path,
      "Retain the same sacrificed permanent's characteristics from the spell's actually paid additional cost.", sacrificed.rules);
    const chosenCard = cardChoice && effect.effect === "grant-play-permission" &&
      effect.permission.who.player === "you" && !effect.permission.fromAmong &&
      isDeepStrictEqual(effect.permission.what, { ref: "that", filter: { cls: "card" } })
      ? { path: `${path}/permission/what`, candidate: cardChoice.result, rules: ["400.7j", "608.2c", "608.2d"] } : undefined;
    const grantedExile = this.immediateExilePlay(effect, path, context, precedingEffect);
    const unannouncedDivision = effect.effect === "distribute-counters" && !announcedDivision(effect);
    this.walk(effect, path, count || unannouncedDivision || chosenCard || suspension || remainder || sacrificed || grantedExile
      ? { ...context, resolutionCount: count, unannouncedDivision,
          boundOperand: chosenCard ?? suspension ?? remainder ?? sacrificed ?? grantedExile } : graveyardReturn);
  }
  // CR 608.2d: "[player] may" is decided while the instruction is applied;
  // declining it produces no results, as any impossible instruction would.
  const decider = effect.optional ? (effect.decidedBy ? this.at(child(path, "decidedBy"))?.id : context.you)
    : context.decider === "unresolved" ? undefined : context.decider;
  const objectPaths: string[] = [];
  const playerPaths: string[] = [];
  let produces: "objects" | "quantity" | undefined;
  let cardinality: BindingCardinality = "unknown";
  let identity: Extract<BindingNode, { node: "event-result" }>["identity"] = "same-objects";
  switch (effect.effect) {
    case "search": objectPaths.push(child(path, "for")); playerPaths.push(child(path, "who")); break;
    case "choose-object": objectPaths.push(child(path, "what")); playerPaths.push(child(path, "who")); break;
    case "separate-piles": objectPaths.push(child(path, "what")); playerPaths.push(child(path, "who")); break;
    // "Exile ... until this leaves the battlefield" (CR 610.3): an exile whose
    // return is a later, separate one-shot effect.
    case "exile-until":
      effect.what.forEach((_, index) => objectPaths.push(child(child(path, "what"), index)));
      if (effect.who) playerPaths.push(child(path, "who"));
      produces = "objects"; identity = "zone-successors";
      break;
    case "exile": case "move-zone": case "return-to-hand":
      objectPaths.push(child(path, "what"));
      if (effect.who) playerPaths.push(child(path, "who"));
      // The public-zone successor is trackable under CR 400.7j. Following
      // a hidden-zone move needs additional rules and remains diagnosed.
      if (effect.effect === "exile" || (effect.effect === "move-zone" && ["battlefield", "graveyard", "exile", "stack", "command"].includes(effect.to.zone))) {
        produces = "objects"; identity = "zone-successors";
      }
      break;
    case "create-token": playerPaths.push(child(path, "who")); produces = "objects"; cardinality = cardCount(effect.count); identity = "new-objects"; break;
    case "draw": case "mill":
      playerPaths.push(child(path, "who")); produces = "objects"; cardinality = cardCount(effect.amount); identity = "new-objects"; break;
    case "gain-life": case "lose-life":
      playerPaths.push(child(path, "who")); produces = "quantity"; cardinality = "one"; identity = "measured-quantity"; break;
    case "damage":
      objectPaths.push(child(path, "source"));
      produces = "quantity"; cardinality = "one"; identity = "measured-quantity";
      // Damage's recipient is also a salient participant; treating its
      // source as an automatic "it" antecedent would be a false binding.
      if (effect.to.to === "object") objectPaths.push(child(child(path, "to"), "ref"));
      // "Deals 2 damage to target player. That player discards a card."
      else if (effect.to.to === "player") playerPaths.push(child(child(path, "to"), "ref"));
      else if (effect.to.to === "any-target") objectPaths.push(child(path, "to"));
      else if (effect.to.to === "divided") objectPaths.push(child(child(path, "to"), "among"));
      else if (effect.to.to === "them") this.diagnostic("unsupported-scope", child(path, "to"), "A bare \"them\" damage recipient needs its player-or-object antecedent bound.", ["109.1", "608.2c"]);
      else if (effect.to.to === "each") effect.to.refs.forEach((_ref, index) => objectPaths.push(child(child(child(path, "to"), "refs"), index)));
      break;
    case "fight": objectPaths.push(child(path, "a"), child(path, "b")); break;
    case "fight-each-other": objectPaths.push(child(path, "what")); break;
    case "look-at":
      objectPaths.push(child(path, "what")); playerPaths.push(child(path, "who"));
      // "Look at the top three cards of your library": those cards (CR 401.5).
      if (!("ref" in effect.what)) { produces = "objects"; identity = "same-objects"; cardinality = cardCount(effect.what.top); }
      break;
    // A discarded card goes to its owner's graveyard, a public zone where
    // the rest of the effect can find it (CR 400.7j, 701.9a).
    case "discard":
      objectPaths.push(child(path, "what")); playerPaths.push(child(path, "who"));
      produces = "objects"; identity = "zone-successors"; break;
    case "reveal": case "sacrifice": case "gain-control":
      objectPaths.push(child(path, "what")); playerPaths.push(child(path, "who")); break;
    case "tap": case "untap":
      objectPaths.push(child(path, "what"));
      if (effect.who) playerPaths.push(child(path, "who"));
      break;
    case "counter":
      objectPaths.push(child(path, "what"));
      if (effect.countered?.to.zone === "exile") {
        produces = "objects"; identity = "zone-successors";
        this.engineFeature("counter-destination", path,
          "Find only the public successor created by a successful counter and apply its printed entry state.", ["400.7j", "701.5"]);
      }
      break;
    case "destroy": case "pump": case "set-pt":
    case "transform": case "convert": case "phase-out": case "phase-in":
    case "flip": case "detain": case "suspect": case "unsuspect": case "prepare": case "unprepare": case "become-status":
    case "gain-abilities": case "lose-abilities": case "cant-have-abilities": case "untap-step-suppression":
    case "become": case "cant": case "remove-from-combat": case "become-blocked": case "combat-rule":
      objectPaths.push(child(path, "what")); break;
    case "turn-face":
      objectPaths.push(child(path, "what"));
      if (effect.who) playerPaths.push(child(path, "who"));
      break;
    case "attach": objectPaths.push(child(path, "what"), child(path, "to")); playerPaths.push(child(path, "who")); break;
    case "unattach": objectPaths.push(child(path, "what"), child(path, "from")); playerPaths.push(child(path, "who")); break;
    case "put-counters": objectPaths.push(child(path, "on")); break;
    case "put-sticker": objectPaths.push(child(path, "on")); playerPaths.push(child(path, "who")); break;
    case "remove-counters": objectPaths.push(child(path, "from")); break;
    case "add-mana": case "get-counters": case "lose-counters": case "get-emblem": case "pay-cost": case "choose-value": case "player-gain-abilities":
    case "shuffle": case "shuffle-pile": case "choose-pile": case "win-game": case "lose-game": case "lose-unspent-mana": playerPaths.push(child(path, "who")); break;
    case "declare-target": objectPaths.push(child(path, "what")); break;
    case "shuffle-into-library":
      // CR 701.24: the objects go into a hidden zone; their last-known information remains.
      effect.what.forEach((_ref, index) => objectPaths.push(child(child(path, "what"), index)));
      playerPaths.push(child(path, "who"));
      break;
    // Instructions whose only references are their printed participants;
    // they produce no objects a later pronoun could mean.
    case "double-counters": case "put-counters-of": objectPaths.push(child(path, "on")); break;
    case "switch-pt": case "multiply-pt": objectPaths.push(child(path, "what")); break;
    case "move-counters": objectPaths.push(child(path, "from"), child(path, "to")); break;
    case "exchange-control": effect.what.forEach((_, index) => objectPaths.push(child(child(path, "what"), index))); break;
    case "add-turns": case "set-life-total": case "double-life-total": case "double-mana-pool": playerPaths.push(child(path, "who")); break;
    case "add-phases": case "end-turn": case "end-combat": case "player-rule": case "additional-land-plays": case "modify-spells-cast": case "cost-modification": break;
    // A prevention or redirection shield (an engine feature, recorded where
    // it is walked) establishes no object a later pronoun could mean.
    case "prevent-damage": case "redirect-damage": break;
    // A permission names the card it lets a player play; the card stays
    // where it is (CR 601.2a moves it only when it is cast).
    case "grant-play-permission":
      objectPaths.push(child(child(path, "permission"), "what")); playerPaths.push(child(child(path, "permission"), "who")); break;
    // The card moves only if it is cast, and the effect can then find the
    // spell it became (CR 400.7h), so it remains the antecedent.
    case "play-card":
      objectPaths.push(child(child(path, "instruction"), "what")); playerPaths.push(child(child(path, "instruction"), "who")); break;
    case "prohibit": case "cast-limit": case "loyalty-activation-limit": case "control-player": case "repeat-process": case "end-effect-action": case "legend-rule-exemption": case "player-combat-rule": case "targeting-as-though":
    case "distribute-counters":
      if (announcedDivision(effect) && this.at(child(path, "among"))?.domain === "object") {
        objectPaths.push(child(path, "among"));
        break;
      }
    // falls through
    case "skip-time": case "set-day-night":
    case "exchange-life-totals":
    case "exchange-values": case "exchange-zones": case "choose-random": case "choose-friend-or-foe":
    case "scan-library": case "grant-look-permission": case "self-replacement": case "look-top-may-reveal-for-mana":
      this.diagnostic("unsupported-scope", path, "This instruction's event/result associations are not yet established.", ["608.2c"]);
      if (effect.effect === "scan-library") context.uncertainScanOrigin = true;
      context.objects = []; context.players = []; context.quantities = [];
      return;
    case "copy-stack-object": case "copy-card": case "choose-new-targets":
      if (effect === context.directCreatureSpellCopy) {
        this.engineFeature("permanent-spell-copy", path,
          "Recheck the target, copy its spell decisions and copiable values with the printed exception, and resolve the copy as a token permanent.",
          ["601.2c", "608.2b", "707.9", "707.10", "707.10f"]);
        objectPaths.push(child(path, "what")); playerPaths.push(child(path, "who"));
        produces = "objects"; cardinality = "one"; identity = "new-objects";
        break;
      }
      this.diagnostic("unsupported-scope", path, "Stack-copy results and copy target-selection timing require their own binding scope.", ["707.10", "115.7"]);
      if (effect.effect !== "choose-new-targets") context.uncertainStackOrigin = true;
      if (effect.effect === "copy-card") context.uncertainCardCopyOrigin = true;
      context.objects = []; context.players = []; context.quantities = [];
      return;
    case "change-text":
      this.diagnostic("unsupported-scope", path, "Chosen words of a text-changing effect require their own binding scope.", ["612"]);
      context.objects = []; context.players = []; context.quantities = [];
      return;
    case "change-targets":
      this.diagnostic("unsupported-scope", path, "Changed targets of another spell or ability require their own binding scope.", ["115.7"]);
      context.objects = []; context.players = []; context.quantities = [];
      return;
    // CR 701, 725, 726: a keyword action's own choices (which creature
    // bolster picks, proliferate's recipients, support's targets) are fixed by
    // its rule, not printed references. Its printed participants bind
    // normally; an action that creates or reveals new objects adds them as a
    // result of unknown size, so a later pronoun is never resolved past them.
    case "airbend":
      // Airbend exiles its target (it may then be cast from exile).
      objectPaths.push(child(path, "what")); playerPaths.push(child(path, "who"));
      produces = "objects"; identity = "zone-successors"; break;
    case "manifest-dread":
      // CR 701.62a, 701.40a: at most one card becomes a face-down
      // creature. The other looked-at cards never enter this result.
      playerPaths.push(child(path, "who"));
      produces = "objects"; identity = "new-objects"; cardinality = "one"; break;
    case "manifest": case "cloak":
      // The card is turned face down and put onto the battlefield: a new object (CR 400.7, 701.40, 701.58).
      objectPaths.push(child(path, "what")); playerPaths.push(child(path, "who"));
      produces = "objects"; identity = "new-objects"; break;
    case "learn": case "open-attraction": case "assemble-contraption": case "incubate": case "create-role": case "amass":
    case "discover": case "endure":
      if ("what" in effect) objectPaths.push(child(path, "what"));
      if ("attachedTo" in effect) objectPaths.push(child(path, "attachedTo"));
      if ("who" in effect && effect.who) playerPaths.push(child(path, "who"));
      produces = "objects"; identity = "new-objects"; break;
    case "keyword-action":
      playerPaths.push(child(path, "who"));
      if (effect.action !== "proliferate") { produces = "objects"; identity = "new-objects"; }
      break;
    case "become-monarch": case "take-initiative": case "ring-tempts": case "forage": case "time-travel": case "planeswalk": case "chaos-ensues":
    case "behold": case "collect-evidence": case "fateseal": case "venture": case "earthbend": case "goad": case "unlock-door": case "lock-or-unlock-door": case "empower-jace": case "clash": case "vote":
    case "adapt": case "monstrosity": case "bolster": case "support": case "scry": case "surveil": case "blight": case "explore": case "connive": case "regenerate":
      if ("what" in effect) objectPaths.push(child(path, "what"));
      if ("who" in effect && effect.who) playerPaths.push(child(path, "who"));
      break;
    case "optional-group": case "choice": case "villainous-choice": case "simultaneous": case "modal": case "create-delayed-trigger": case "create-reflexive-trigger": case "create-replacement": case "roll-dice": case "roll-again": case "flip-coin": case "pay-cost":
      return; // Handled above; retained for exhaustiveness.
    default: {
      const unreachable: never = effect;
      throw new Error(`Unhandled effect variant: ${JSON.stringify(unreachable)}`);
    }
  }
  const objectParticipants = objectPaths.map(path => this.at(path)).filter((candidate): candidate is Candidate => candidate !== undefined);
  const objects = objectParticipants.filter(candidate => candidate.domain === "object" || candidate.domain === "object-or-player");
  const players = combine(objectParticipants.filter(candidate => candidate.domain === "player"),
    playerPaths.map(path => this.at(path)).filter((candidate): candidate is Candidate => candidate !== undefined));
  const event = this.event(path, context, effect.effect, [...objects, ...players], {
    ...(effect.optional || context.decider ? { optional: true as const, ...(decider ? { decider } : {}) } : {}),
    ...(effect.forEach ? { repeated: "for-each" as const } : {}), ...(context.conditional ? { conditional: true as const } : {}) });
  const division = effect.effect === "distribute-counters" ? announcedDivision(effect) : undefined;
  if (division) {
    // The announced targets are the recipients; the division belongs to the
    // ability that announces them (CR 601.2d, 602.2b).
    const announcing = this.result.nodes.find(node => node.id === (context.targetScope ?? context.ability));
    if (announcing?.node === "ability" && objects.length === 1)
      this.node({ id: this.id(path, "target-division"), path, node: "target-division", scope: context.ability,
        chooser: announcing.you, event, targets: objects[0].id, total: division.total, minimumEach: 1,
        rules: ["601.2c", "601.2d", "602.2b"] });
    else this.diagnostic("unsupported-number-binding", child(path, "among"),
      "The division requires its announcing ability and one target selection.", ["601.2d"]);
  }
  if (effect.effect === "exile" && effect.what.ref === "library-cards" &&
      (effect.what.of?.owner === "each-player" || effect.what.of?.owner === "each-opponent")) {
    const rules = ["400.1", "400.7j", "608.2f", "609.3"];
    this.engineFeature("multi-library-exile", path, "Process each eligible library separately and capture only cards actually moved to exile.", rules);
  }
  if (immediateCasts && castPool) {
    const limit = this.at(`${path}/instruction/what`);
    const pool = this.at(`${path}/instruction/fromAmong`);
    if (pool?.id === castPool.id && limit) {
      const rules = ["400.7j", "608.2d", "608.2g"];
      this.node({ id: this.id(path, "sequential-cast"), path, node: "sequential-cast", scope: context.ability,
        event, pool: pool.id, caster: context.you, limit: limit.id, rules });
      this.engineFeature("sequential-cast", path, "Let the caster cast eligible members one at a time during resolution and stop at any time.", rules);
    } else if (pool) {
      this.diagnostic("unsupported-scope", path, "The immediate casts need the exact captured exile set.", ["400.7j", "608.2g"]);
    }
  }
  if ((effect.effect === "exile" || effect.effect === "exile-until") && context.linkedExile?.path === path) {
    const rules = ["400.7", context.linkedExile.rule, "607.3", "607.5", ...(effect.effect === "exile-until" ? ["610.3"] : [])];
    this.node({ id: this.id(path, "linked-exile-pool"), path, node: "linked-exile-pool", scope: context.ability,
      source: context.source, producer: context.ability, event, membership: "still-in-exile",
      identity: "source-incarnation", cardinality: "many", rules });
    this.engineFeature("linked-exile-pool", path, "Record only cards exiled by this linked ability instance and still in exile.", rules);
  }
  if (sharedLandSearch && effect.effect === "search" && objects.length === 1) {
    const found = this.eventResult(path, event, "objects", "many", "same-objects", { cls: "card", types: ["land"] });
    context.objects = combine(context.objects.filter(candidate => candidate.id !== objects[0].id), [found]);
    return;
  }
  if (effect.effect === "become" && "copyOf" in effect.spec) {
    const model = this.at(`${path}/spec/copyOf`);
    if (model && model.cardinality === "one" && this.announced(model)) {
      context.objects = combine(context.objects, [model]);
      context.mentionedObject = true;
    }
  }
  if (!context.barrier && !context.decider && effect.effect === "move-zone" && effect.optional && !effect.decidedBy &&
      !effect.who && !effect.forEach && !effect.forEachVote && effect.to.zone === "battlefield" &&
      effect.what.ref === "filter" && objectCardinality(effect.what) === "one" &&
      isDeepStrictEqual(effect.what.filter, { types: ["land"], cls: "card", fromAmong: { ref: "them" } }) &&
      context.precedingSentence?.path && precedingEffect === `${context.precedingSentence.path}/steps/0` &&
      context.precedingSentence.steps.length === 1 && context.precedingSentence.steps[0].effect === "look-at" &&
      !context.precedingSentence.steps[0].optional && context.precedingSentence.steps[0].who.player === "you" &&
      !("ref" in context.precedingSentence.steps[0].what) &&
      isDeepStrictEqual(context.precedingSentence.steps[0].what.of, { zone: "library", owner: "your" }) &&
      !context.precedingSentence.steps[0].forEach && !context.precedingSentence.steps[0].forEachVote &&
      objects.length === 1) {
    const pool = this.at(`${path}/what/filter/fromAmong`);
    if (pool && pool.id === this.id(precedingEffect, "objects")) {
      const selected = this.eventResult(child(path, "selection"), event, "objects", "one", "same-objects",
        { cls: "card", types: ["land"] }, pool.id);
      context.lookedCardChoice = { path, pool, excluding: selected.id };
    }
  }
  if (effect.effect === "choose-object" && !effect.optional && !effect.forEach && !effect.forEachVote && !effect.decidedBy &&
      !context.decider && castPool?.cardinality === "many" && effect.who.player === "you" &&
      isDeepStrictEqual(effect.what, { ref: "from-pool", pool: { ref: "them" }, count: { amount: "fixed", value: 1 } }) &&
      this.at(`${path}/what/pool`)?.id === castPool.id && objects.length === 1) {
    const result = this.eventResult(path, event, "objects", "one", "same-objects", { cls: "card" }, castPool.id);
    context.directCardChoice = { path, result };
  }
  if (effect.effect === "declare-target" && objects.length === 1 && this.announced(objects[0]))
    context.declaredTargets = [...(context.declaredTargets ?? []), objects[0]];
  if (effect.effect === "vote" && !effect.optional && !effect.forEach && !effect.forEachVote && !effect.decidedBy &&
      effect.starting?.player === "you" && effect.choices.choices === "named" &&
      effect.choices.names.length === 2 && effect.choices.names[0] !== effect.choices.names[1]) {
    const rules = ["701.38a", "701.38b", "701.38d"];
    context.namedVote = { path, event, names: effect.choices.names };
    this.node({ id: this.id(path, "named-vote"), path, node: "named-vote", scope: context.ability, event,
      start: context.you, names: effect.choices.names, rules });
  }
  const countedMove = effect.effect === "move-zone" && context.countedLinkedPool && objects.length === 1 &&
    objects[0].id === context.countedLinkedPool.id && isDeepStrictEqual(effect.to, { zone: "graveyard", owner: "its-owner" });
  if (countedMove) {
    const rules = ["400.7j", "608.2c", "608.2h"];
    const result = this.eventResult(path, event, "quantity", "one", "measured-quantity");
    this.node({ id: this.id(path, "actual-destination-count"), path, node: "actual-destination-count",
      scope: context.ability, event, result: result.id, destination: "graveyard", rules });
    this.engineFeature("actual-destination-count", path, "Count only the linked cards actually put into the stated graveyards.", rules);
    context.quantities = combine(context.quantities, [result]);
  }
  if (effect.effect === "damage" || effect.effect === "lose-life")
    context.measuredEvents = [...(context.measuredEvents ?? []), { operation: effect.effect, event }];
  const preEntryChoice = context.entryChoice?.path === path;
  if (effect.effect === "choose-value" && !context.barrier && players[0]?.cardinality === "one" &&
      (preEntryChoice || (context.resolutionChoice?.path === path && context.phase === "resolution"))) {
    this.node({ id: this.id(path, "value-choice"), path, node: "value-choice", scope: context.ability,
      event, chooser: players[0].id, category: effect.category,
      ...(effect.category === "card-type" && effect.within === "permanent-types" ? { within: effect.within } : {}),
      ...(effect.category === "type" ? { options: effect.options } : {}),
      lifetime: preEntryChoice ? { kind: "source-incarnation", source: context.source, lookup: "current-or-last-known" }
        : context.weightedMode ? { kind: "mode-occurrence", modal: context.weightedMode } : { kind: "resolution" },
      rules: preEntryChoice ? ENTRY_CHOICE_RULES : ["607.1c", "607.2d", "608.2d"] });
    if (!preEntryChoice) context.availableChoice = context.resolutionChoice;
  }
  if (effect.optional) context.decisions?.push(path);
  if (effect.effect === "vote") context.votes = [...(context.votes ?? []), event];
  if (objects.length || produces === "objects") context.mentionedObject = true;
  if (objects.some(object => object.id === context.source || this.sourceAliases.has(object.id))) context.sourceNamed = true;
  // Retain all distinct established participants. Replacing this set with
  // the most recently mentioned noun would silently guess an antecedent.
  if (objectPaths.length) context.objects = combine(context.objects, objects);
  if (playerPaths.length) context.players = combine(context.players, players.filter(player => player.id !== context.you));
  // "Search target player's library ... Then that player shuffles".
  context.players = combine(context.players, this.targetPlayersUnder(path, context));
  for (const [index, object] of objects.entries()) {
    const related = this.relationPlayer(object, child(path, `participant-${index}`), context);
    if (related) context.players = combine(context.players, [related]);
  }
  if (produces) {
    if (produces === "objects" && objects.length === 1 && identity !== "new-objects") cardinality = objects[0].cardinality;
    // "... for each Elf you control": how many objects result is counted on resolution.
    if (effect.forEach && produces === "objects") cardinality = "unknown";
    const perPlayer = players.some(player => player.cardinality !== "one");
    const aggregateTokens = perPlayer && effect.effect === "create-token" && !context.decider && !effect.decidedBy &&
      !effect.optional && !effect.forEach && !effect.forEachVote &&
      (effect.who?.player === "each-player" || effect.who?.player === "each-opponent") && effect.count.amount === "fixed";
    if (perPlayer) cardinality = aggregateTokens ? "many" : "unknown";
    // What the result is known to be: a card in its new zone, a permanent on
    // the battlefield, a token, or cards from a library (CR 110.1, 111.1).
    const destination = effect.effect === "move-zone" ? effect.to.zone : undefined;
    const established: ObjectFilter | undefined = identity === "zone-successors"
      ? { ...(objects[0]?.description ?? {}), cls: destination === "battlefield" ? "permanent" : "card" }
      : effect.effect === "manifest-dread" ? { cls: "permanent", types: ["creature"] }
      : effect.effect === "create-token" ? { cls: "token", ...("types" in effect.token && effect.token.types.length ? { types: effect.token.types as NonNullable<ObjectFilter["types"]> } : {}) }
      : ["look-at", "mill", "draw", "reveal"].includes(effect.effect) ? { cls: "card" } : undefined;
    const result = this.eventResult(path, event, produces, cardinality, identity, established);
    if (aggregateTokens) this.groupedTokenEvents.set(result.id, path);
    if (effect.effect === "exile") {
      if (!context.barrier && !context.decider && !effect.optional && !effect.faceDown &&
          effect.what.ref === "trigger-event-object" && effect.what.event === "cast" &&
          context.triggerObject?.event === "cast" && objects.length === 1 && objects[0].id === context.triggerObject.candidate.id)
        context.explicitExiledCast = { path, result };
      const fromPool = effect.what.ref === "from-pool" ? this.at(`${path}/what/pool`) : undefined;
      if (objects[0]?.description?.cls === "card" || fromPool?.description?.cls === "card")
        context.exileResults = [...(context.exileResults ?? []), { event, result, ...(effect.faceDown ? { faceDown: true as const } : {}) }];
      else context.uncertainExileOrigin = true;
    } else if (effect.effect === "exile-until" || effect.effect === "airbend" ||
        (effect.effect === "move-zone" && effect.to.zone === "exile")) context.uncertainExileOrigin = true;
    // Each player's action is its own event (CR 101.4); a later reference to
    // their results would need a per-player aggregate. Only such a reference
    // is unresolved; the instruction itself is not.
    if (perPlayer && !aggregateTokens) this.invalidNodes.set(result.id, "unsupported-scope");
    if (produces === "objects") {
      let successor = result;
      if (identity === "zone-successors") {
        // The successor stands for the moved object when the effect acts on
        // it again (CR 400.7j); asked about as it was, the moved object is
        // known by its last-known information (CR 608.2h).
        const moved = new Set(objects.map(object => object.id));
        if (context.sourceSuccessor && moved.has(context.sourceSuccessor.id) && moved.size === 1) context.sourceSuccessor = result;
        // A card that came from a library or hand has no information there
        // to look back at; the effect uses the moved card's (CR 608.2h).
        const hidden = effect.effect === "discard" || /"zone":"(library|hand)"|"top":/.test(JSON.stringify(this.result.nodes.find(node => node.id === objects[0]?.id) ?? {}));
        if (effect.effect === "discard") successor = { ...result, triggerSuccessor: true };
        if (!hidden) context.departed = combine(context.departed ?? [], objects.filter(object => object.domain === "object"));
        if (moved.size === 1 && !hidden) successor = { ...result, successorOf: objects[0].id };
        context.objects = context.objects.filter(object => !moved.has(object.id));
      }
      context.objects = combine(context.objects, [successor]);
    }
    else {
      if (echoes(effect)) this.echoQuantities.add(result.id);
      context.quantities = combine(context.quantities, [result]);
    }
  }
  // "That many" after an instruction is how many objects or counters it
  // affected ("Discard all the cards in your hand, then draw that many cards").
  if (!countedMove && (["draw", "mill", "create-token", "put-counters", "remove-counters", "lose-counters"].includes(effect.effect)
    || (["discard", "sacrifice", "destroy", "exile", "move-zone", "return-to-hand", "shuffle-into-library"].includes(effect.effect)
      && objects.some(object => object.cardinality !== "one")))) {
    const quantity = this.eventResult(path, event, "quantity", "one", "measured-quantity");
    if (echoes(effect)) this.echoQuantities.add(quantity.id);
    if (players.some(player => player.cardinality !== "one")) this.invalidNodes.set(quantity.id, "unsupported-scope");
    context.quantities = combine(context.quantities, [quantity]);
  }
  if (["exile", "exile-until", "move-zone", "return-to-hand", "destroy", "counter", "sacrifice", "discard", "airbend", "manifest", "cloak", "shuffle-into-library"].includes(effect.effect)) {
    if (objects.some(object => object.id === context.source)) context.sourceMoved = true;
    if (objects.some(object => object.id === context.sourceSuccessor?.id)) context.sourceSuccessor = undefined;
    else if (objects.some(object => object.id === context.source)) context.sourceSuccessor = undefined;
    if (identity !== "zone-successors") {
      // Hidden destinations and implicit/replaced zone moves need a
      // separate successor/LKI model. Never retain the old target as "it".
      const moved = new Set(objects.map(object => object.id));
      context.departed = combine(context.departed ?? [], context.objects.filter(object => moved.has(object.id)));
      context.objects = context.objects.filter(object => !moved.has(object.id));
    }
  }
}
