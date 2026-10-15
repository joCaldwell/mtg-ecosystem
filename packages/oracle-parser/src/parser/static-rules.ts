// Static abilities that state a rule about one event or procedure instead of
// modifying characteristics: what happens as a permanent enters, what a
// permanent can be attached to, and how the untap step treats permanents.
// CR 301.5, 303.4, 502.3, 614.1c, 614.12, 731.1:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { ObjectRef, PlayerRef, StaticEffect } from "../ast.ts";
import type { Token } from "../lexer.ts";
import { Cursor } from "./cursor.ts";
import { parseGainedAbilities, parseSentences } from "./effects.ts";
import { parseCastScope } from "./cost-modifiers.ts";
import { hasFreshProtectionChoice } from "./protection.ts";
import { parseCount, parseFilter, parseObjectRef } from "./refs.ts";

function isWord(token: Token | undefined, ...values: string[]): boolean {
  return token?.kind === "word" && !token.possessive && values.includes(token.value);
}

/** "If it's neither day nor night, it becomes day as this creature enters."
 * The designation changes while the permanent is entering (CR 614.1c,
 * 731.1), exactly like "As ~ enters, …"; it is neither a trigger nor an
 * instruction that resolves. Only day/night changes are known to be worded
 * with the entering clause last. */
function parseTrailingAsEnters(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    const rest = c.tokens.slice(c.pos);
    let split = -1;
    for (let index = rest.length - 1; index > 0; index--) if (isWord(rest[index], "as")) { split = index; break; }
    if (split < 1) return null;
    const tail = new Cursor(rest.slice(split + 1));
    const what: ObjectRef | null = parseObjectRef(tail);
    if (!what || (what.ref !== "self" && what.ref !== "this") || tail.word("enters") === null) return null;
    tail.words("the", "battlefield");
    tail.punct(".");
    if (!tail.done()) return null;
    const body = new Cursor(rest.slice(0, split));
    const effects = parseSentences(body, { modalOptions: [] });
    if (!effects || !body.done() || effects.length !== 1) return null;
    if (!effects[0].steps.every(step => step.effect === "set-day-night" && !step.optional && !step.duration) || effects[0].otherwise)
      return c.fail("day/night change as the permanent enters");
    c.pos = c.tokens.length;
    return { static: "as-enters", what, effects };
  });
}

/** "Untap all creatures you control during each other player's untap step."
 * The affected permanents are this ability's controller's (or the source
 * itself), so the players contrasted by "other" are every player but you. */
function parseUntapDuringStep(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    if (c.word("untap") === null) return null;
    const what = parseObjectRef(c);
    if (!what || !["all", "each", "this", "self"].includes(what.ref) || c.word("during") === null) return null;
    if (("filter" in what) && what.filter.control !== "you") return c.fail("permanents you control");
    let owner: PlayerRef;
    if (c.word("each") === null) return null;
    if (c.word("other") !== null) {
      if (c.possessive("player") === null) return null;
      owner = { player: "filtered", base: { player: "each-player" }, filter: { otherThan: { player: "you" } } };
    } else if (c.possessive("opponent") !== null) owner = { player: "each-opponent" };
    else return null;
    if (!c.words("untap", "step")) return null;
    c.punct(".");
    if (!c.done()) return null;
    return { static: "untap-during-step", what, step: { occurrence: "each", owner } };
  });
}

/** "Players can't untap more than one artifact during their untap steps."
 * The possessive must name the limited players' own untap steps. */
function parseUntapStepLimit(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    let who: PlayerRef;
    let own: string[];
    if (c.word("players") !== null) { who = { player: "each-player" }; own = ["their", "untap", "steps"]; }
    else if (c.words("your", "opponents")) { who = { player: "each-opponent" }; own = ["their", "untap", "steps"]; }
    else if (c.word("you") !== null) { who = { player: "you" }; own = ["your", "untap", "step"]; }
    else return null;
    if (!c.words("can't", "untap", "more", "than")) return null;
    const maximum = parseCount(c);
    const what = maximum && parseFilter(c);
    if (!maximum || !what || what.control || what.controlledBy || what.zone) return null;
    if (!c.words("during", ...own)) return null;
    c.punct(".");
    if (!c.done() && !c.isWord("otherwise")) return null;
    return { static: "untap-step-limit", who, what, maximum };
  });
}

/** "The first creature spell you cast each turn has convoke." The ordinal
 * counts that player's matching casts of the turn, exactly as for the cost
 * changes worded the same way; only ordinal grants are parsed here. */
function parseOrdinalSpellGrant(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    if (!c.isWord("the")) return null;
    const rest = c.tokens.slice(c.pos);
    const verb = rest.findIndex(token => isWord(token, "has") || (token.kind === "punct" && token.value === '"'));
    if (verb < 3 || !isWord(rest[verb], "has")) return null;
    const scope = parseCastScope(rest.slice(0, verb));
    if (!scope || !scope.occurrence || !("ordinal" in scope.occurrence) || !scope.caster) return null;
    const body = new Cursor(rest.slice(verb + 1));
    const abilities = parseGainedAbilities(body);
    if (!abilities || hasFreshProtectionChoice(abilities)) return null;
    body.punct(".");
    if (!body.done()) return null;
    c.pos = c.tokens.length;
    return { static: "cast-spell-abilities", scope, abilities };
  });
}

/** "This Equipment can be attached only to a creature with power 3 or
 * greater.": a limit on what the source can legally be attached to (CR
 * 301.5, 303.4c, 701.3b), not an equip restriction or a targeting rule. */
function parseAttachmentLimit(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    const what = parseObjectRef(c);
    if (!what || (what.ref !== "self" && what.ref !== "this")) return null;
    if (!c.words("can", "be", "attached", "only", "to") || !c.isWord("a", "an")) return null;
    const to = parseObjectRef(c);
    if (!to || to.ref !== "filter" || to.upTo || to.groupConstraints || to.random
      || to.count.amount !== "fixed" || to.count.value !== 1) return null;
    c.punct(".");
    return c.done() ? { static: "attachment-limit", what, to: to.filter } : null;
  });
}

export function parseStaticRule(c: Cursor): StaticEffect | null {
  return parseTrailingAsEnters(c) ?? parseUntapDuringStep(c) ?? parseUntapStepLimit(c) ?? parseOrdinalSpellGrant(c) ?? parseAttachmentLimit(c);
}
