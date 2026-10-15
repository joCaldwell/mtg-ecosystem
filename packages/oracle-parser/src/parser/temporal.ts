// CR500.7-8,505.1b,724.1-2. No implicit owner/anchor inference.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Condition, Effect, PlayerRef, SkippableTime, StaticEffect, Trigger, TurnRef } from '../ast.ts';
import { Cursor } from './cursor.ts';
interface Grammar {
  actor(c: Cursor, verb: string, conjugated: string): PlayerRef | null;
  amount(c: Cursor): Amount | null;
}

/** These bounded headers belong to created delayed abilities. Ordinary
 * printed phase triggers do not strip their turn qualifier (CR603.7b).
 * Original tokens are consumed directly, preserving target provenance. */
export function parseTurnLinkedTrigger(c: Cursor): Trigger | null {
  return c.attempt(c => {
    if (!c.words("at", "the", "beginning", "of")) return null;
    if (c.words("that", "combat"))
      return { trigger: "phase-instance", phase: { phase: "that", kind: "combat", scope: "contextual" }, boundary: "beginning" };
    let turn: TurnRef;
    if (c.word("that") !== null) turn = { turn: "that", scope: "contextual" };
    else if (c.words("the", "next")) turn = { turn: "next" };
    else if (c.words("your", "next")) turn = { turn: "next", whose: { player: "you" } };
    else if (!c.pregame) return null;
    else {
      // Pregame delayed abilities (CR 103.6b): "the first upkeep" is the upkeep
      // of the game's first turn, "your first upkeep" that of your first turn.
      const first = c.attempt((c): Trigger | null => {
        const whose = c.words("the", "first") ? undefined : c.words("your", "first") ? { player: "you" as const } : null;
        if (whose === null) return null;
        const firstTurn: TurnRef = whose ? { turn: "first", whose } : { turn: "first" };
        if (c.word("upkeep") !== null) return { trigger: "phase-in-turn", phase: "upkeep", turn: firstTurn };
        // CR 505.1a: the first main phase of a turn is the precombat one.
        return whose && c.words("main", "phase", "of", "the", "game")
          ? { trigger: "phase-in-turn", phase: "precombat-main", turn: firstTurn } : null;
      });
      return first;
    }
    if (c.possessive("turn") === null) return null;
    const phase = c.word("upkeep") !== null ? "upkeep"
      : c.words("end", "step") ? "end-step"
      : c.words("draw", "step") ? "draw-step"
      : c.words("untap", "step") ? "untap-step" : null;
    return phase ? { trigger: "phase-in-turn", phase, turn } : null;
  });
}

function parseSkippedUnit(c: Cursor): SkippableTime | null {
  return c.attempt(c => {
    if (c.word("turn", "turns") !== null) return { kind: "turn" };
    const phase = c.word("main", "combat");
    if (phase === "main" || phase === "combat")
      return c.word("phase", "phases") !== null ? { kind: "phase", phase } : null;
    const step = c.word("untap", "upkeep", "draw");
    return (step === "untap" || step === "upkeep" || step === "draw") && c.word("step", "steps") !== null
      ? { kind: "step", step } : null;
  });
}

/** Possessives refer to the explicit skipping actor. Do not silently convert
 * an unrelated player's occurrence into the actor's own occurrence. */
function parseActorPossessive(c: Cursor, who: PlayerRef): boolean {
  return c.word(who.player === "you" ? "your" : "their") !== null;
}

function parseSkipEffect(c: Cursor, g: Grammar): Effect | null {
  return c.attempt(c => {
    const who = g.actor(c, "skip", "skips");
    if (!who) return null;
    if (c.word("all") !== null) {
      const what = parseSkippedUnit(c);
      if (!what || what.kind === "turn" || c.word("of") === null || !parseActorPossessive(c, who)
        || !c.words("next", "turn")) return null;
      return { effect: "skip-time", who, what, occurrences: { kind: "all" }, during: { turn: "next", whose: who } };
    }
    if (!parseActorPossessive(c, who) || c.word("next") === null) return null;
    const count = g.amount(c) ?? { amount: "fixed" as const, value: 1 };
    if (count.amount === "fixed" && count.value < 0) return null;
    const what = parseSkippedUnit(c);
    return what ? { effect: "skip-time", who, what, occurrences: { kind: "next", count } } : null;
  });
}

/** Indefinite skip instructions on permanents are recurring replacement
 * abilities, not spell instructions. Require the full clause, with an
 * optional continuous condition; `next` and explicit lifetimes do not fit. */
export function parseStaticSkip(c: Cursor, g: {
  player(c: Cursor): PlayerRef | null;
  condition(c: Cursor): Condition | null;
}): StaticEffect | null {
  return c.attempt(c => {
    let who: PlayerRef | null;
    if (c.word("skip") !== null) who = { player: "you" };
    else {
      who = c.word("players") !== null ? { player: "each-player" } : g.player(c);
      if (!who || c.word("skip", "skips") === null) return null;
    }
    if (!["you", "each-player", "each-opponent", "enchanted-player"].includes(who.player)
      || !parseActorPossessive(c, who)) return null;
    const what = parseSkippedUnit(c);
    if (!what) return null;
    const effect: StaticEffect = { static: "skip-time", who, what };
    const condition = c.word("if") !== null ? g.condition(c) : undefined;
    if (condition === null) return null;
    c.punct(".");
    if (!c.done()) return null;
    return condition ? { static: "conditional", condition, effect } : effect;
  });
}

export function parseCurrentPhaseCondition(c: Cursor): Condition | null {
  return c.attempt(c => {
    if (c.word("it's") === null && !c.words("it", "is")) return null;
    const negate = c.word("not") !== null;
    let whose: PlayerRef | undefined;
    if (c.word("your") !== null) whose = { player: "you" };
    else if (c.word("the") === null) return null;
    const ordinalWord = c.word("first", "second");
    const phase = c.word("main", "combat");
    if ((phase !== "main" && phase !== "combat") || c.word("phase") === null) return null;
    if (!whose) {
      if (c.word("of") === null) return null;
      if (c.word("your") !== null) whose = { player: "you" };
      else if (c.word("the") === null) return null;
      if (c.word("turn") === null) return null;
    }
    const condition: Condition = { condition: "current-phase", phase, ...(whose ? { whose } : {}),
      ...(ordinalWord ? { ordinal: ordinalWord === "first" ? 1 : 2 } : {}) };
    return negate ? { condition: "not", of: condition } : condition;
  });
}
type ExtraPhases = Extract<Effect, { effect: "add-phases" }>;

function parsePhaseAnchor(c: Cursor): ExtraPhases["after"] | null {
  return c.attempt(c => {
    if (c.word("this") !== null) {
      const kind = c.word("main", "combat");
      if (c.word("phase") === null) return null;
      return { phase: "current", ...(kind === "main" || kind === "combat" ? { kind } : {}) };
    }
    if (c.word("the") === null) return null;
    const ordinal = c.word("first", "second");
    const kind = c.word("main", "combat");
    if (!ordinal || (kind !== "main" && kind !== "combat") || !c.words("phase", "this", "turn")) return null;
    return { phase: "ordinal", kind, ordinal: ordinal === "first" ? 1 : 2, turn: "current" };
  });
}

function parseAdditionalPhase(c: Cursor, g: Grammar): ExtraPhases["sequence"][number] | null {
  return c.attempt(c => {
    const count = g.amount(c);
    if (!count || count.amount === "fixed" && count.value < 0 || c.word("additional") === null) return null;
    // CR 501.1: an added beginning phase has untap, upkeep and draw steps.
    const kind = c.word("beginning", "main", "combat");
    if ((kind !== "beginning" && kind !== "main" && kind !== "combat") || c.word("phase", "phases") === null) return null;
    return { phase: kind, count };
  });
}

export function parseTemporal(c: Cursor, g: Grammar): Effect[] | null {
  return c.attempt(c => {
    const skipped = parseSkipEffect(c, g);
    if (skipped) return [skipped];
    const turns = c.attempt((c): Effect | null => {
      const who = g.actor(c, "take", "takes");
      if (!who) return null;
      const count = g.amount(c);
      if (!count || count.amount === "fixed" && count.value < 0 || c.word("extra") === null
        || c.word("turn", "turns") === null || !c.words("after", "this", "one")) return null;
      return { effect: "add-turns", who, count, after: { turn: "current" } };
    });
    if (turns) return [turns];
    const added = c.attempt((c): Effect | null => {
      let after: ExtraPhases["after"] | null = null;
      if (c.word("after") !== null) {
        after = parsePhaseAnchor(c);
        if (!after || !c.punct(",")) return null;
      }
      if (!c.words("there", "is") && !c.words("there", "are") && c.word("there's") === null) return null;
      const first = parseAdditionalPhase(c, g);
      if (!first) return null;
      if (!after) {
        if (c.word("after") === null) return null;
        after = parsePhaseAnchor(c);
        if (!after) return null;
      }
      const sequence = [first];
      while (c.attempt(c => {
        c.punct(",");
        return c.words("followed", "by") ? true : null;
      })) {
        const next = parseAdditionalPhase(c, g);
        if (!next) return null;
        sequence.push(next);
      }
      return { effect: "add-phases", after, sequence };
    });
    if (added) return [added];
    if (c.words("end", "the", "turn")) return [{ effect: "end-turn" }];
    if (c.words("end", "the", "combat", "phase")) return [{ effect: "end-combat" }];
    return null;
  });
}
