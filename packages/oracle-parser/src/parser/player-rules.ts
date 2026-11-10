// Rules of the game changed for particular players: maximum hand size
// (CR 402.2), life-total prohibitions (119.7–119.8), winning and losing
// (104.2–104.3, 810.8a), and unspent mana (500.5). These are continuous
// effects on game rules (613.11), never one-shot instructions.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Color, Duration, Effect, PlayerRef, PlayerRule, StaticEffect } from "../ast.ts";
import { COLORS } from "../vocab.ts";
import type { Cursor } from "./cursor.ts";
import { parseAmount, parseDuration, parseEqualTo, parsePlayerRef, parsePossessivePlayer } from "./refs.ts";

type RuleBody = PlayerRule extends infer R ? R extends { who: PlayerRef } ? Omit<R, "who"> : never : never;

/** Bare plurals state the rule for every such player. */
function parseSubject(c: Cursor): PlayerRef | null {
  return c.attempt((c): PlayerRef | null => {
    if (c.word("players") !== null) return { player: "each-player" };
    if (c.words("your", "opponents") || c.word("opponents") !== null) return { player: "each-opponent" };
    return parsePlayerRef(c);
  });
}

/** "<player>'s maximum hand size is …": a stated number replaces the normal
 * seven; "reduced by" / "increased by" adjust whatever it currently is. */
function parseHandSizeStatement(c: Cursor): PlayerRule[] | null {
  return c.attempt((c): PlayerRule[] | null => {
    const who = parsePossessivePlayer(c);
    if (!who || !c.words("maximum", "hand", "size", "is")) return null;
    const direction = c.words("reduced", "by") ? "reduce" as const : c.words("increased", "by") ? "increase" as const : null;
    if (direction) {
      const amount = parseAmount(c);
      return amount ? [{ who, rule: "maximum-hand-size", change: { operation: direction, amount } }] : null;
    }
    const value = parseEqualTo(c) ?? parseAmount(c);
    return value ? [{ who, rule: "maximum-hand-size", change: { operation: "set", value } }] : null;
  });
}

/** "<player>'s life total can't change": the player can neither gain nor lose
 * life. Setting a life total is gaining or losing the difference (CR 119.5),
 * and exchanges or costs that would raise or lower it are stopped by
 * 119.7 and 119.8 together. */
function parseLifeTotalStatement(c: Cursor): PlayerRule[] | null {
  return c.attempt((c): PlayerRule[] | null => {
    const who = parsePossessivePlayer(c);
    if (!who || !c.words("life", "total", "can't", "change")) return null;
    return [{ who, rule: "cant-gain-life" }, { who, rule: "cant-lose-life" }];
  });
}

/** The verb phrase after "can't" that names one action. */
function parseSimpleProhibition(c: Cursor): RuleBody | null {
  return c.attempt((c): RuleBody | null => {
    if (c.words("gain", "life")) return { rule: "cant-gain-life" };
    if (c.words("lose", "life")) return { rule: "cant-lose-life" };
    if (c.words("draw", "cards")) return { rule: "cant-draw-cards" };
    if (c.words("search", "libraries")) return { rule: "cant-search-libraries" };
    if (c.words("cycle", "cards")) return { rule: "cant-cycle-cards" };
    return null;
  });
}

/** The predicate after a player subject. "Can't lose the game or win the
 * game" prohibits both outcomes (the negation covers each alternative). */
function parsePredicate(c: Cursor): RuleBody[] | null {
  return c.attempt((c): RuleBody[] | null => {
    if (c.word("have", "has") !== null)
      return c.words("no", "maximum", "hand", "size") ? [{ rule: "maximum-hand-size", change: { operation: "none" } }] : null;
    if (c.word("can't") !== null) {
      const simple = parseSimpleProhibition(c);
      if (simple) {
        // "Players can't draw cards or gain life": the negation covers each alternative.
        const bodies = [simple];
        for (;;) {
          const next = c.attempt(c => c.word("or") !== null ? parseSimpleProhibition(c) : null);
          if (!next) break;
          bodies.push(next);
        }
        return bodies;
      }
      const first = c.word("win", "lose");
      if (!first || !c.words("the", "game")) return null;
      const outcome = (verb: string): RuleBody => ({ rule: verb === "win" ? "cant-win-game" : "cant-lose-game" });
      const second = c.attempt(c => {
        if (c.word("or") === null) return null;
        const verb = c.word("win", "lose");
        return verb && verb !== first && c.words("the", "game") ? verb : null;
      });
      return second ? [outcome(first), outcome(second)] : [outcome(first)];
    }
    if (c.word("don't", "doesn't") === null || c.word("lose") === null) return null;
    if (c.words("the", "game", "for", "having")) {
      // CR 704.5a: exactly the "0 or less life" state-based loss.
      return c.number() === 0 && c.words("or", "less", "life") ? [{ rule: "no-loss-at-zero-life" }] : null;
    }
    if (c.word("unspent") === null) return null;
    const color = c.peek();
    let mana: "any" | Color = "any";
    if (color?.kind === "word" && !color.possessive && COLORS.has(color.value)) { mana = color.value as Color; c.pos++; }
    // CR 500.5: mana empties as each step and each phase ends; both are named.
    return c.words("mana", "as", "steps", "and", "phases", "end") ? [{ rule: "keep-unspent-mana", mana }] : null;
  });
}

/** One clause: a possessive hand-size statement, or a player with a
 * predicate. `elided` supplies the subject of a coordinated bare predicate. */
function parseClause(c: Cursor, elided?: PlayerRef): PlayerRule[] | null {
  return parseHandSizeStatement(c) ?? parseLifeTotalStatement(c) ?? c.attempt((c): PlayerRule[] | null => {
    const who = parseSubject(c) ?? elided;
    if (!who) return null;
    const bodies = parsePredicate(c);
    return bodies && bodies.map(body => ({ who, ...body }) as PlayerRule);
  });
}

/** Static abilities name the affected players themselves; a pronoun or an
 * arbitrary single player has no antecedent in a static ability, and static
 * abilities never target (CR 115.1, 604.1). */
function hasStaticSubject(rule: PlayerRule): boolean {
  const who = rule.who.player === "filtered" ? rule.who.base : rule.who;
  return ["you", "each-player", "each-opponent", "enchanted-player", "chosen-player"].includes(who.player);
}

/** "You can't lose the game and your opponents can't win the game."
 * Coordinated clauses are separate rules; a bare second predicate keeps the
 * first clause's player. The whole ability must be consumed. */
export function parsePlayerRuleStatic(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    const rules = parseClause(c);
    if (!rules) return null;
    for (;;) {
      const next = c.attempt(c => {
        c.punct(",");
        return c.word("and") !== null ? parseClause(c, rules[rules.length - 1].who) : null;
      });
      if (!next) break;
      rules.push(...next);
    }
    if (!rules.every(hasStaticSubject)) return c.fail("player named by a static rule");
    c.punct(".");
    if (!c.done() && !c.isWord("otherwise")) return null;
    const effects = rules.map((rule): StaticEffect => ({ static: "player-rule", rule }));
    return effects.length === 1 ? effects[0] : { static: "group", effects };
  });
}

/** The resolving form: one clause with its stated lifetime (CR 611.2a). A
 * rule printed without one is a static ability, not an instruction, so a
 * duration is required here, either trailing or leading the sentence. */
export function parsePlayerRuleEffect(c: Cursor, leadingDuration?: Duration): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    const rules = parseClause(c);
    if (!rules) return null;
    const duration = leadingDuration ?? parseDuration(c);
    if (!duration) return c.fail("duration of a resolving player rule");
    return rules.map((rule): Effect => ({ effect: "player-rule", rule, duration }));
  });
}
