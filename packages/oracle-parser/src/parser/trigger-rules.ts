// Static rules about whether, and how many times, abilities trigger.
// CR 603.2 (matching a trigger event), 603.2d (additional triggers).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Condition, ObjectFilter, StaticEffect, Trigger } from "../ast.ts";
import type { Token } from "../lexer.ts";
import { Cursor } from "./cursor.ts";
import { parseCondition, parseFilter, parseObjectRef } from "./refs.ts";
import { parseTrigger } from "./triggers.ts";

/** Gerund → finite verb (third person, second person). The causing event of
 * "a creature dying causes …" is the event of "whenever a creature dies". */
const GERUNDS: Record<string, [string, string]> = {
  entering: ["enters", "enter"], leaving: ["leaves", "leave"], dying: ["dies", "die"],
  attacking: ["attacks", "attack"], dealing: ["deals", "deal"], gaining: ["gains", "gain"],
  drawing: ["draws", "draw"], casting: ["casts", "cast"], copying: ["copies", "copy"],
};
const CAUSE_EVENTS = new Set(["enters", "leaves", "dies", "attacks", "is-dealt-damage", "deals-damage", "gains-life", "draws", "cast", "copy"]);

function isCauseEvent(trigger: Trigger): boolean {
  // Grouped, ordinal and turn-restricted events have no gerund wording here.
  if (trigger.grouping || trigger.occurrence || trigger.during || trigger.controller) return false;
  if (trigger.trigger === "any-of") return trigger.events.every(isCauseEvent);
  return CAUSE_EVENTS.has(trigger.trigger);
}

/** The event description uses exactly the trigger-event grammar, so the same
 * event is the same node as in "Whenever <event>, …". */
function parseCause(tokens: Token[]): Trigger | null {
  const first = tokens[0];
  const secondPerson = first?.kind === "word" && !first.possessive && first.value === "you";
  let verbs = 0;
  const finite = tokens.map((token): Token => {
    if (token.kind !== "word" || token.possessive) return token;
    const value = token.value === "being" ? "is" : GERUNDS[token.value]?.[secondPerson ? 1 : 0];
    if (!value) return token;
    verbs++;
    return { ...token, value, raw: value };
  });
  if (!verbs) return null;
  const c = new Cursor([{ kind: "word", value: "whenever", raw: "whenever", possessive: false }, ...finite]);
  const trigger = parseTrigger(c);
  return trigger && c.done() && isCauseEvent(trigger) ? trigger : null;
}

/** "a triggered ability of <source>", "abilities [of <sources>]". Every
 * ability that triggers is a triggered ability (CR 603.1); the printed
 * category word is retained only when printed. */
function parseAbilities(c: Cursor): ObjectFilter | null {
  return c.attempt((c): ObjectFilter | null => {
    c.word("a", "an");
    if (c.word("ability", "abilities") !== null) {
      if (c.word("of") === null) return { cls: "ability" };
      const abilitySource = parseObjectRef(c);
      return abilitySource && abilitySource.ref !== "target" ? { cls: "ability", abilitySource } : null;
    }
    const filter = parseFilter(c);
    if (filter?.cls !== "ability" || filter.abilityKind !== "triggered" || filter.anyOf || filter.manaAbility !== undefined || filter.loyaltyAbility) return null;
    return filter;
  });
}

function indexOfWords(tokens: Token[], ...words: string[]): number {
  return tokens.findIndex((_, index) => words.every((word, offset) => {
    const token = tokens[index + offset];
    return token?.kind === "word" && !token.possessive && token.value === word;
  }));
}

function parseAll<T>(tokens: Token[], parse: (c: Cursor) => T | null): T | null {
  const c = new Cursor(tokens);
  const result = parse(c);
  return result !== null && c.done() ? result : null;
}

/** "If <event> causes <abilities> to trigger, that ability triggers an
 * additional time." / "If <abilities> triggers [while <condition>], …" */
function parseAdditionalTriggers(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    if (c.word("if") === null) return null;
    const rest = c.tokens.slice(c.pos);
    // The consequence names the same ability; "it" has no other antecedent.
    const tail = (subject: string[]) => {
      const index = indexOfWords(rest, ...subject, "triggers", "an", "additional", "time");
      const comma = rest[index - 1];
      const period = rest[index + subject.length + 4];
      const end = index + subject.length + 4 + (period?.kind === "punct" && period.value === "." ? 1 : 0);
      return index > 0 && comma?.kind === "punct" && comma.value === "," && end === rest.length ? index - 1 : -1;
    };
    let end = tail(["that", "ability"]);
    if (end < 0) end = tail(["it"]);
    if (end < 0) return c.fail("additional-trigger rule");
    const clause = rest.slice(0, end);
    const causes = indexOfWords(clause, "causes");
    let effect: StaticEffect | null = null;
    if (causes > 0) {
      const cause = parseCause(clause.slice(0, causes));
      const abilities = parseAll(clause.slice(causes + 1), c => {
        const abilities = parseAbilities(c);
        return abilities && c.words("to", "trigger") ? abilities : null;
      });
      if (cause && abilities) effect = { static: "additional-triggers", abilities, cause, additional: 1 };
    } else {
      effect = parseAll(clause, (c): StaticEffect | null => {
        const abilities = parseAbilities(c);
        if (!abilities || c.word("triggers") === null) return null;
        let condition: Condition | null = null;
        if (c.word("while") !== null && !(condition = parseCondition(c))) return null;
        return { static: "additional-triggers", abilities, additional: 1, ...(condition ? { condition } : {}) };
      });
    }
    if (!effect) return c.fail("additional-trigger rule");
    c.pos = c.tokens.length;
    return effect;
  });
}

/** "<Objects> entering don't cause <abilities> to trigger." */
function parseTriggerSuppression(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    const rest = c.tokens.slice(c.pos);
    let split = indexOfWords(rest, "don't", "cause");
    if (split < 0) split = indexOfWords(rest, "doesn't", "cause");
    if (split <= 0) return null;
    const cause = parseCause(rest.slice(0, split));
    const abilities = parseAll(rest.slice(split + 2), c => {
      const abilities = parseAbilities(c);
      if (!abilities || !c.words("to", "trigger")) return null;
      c.punct(".");
      return abilities;
    });
    if (!cause || !abilities) return c.fail("trigger-suppression rule");
    c.pos = c.tokens.length;
    return { static: "trigger-suppression", abilities, cause };
  });
}

export function parseTriggerRule(c: Cursor): StaticEffect | null {
  return parseAdditionalTriggers(c) ?? parseTriggerSuppression(c);
}
