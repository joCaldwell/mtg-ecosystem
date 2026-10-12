// Designations and defined states used as conditions. The actions that
// confer them (becoming the monarch, taking the initiative, it becoming
// day) are instructions and are parsed elsewhere.
// CR 102.1, 122.1f, 309.7, 700.8c, 702.62b, 702.112b, 702.131c, 725, 726, 731:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Condition, ObjectRef, PlayerDesignation, PlayerRef } from "../ast.ts";
import type { ConditionGrammar } from "./conditions.ts";
import { Cursor } from "./cursor.ts";

const negated = (of: Condition, negate: boolean): Condition => negate ? { condition: "not", of } : of;

/** "you're", "you aren't", "an opponent is not": subject plus copula. */
function parsePlayerCopula(c: Cursor, g: ConditionGrammar): { who: PlayerRef; negate: boolean } | null {
  return c.attempt(c => {
    if (c.word("you're") !== null) return { who: { player: "you" }, negate: c.word("not") !== null };
    const who = g.parsePlayerRef(c);
    if (!who) return null;
    if (c.word("isn't", "aren't") !== null) return { who, negate: true };
    if (c.word("is", "are") === null) return null;
    return { who, negate: c.word("not") !== null };
  });
}

function parsePlayerHas(c: Cursor, g: ConditionGrammar): { who: PlayerRef; negate: boolean } | null {
  return c.attempt(c => {
    const who = g.parsePlayerRef(c);
    if (!who) return null;
    if (c.word("don't", "doesn't") !== null) return c.word("have") !== null ? { who, negate: true } : null;
    return c.word("have", "has") !== null ? { who, negate: false } : null;
  });
}

function parseDesignation(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt((c): Condition | null => {
    // CR 725.1: no player is the monarch until an effect creates one.
    if (c.words("there", "is", "no", "monarch"))
      return { condition: "not", of: { condition: "designation", who: { player: "a-player" }, designation: "monarch" } };
    const copula = parsePlayerCopula(c, g);
    if (copula) {
      if (!c.words("the", "monarch")) return null;
      return negated({ condition: "designation", who: copula.who, designation: "monarch" }, copula.negate);
    }
    const has = parsePlayerHas(c, g);
    if (!has || c.word("the") === null) return null;
    let designation: PlayerDesignation;
    if (c.word("initiative") !== null) designation = "initiative";
    else if (c.possessive("city") !== null && c.word("blessing") !== null) designation = "city's-blessing";
    else return null;
    return negated({ condition: "designation", who: has.who, designation }, has.negate);
  });
}

function parseDayNight(c: Cursor): Condition | null {
  return c.attempt((c): Condition | null => {
    if (c.word("it's") === null && !c.words("it", "is")) return null;
    if (c.words("neither", "day", "nor", "night")) return { condition: "day-night", state: "neither" };
    const state = c.word("day", "night");
    return state === "day" || state === "night" ? { condition: "day-night", state } : null;
  });
}

function parseDungeonOrParty(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt((c): Condition | null => {
    let who: PlayerRef | null;
    let negate = false;
    let perfect = false;
    if (c.word("you've") !== null) { who = { player: "you" }; perfect = true; }
    else {
      who = g.parsePlayerRef(c);
      if (!who) return null;
      if (c.word("haven't", "hasn't") !== null) { negate = true; perfect = true; }
      else if (c.word("have", "has") === null) return null;
    }
    if (c.words("completed", "a", "dungeon")) return negated({ condition: "completed-dungeon", who }, negate);
    // "Have" is possession here, not a perfect auxiliary.
    if (!perfect && c.words("a", "full", "party")) return { condition: "full-party", who };
    return null;
  });
}

/** "It's not their turn" names the active player through a possessive. The
 * controller's own turn keeps its established your-turn nodes. */
function parseTurnOf(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt((c): Condition | null => {
    let negate = false;
    if (c.word("it's") !== null) negate = c.word("not") !== null;
    else if (c.words("it", "isn't")) negate = true;
    else if (c.words("it", "is")) negate = c.word("not") !== null;
    else return null;
    const who = g.parsePossessivePlayer?.(c) ?? null;
    if (!who || who.player === "you" || c.word("turn") === null) return null;
    return negated({ condition: "turn-of", who }, negate);
  });
}

/** CR 122.1f: poisoned is exactly having one or more poison counters. */
function parsePoisoned(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt((c): Condition | null => {
    const copula = parsePlayerCopula(c, g);
    if (!copula || c.word("poisoned") === null) return null;
    return negated({ condition: "player-counters", who: copula.who, counter: { type: "poison" },
      comparison: { op: "ge", value: { amount: "fixed", value: 1 } } }, copula.negate);
  });
}

/** Renowned (CR 702.112b) is a designation of a permanent; suspended
 * (702.62b) is the defined state of an exiled card with suspend and a time
 * counter. Neither is a characteristic or a keyword the object has. */
function parseObjectDesignation(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt((c): Condition | null => {
    let what: ObjectRef | null;
    let negate = false;
    if (c.word("it's") !== null) { what = { ref: "it" }; negate = c.word("not") !== null; }
    else {
      what = g.parseObjectRef(c);
      if (!what || !["self", "this", "it", "that", "enchanted", "equipped"].includes(what.ref)) return null;
      if (what.ref === "that" && what.plural) return null;
      if (c.word("isn't") !== null) negate = true;
      else if (c.word("is") !== null) negate = c.word("not") !== null;
      else return null;
    }
    const status = c.word("renowned", "suspended");
    return status ? negated({ condition: "status", what, status }, negate) : null;
  });
}

export function parseDesignationCondition(c: Cursor, g: ConditionGrammar): Condition | null {
  return parseDesignation(c, g) ?? parseDayNight(c) ?? parseDungeonOrParty(c, g) ?? parseTurnOf(c, g)
    ?? parsePoisoned(c, g) ?? parseObjectDesignation(c, g);
}
