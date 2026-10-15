// "Enchanted creature gets -2/-2. It gets -5/-5 instead as long as you've
// completed a dungeon." The second sentence states the modification the
// same static ability applies while the condition holds, in place of the
// first one (not in addition to it). CR 611.3a: a static ability's effect
// applies at any given time to whatever its text indicates at that time.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Condition, ObjectRef, StaticEffect, StaticModifier } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseCondition } from "./refs.ts";

type ModifierParser = (c: Cursor, what: ObjectRef) => { modifiers: StaticModifier[]; condition?: Condition } | null;

const SINGLE_SUBJECTS = new Set(["self", "this", "enchanted", "equipped"]);

type StaticParser = (c: Cursor) => StaticEffect | null;

const isWord = (c: Cursor, at: number, value: string) => {
  const token = c.tokens[at];
  return token?.kind === "word" && !token.possessive && token.value === value;
};

/** The first sentence must be one complete unconditional modification of a
 * single object; the second ("It gets … instead as long as …") becomes the
 * conditional static whose otherwise branch is the first. */
export function parseStaticInsteadAlternative(c: Cursor, parseFirst: StaticParser, parseModifiers: ModifierParser): StaticEffect | null {
  return c.attempt(c => {
    let period = c.pos;
    while (period < c.tokens.length && !(c.tokens[period].kind === "punct" && (c.tokens[period] as { value: string }).value === ".")) {
      if (c.tokens[period].kind === "punct" && (c.tokens[period] as { value: string }).value === '"') return null;
      period++;
    }
    if (!isWord(c, period + 1, "it")) return null;
    const first = new Cursor(c.tokens.slice(c.pos, period + 1));
    const effect = parseFirst(first);
    if (!effect || !first.done()) return null;
    if (effect.static !== "modify" || effect.condition || effect.duration || effect.bindings || !SINGLE_SUBJECTS.has(effect.what.ref)) return null;
    c.pos = period + 2;
    let at = c.pos;
    while (at < c.tokens.length && !isWord(c, at, "instead")) at++;
    if (at >= c.tokens.length) return null;
    const body = new Cursor(c.tokens.slice(c.pos, at));
    const parsed = parseModifiers(body, effect.what);
    if (!parsed || parsed.condition || !body.done() || !parsed.modifiers.length) return null;
    c.pos = at + 1;
    if (!c.words("as", "long", "as")) return null;
    const condition = parseCondition(c);
    if (!condition) return null;
    c.punct(".");
    if (!c.done()) return null;
    return { static: "conditional", condition, effect: { static: "modify", what: effect.what, modifiers: parsed.modifiers }, otherwise: effect };
  });
}
