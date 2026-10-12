// Conditions about players: one quantity compared between two players, what
// a player did earlier in a period, and the collective "your opponents".
// CR 608.2i (looking back), 700.13 (crimes), 305 (land plays), 121 (draws):
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Comparison, Condition, ObjectFilter, PlayerHistoryAction, PlayerQuantity, PlayerRef } from "../ast.ts";
import type { ConditionGrammar } from "./conditions.ts";
import { Cursor } from "./cursor.ts";

const atLeastOne: Comparison = { op: "ge", value: { amount: "fixed", value: 1 } };
const negated = (of: Condition, negate: boolean): Condition => negate ? { condition: "not", of } : of;

/** The compared players supply the controller, and history asks about the
 * object as the action happened; neither may carry its own scope. */
function hasScope(filter: ObjectFilter): boolean {
  return !!(filter.zone || filter.control || filter.controlledBy || filter.own || filter.fromAmong || filter.relation)
    || !!filter.allOf?.some(hasScope) || !!filter.anyOf?.some(hasScope);
}

/** "more lands than": the counted kind ends at the comparison word. */
function parseBoundedFilter(c: Cursor, g: ConditionGrammar, end: string): ObjectFilter | null {
  return c.attempt(c => {
    let offset = 0;
    for (;; offset++) {
      const token = c.peek(offset);
      if (!token || token.kind === "punct") return null;
      if (token.kind === "word" && !token.possessive && token.value === end) break;
    }
    if (offset === 0) return null;
    const bounded = new Cursor(c.tokens.slice(c.pos, c.pos + offset));
    const filter = g.parseFilter(bounded);
    if (!filter || !bounded.done() || hasScope(filter)) return null;
    c.pos += offset + 1;
    return filter;
  });
}

/** Both operands stay explicit with their own quantifiers. "Each other
 * player" excludes the particular player being tested as the subject. */
function parsePlayerComparison(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt((c): Condition | null => {
    const who = g.parsePlayerRef(c);
    if (!who) return null;
    let quantity: PlayerQuantity;
    let op: "gt" | "lt";
    if (c.word("control", "controls") !== null) {
      const word = c.word("more", "fewer");
      if (!word) return null;
      op = word === "more" ? "gt" : "lt";
      const of = parseBoundedFilter(c, g, "than");
      if (!of) return null;
      quantity = { quantity: "controlled-count", of };
    } else if (c.word("have", "has") !== null) {
      const word = c.word("more", "fewer", "less");
      if (!word) return null;
      op = word === "more" ? "gt" : "lt";
      if (word !== "less" && c.words("cards", "in", "hand")) quantity = { quantity: "cards-in-hand" };
      else if (word !== "fewer" && c.word("life") !== null) quantity = { quantity: "life-total" };
      else return null;
      if (c.word("than") === null) return null;
    } else return null;
    const than: PlayerRef | { player: "each-other-player" } | null = c.words("each", "other", "player")
      ? { player: "each-other-player" } : g.parsePlayerRef(c);
    return than && { condition: "player-comparison", who, quantity, op, than };
  });
}

/** "Your opponents control" counts their permanents together, exactly like
 * "permanents your opponents control"; it is not a test of each opponent. */
function parseOpponentsControl(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt((c): Condition | null => {
    if (!c.words("your", "opponents", "control")) return null;
    const explicit = c.isWord("no", "exactly");
    const comparison: Comparison | null = c.word("no") !== null
      ? { op: "eq", value: { amount: "fixed", value: 0 } } : g.parseComparison(c);
    if (!comparison || (comparison.op === "eq" && !explicit)) return null;
    const of = g.parseFilter(c);
    if (!of || hasScope(of)) return null;
    return { condition: "count", of: { ...of, control: "opponent" }, comparison };
  });
}

type Tense = "past" | "perfect" | "base";
const VERBS: Record<string, { action: PlayerHistoryAction["action"]; tense: Tense[] }> = {
  sacrificed: { action: "sacrifice", tense: ["past", "perfect"] }, sacrifice: { action: "sacrifice", tense: ["base"] },
  discarded: { action: "discard", tense: ["past", "perfect"] }, discard: { action: "discard", tense: ["base"] },
  created: { action: "create-token", tense: ["past", "perfect"] }, create: { action: "create-token", tense: ["base"] },
  cycled: { action: "cycle", tense: ["past", "perfect"] }, cycle: { action: "cycle", tense: ["base"] },
  drew: { action: "draw", tense: ["past"] }, drawn: { action: "draw", tense: ["perfect"] }, draw: { action: "draw", tense: ["base"] },
  played: { action: "play-land", tense: ["past", "perfect"] }, play: { action: "play-land", tense: ["base"] },
  committed: { action: "commit-crime", tense: ["past", "perfect"] }, commit: { action: "commit-crime", tense: ["base"] },
  scried: { action: "scry", tense: ["past", "perfect"] }, scry: { action: "scry", tense: ["base"] },
  surveilled: { action: "surveil", tense: ["past", "perfect"] }, surveil: { action: "surveil", tense: ["base"] },
};

function parseVerb(c: Cursor, tense: Tense): PlayerHistoryAction["action"] | null {
  return c.attempt(c => {
    const word = c.anyWord();
    const verb = word && VERBS[word.value];
    return verb && verb.tense.includes(tense) ? verb.action : null;
  });
}

/** "A Food" is one or more; a bare plural number is not given a bound. */
function parseCounted(c: Cursor, g: ConditionGrammar): Comparison | null {
  if (c.word("a", "an") !== null) return atLeastOne;
  const explicit = c.isWord("exactly");
  const comparison = g.parseComparison(c);
  return comparison && (comparison.op !== "eq" || explicit) ? comparison : null;
}

function parsePlayerActionHistory(c: Cursor, g: ConditionGrammar): Condition | null {
  return c.attempt((c): Condition | null => {
    let who: PlayerRef | null;
    let tense: Tense = "past";
    let negate = false;
    if (c.word("you've") !== null) { who = { player: "you" }; tense = "perfect"; }
    else {
      who = g.parsePlayerRef(c);
      if (!who) return null;
      if (c.word("didn't") !== null) { tense = "base"; negate = true; }
      else if (c.word("haven't", "hasn't") !== null) { tense = "perfect"; negate = true; }
      else if (c.word("have", "has") !== null) tense = "perfect";
    }
    const action = parseVerb(c, tense);
    if (!action) return null;
    const history = (item: PlayerHistoryAction, comparison: Comparison): Condition | null => {
      const during = c.words("this", "turn") ? "this-turn" : c.words("last", "turn") ? "last-turn" : null;
      return during && { condition: "player-action-history", who: who!, action: item, comparison, during };
    };
    let result: Condition | null;
    if (action === "scry" || action === "surveil") {
      // "Scried or surveilled" names two alternative actions of one player.
      const other = c.attempt(c => c.word("or") !== null ? parseVerb(c, tense) : null);
      if (other && (other === action || (other !== "scry" && other !== "surveil"))) return null;
      const first = history({ action }, atLeastOne);
      if (!first) return null;
      result = other && first.condition === "player-action-history"
        ? { condition: "any-of", conditions: [first, { ...first, action: { action: other } }] } : first;
    } else if (action === "commit-crime") {
      result = c.words("a", "crime") ? history({ action }, atLeastOne) : null;
    } else if (action === "play-land") {
      result = c.words("a", "land") ? history({ action }, atLeastOne) : null;
    } else if (action === "draw") {
      const comparison = parseCounted(c, g);
      result = comparison && c.word("card", "cards") !== null ? history({ action }, comparison) : null;
    } else {
      const comparison = parseCounted(c, g);
      const what = comparison && g.parseFilter(c);
      if (!comparison || !what || hasScope(what)) return null;
      // Cards are discarded or cycled and tokens created; other nouns here
      // would describe some different event.
      if ((action === "discard" || action === "cycle") && what.cls !== "card") return null;
      if (action === "create-token" && what.cls !== "token") return null;
      if (action === "sacrifice" && (what.cls === "card" || what.cls === "spell" || what.cls === "ability")) return null;
      result = history({ action, what }, comparison);
    }
    return result && negated(result, negate);
  });
}

export function parsePlayerCondition(c: Cursor, g: ConditionGrammar): Condition | null {
  return parsePlayerComparison(c, g) ?? parseOpponentsControl(c, g) ?? parsePlayerActionHistory(c, g);
}
