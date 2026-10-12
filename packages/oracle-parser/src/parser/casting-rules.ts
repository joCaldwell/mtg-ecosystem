// Rules about when and how often cards may be played, as opposed to the
// permissions in permissions.ts that say which cards may be played from where.
// CR 113.6e/601.3/506.8 (casting restrictions on the card itself), 305.2
// (number of land plays), 401.5–6 (playing with hidden cards revealed).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt

import type {
  Amount, CastTimingPoint, CastWindow, Condition, Duration, Effect, LandPlayAllowance, PlayerRef, StaticEffect,
} from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseAmount, parseCondition, parseDuration } from "./refs.ts";

// ---------------------------------------------------------------------------
// "Cast this spell only …"
// ---------------------------------------------------------------------------

function parseTurnOwner(c: Cursor): "your" | "opponent" | null {
  return c.attempt((c) => {
    if (c.word("your") !== null) return "your" as const;
    return c.word("an") !== null && c.possessive("opponent") !== null ? "opponent" as const : null;
  });
}

function parseTimingPoint(c: Cursor, window: CastWindow): CastTimingPoint | null {
  return c.attempt((c): CastTimingPoint | null => {
    if (c.words("attackers", "are", "declared")) return "attackers-declared";
    if (c.words("blockers", "are", "declared")) return "blockers-declared";
    if (c.words("the", "combat", "damage", "step")) return "combat-damage-step";
    if (c.word("combat") !== null) return "combat";
    // Reset: "their" is the opponent whose turn the same window names.
    if (window.turn === "opponent" && c.words("their", "upkeep", "step")) return "upkeep-step";
    return c.fail("casting-window boundary");
  });
}

function parseCastWindow(c: Cursor): CastWindow | null {
  return c.attempt((c): CastWindow | null => {
    let window: CastWindow = {};
    for (let parts = 0; ; parts++) {
      // Each part either extends a copy of the window or leaves it untouched.
      const next = c.attempt((c): CastWindow | null => {
        const draft: CastWindow = { ...window };
        const setTurn = (turn: "your" | "opponent"): boolean => !draft.turn && (draft.turn = turn, true);
        const setDuring = (during: NonNullable<CastWindow["during"]>): boolean => !draft.during && (draft.during = during, true);
        // Siren's Call separates its two requirements with a comma.
        if (parts > 0) c.punct(",");
        if (c.word("during") !== null) {
          if (c.word("combat") !== null) return setDuring("combat") ? draft : null;
          const step = c.attempt((c) => {
            const whose = c.word("the", "your");
            if (!whose || c.word("declare") === null) return null;
            const kind = c.word("attackers", "blockers");
            if (!kind || c.word("step") === null) return null;
            if (whose === "your" && !setTurn("your")) return null;
            return setDuring(kind === "attackers" ? "declare-attackers" : "declare-blockers") ? true : null;
          });
          if (step) return draft;
          const turn = parseTurnOwner(c);
          if (!turn || !setTurn(turn)) return null;
          if (c.word("turn") !== null) return draft;
          if (c.word("upkeep") !== null) return setDuring("upkeep") ? draft : null;
          if (c.words("end", "step")) return setDuring("end-step") ? draft : null;
          return null;
        }
        if (c.word("on") !== null) {
          // "during combat on your turn": a qualifier of a stated phase only.
          const turn = draft.during ? parseTurnOwner(c) : null;
          return turn && setTurn(turn) && c.word("turn") !== null ? draft : null;
        }
        const bound = c.word("before", "after") as "before" | "after" | null;
        if (!bound || draft[bound]) return null;
        const point = parseTimingPoint(c, draft);
        if (!point) return null;
        draft[bound] = point;
        return draft;
      });
      if (!next) return parts > 0 ? window : c.fail("casting window");
      window = next;
    }
  });
}

function parseCastCondition(c: Cursor): Condition | null {
  return c.attempt((c): Condition | null => {
    if (c.word("if") === null) return null;
    if (c.words("you've", "been", "attacked", "this", "step"))
      return { condition: "was-attacked", who: { player: "you" }, during: "this-step" };
    return parseCondition(c);
  });
}

/** A static rule of the card, not an instruction: the complete line must be
 * the restriction. Timing alternatives are joined by "or"; a following
 * "and only if" adds a conjunctive condition. */
export function parseCastRestriction(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    if (!c.words("cast", "this", "spell", "only")) return null;
    const what = { ref: "this" as const, noun: "spell" };
    let windows: CastWindow[] | undefined;
    let condition = parseCastCondition(c);
    if (!condition) {
      const first = parseCastWindow(c);
      if (!first) return null;
      windows = [first];
      for (;;) {
        const next = c.attempt(c => c.word("or") !== null ? parseCastWindow(c) : null);
        if (!next) break;
        windows.push(next);
      }
      if (c.words("and", "only")) {
        condition = parseCastCondition(c);
        if (!condition) return null;
      }
    }
    c.punct(".");
    if (!c.done()) return c.fail("complete casting restriction");
    return { static: "cast-restriction", what, ...(windows ? { windows } : {}), ...(condition ? { condition } : {}) };
  });
}

// ---------------------------------------------------------------------------
// Additional land plays
// ---------------------------------------------------------------------------

/** "Up to N additional lands" and "N additional lands" raise the same limit:
 * every land play is already optional (CR 305.2, 116.2a). Fastbond's "any
 * number of lands" removes the limit instead of adding to it. */
function parseLandAllowance(c: Cursor): Amount | "unlimited" | null {
  return c.attempt((c): Amount | "unlimited" | null => {
    if (c.words("any", "number", "of", "lands")) return "unlimited";
    c.words("up", "to");
    const count: Amount | null = c.word("an") !== null ? { amount: "fixed", value: 1 } : parseAmount(c);
    if (!count || c.word("additional") === null || c.word("land", "lands") === null) return null;
    return count;
  });
}

function parseLandPlayStatic(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    let who: PlayerRef;
    if (c.words("you", "may")) who = { player: "you" };
    else if (c.words("each", "player", "may")) who = { player: "each-player" };
    else return null;
    if (c.word("play") === null) return null;
    const additional = parseLandAllowance(c);
    if (!additional || c.word("on", "during") === null || !c.words("each", "of")) return null;
    if (c.word(who.player === "you" ? "your" : "their") === null || c.word("turns") === null) return null;
    c.punct(".");
    if (!c.done()) return c.fail("complete land-play allowance");
    return { static: "additional-land-plays", allowance: { who, additional } };
  });
}

/** Resolving form: requires its printed lifetime, so a permanent's static
 * wording can never be accepted as a one-shot instruction. */
export function parseLandPlayEffect(c: Cursor, options: { optionalConsumed?: boolean; leadingDuration?: Duration } = {}): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    if (!options.optionalConsumed && !c.words("you", "may")) return null;
    if (c.word("play") === null) return null;
    const additional = parseLandAllowance(c);
    if (!additional) return null;
    const duration = options.leadingDuration ?? parseDuration(c);
    if (!duration || duration.duration === "as-long-as") return null;
    const allowance: LandPlayAllowance = { who: { player: "you" }, additional };
    return [{ effect: "additional-land-plays", allowance, duration }];
  });
}

// ---------------------------------------------------------------------------
// "Play with … revealed"
// ---------------------------------------------------------------------------

function parsePlayRevealed(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    let who: PlayerRef;
    if (c.word("play") !== null) who = { player: "you" };
    else if (c.words("players", "play")) who = { player: "each-player" };
    else if (c.words("your", "opponents", "play")) who = { player: "each-opponent" };
    else return null;
    if (c.word("with") === null) return null;
    const whose = who.player === "you" ? "your" : "their";
    let what: "library-top" | "hand";
    if (c.words("the", "top", "card", "of", whose)) {
      if (c.word(...(whose === "your" ? ["library"] : ["library", "libraries"])) === null) return null;
      what = "library-top";
    } else if (c.word(whose) !== null && c.word(...(whose === "your" ? ["hand"] : ["hand", "hands"])) !== null) what = "hand";
    else return null;
    if (c.word("revealed") === null) return null;
    c.punct(".");
    if (!c.done()) return c.fail("complete play-revealed rule");
    return { static: "play-revealed", who, what };
  });
}

/** Whole-line static rules of this module. */
export function parseCastingRule(c: Cursor): StaticEffect | null {
  return parseCastRestriction(c) ?? parseLandPlayStatic(c) ?? parsePlayRevealed(c);
}
