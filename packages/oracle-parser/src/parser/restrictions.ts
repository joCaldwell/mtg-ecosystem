// Activated-ability timing, usage limits, and explicit activation permissions.
// These constrain activation; they are not instructions executed on resolution.
// CR 602.2, 602.5b–e; https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt

import type { ActivatedAbility, ActivationRestriction, PhaseName, TriggeredAbility } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseCondition } from "./refs.ts";
import { wordNumber } from "../vocab.ts";

export type ActivationRules = Pick<ActivatedAbility, "restriction" | "permission">;
type TimingRestriction = Extract<ActivationRestriction, { restriction: "timing" }>;

function parseBefore(c: Cursor): TimingRestriction["before"] | null {
  return c.attempt(c => {
    if (c.word("before") === null) return null;
    if (c.words("attackers", "are", "declared")) return "declare-attackers";
    if (c.words("blockers", "are", "declared")) return "declare-blockers";
    if (c.words("the", "end", "step")) return "end-step";
    if (c.words("the", "end", "of", "combat", "step")) return "end-of-combat";
    return null;
  });
}

function parseTiming(c: Cursor, hasPermission: boolean): ActivationRestriction | null {
  return c.attempt(c => {
    const beforeOnly = parseBefore(c);
    if (beforeOnly) return { restriction: "timing", before: beforeOnly };
    if (c.word("during") === null) return null;
    let turn: TimingRestriction["turn"];
    if (c.word("your") !== null) turn = "your";
    else if (c.word("their") !== null) {
      if (!hasPermission) return null;
      turn = "activator";
    } else if (c.word("an") !== null) {
      if (c.possessive("opponent") === null) return null;
      turn = "opponent";
    } else c.word("any", "the");

    let phase: PhaseName | undefined;
    if (c.word("turn") !== null) {
      if (!turn) return null;
    } else if (c.word("upkeep") !== null) {
      phase = "upkeep";
      c.word("step");
    } else if (c.words("draw", "step")) phase = "draw-step";
    else if (c.words("declare", "attackers", "step")) phase = "declare-attackers";
    else if (c.words("declare", "blockers", "step")) phase = "declare-blockers";
    else if (c.words("combat", "damage", "step")) phase = "combat-damage";
    else if (c.words("end", "of", "combat", "step")) phase = "end-of-combat";
    else if (c.words("end", "step")) phase = "end-step";
    else if (c.words("precombat", "main", "phase") || c.words("first", "main", "phase")) phase = "precombat-main";
    else if (c.words("second", "main", "phase")) phase = "second-main";
    else if (c.words("postcombat", "main", "phase")) phase = "postcombat-main";
    else if (c.words("main", "phase")) phase = "main";
    else if (c.word("combat") !== null) {
      phase = "combat";
      c.word("phase");
    } else return null;

    const before = c.attempt(c => {
      c.punct(",");
      return parseBefore(c);
    });
    // "during combat after blockers are declared": the combat phase from the
    // declaration of blockers onward (CR 509.1).
    const after = !before && phase === "combat" && c.attempt(c => c.words("after", "blockers", "are", "declared") ? true : null) ? "declare-blockers" as const : undefined;
    return { restriction: "timing", ...(turn ? { turn } : {}), ...(phase ? { phase } : {}), ...(before ? { before } : {}), ...(after ? { after } : {}) };
  });
}

function parseClause(c: Cursor, hasPermission: boolean): ActivationRestriction | null {
  return c.attempt(c => {
    if (c.word("only") === null) return null;
    if (c.words("as", "a", "sorcery")) return { restriction: "sorcery" };
    if (c.words("as", "an", "instant")) return { restriction: "instant" };
    const limit = c.word("once", "twice");
    if (limit) {
      const eachTurn = c.words("each", "turn");
      if (limit === "once" && eachTurn) return { restriction: "once-each-turn" };
      return { restriction: "activation-limit", maximum: limit === "once" ? 1 : 2, period: eachTurn ? "turn" : "object" };
    }
    if (c.word("if") !== null) {
      const condition = parseCondition(c);
      return condition && { restriction: "condition", condition };
    }
    return parseTiming(c, hasPermission);
  });
}

/** "Activate no more than twice each turn" caps activations per turn exactly
 * as "Activate only twice each turn" does (CR 602.5b); a per-turn cap of one
 * keeps its established once-each-turn node. A cap computed from the game
 * state ("no more times each turn than the number of ...") is not a number. */
function parseMaximumClause(c: Cursor): ActivationRestriction | null {
  return c.attempt((c): ActivationRestriction | null => {
    if (!c.words("no", "more", "than")) return null;
    let maximum: number | undefined;
    if (c.word("once") !== null) maximum = 1;
    else if (c.word("twice") !== null) maximum = 2;
    else {
      const count = c.peek();
      maximum = count?.kind === "number" ? count.value : count?.kind === "word" && !count.possessive ? wordNumber(count.value) : undefined;
      if (maximum === undefined || !Number.isInteger(maximum) || maximum < 3) return null;
      c.pos++;
      if (c.word("times") === null) return null;
    }
    if (!c.words("each", "turn")) return null;
    return maximum === 1 ? { restriction: "once-each-turn" } : { restriction: "activation-limit", maximum, period: "turn" };
  });
}

function parsePermission(c: Cursor): ActivationRules["permission"] | null {
  return c.attempt(c => {
    if (c.words("any", "player", "may", "activate", "this", "ability"))
      return { activators: "any-player" };
    if (c.words("only", "your", "opponents", "may", "activate", "this", "ability"))
      return { activators: "opponents-of-source-controller" };
    return null;
  });
}

/** Consumes the complete post-resolution rule tail. Conjoined restrictions
 * remain conjunctions; a permission changes who may activate and never drops
 * timing or count restrictions stated alongside it. */
export function parseActivationRules(c: Cursor): ActivationRules | null {
  return c.attempt(c => {
    const restrictions: ActivationRestriction[] = [];
    let permission: ActivationRules["permission"];
    do {
      const allowed = parsePermission(c);
      if (allowed) {
        if (permission) return c.fail("single activation permission");
        permission = allowed;
      } else {
        if (c.word("activate") === null) return null;
        c.words("this", "ability");
        const restriction = parseMaximumClause(c) ?? parseClause(c, !!permission);
        if (!restriction) return null;
        restrictions.push(restriction);
      }
      // "only A, only B, and only C" lists conjoined restrictions; the comma
      // alone joins nothing unless another complete "only" clause follows.
      while (c.attempt(c => {
        const comma = c.punct(",");
        if (c.word("and", "but") === null && !(comma && c.isWord("only"))) return null;
        const restriction = parseClause(c, !!permission);
        if (!restriction) return null;
        restrictions.push(restriction);
        return true;
      })) { /* every restriction is retained */ }
      if (c.isWord("and", "but") || c.isPunct(",")) return null;
      if (!c.punct(".") && !c.done()) return null;
    } while (!c.done());
    const restriction: ActivationRestriction | undefined = restrictions.length === 0 ? undefined
      : restrictions.length === 1 ? restrictions[0] : { restriction: "all-of", restrictions };
    return { ...(restriction ? { restriction } : {}), ...(permission ? { permission } : {}) };
  });
}

/** CR 603.2h: "Do this only once each turn." closes a triggered ability whose
 * instruction its controller may decline. It limits taking the indicated
 * action, not how often the ability triggers. */
export function parseActionLimit(c: Cursor): TriggeredAbility["actionLimit"] | null {
  return c.attempt(c => {
    if (!c.words("do", "this", "only", "once", "each", "turn")) return null;
    c.punct(".");
    return c.done() ? { maximum: 1, period: "turn" } : null;
  });
}
