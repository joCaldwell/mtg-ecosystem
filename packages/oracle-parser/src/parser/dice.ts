// Numeric/planar rolls and conditional results tables, CR 706.1–7/901.9.
// Natural results, modified results, ignored rolls, and a fresh "roll again"
// are distinct. Tables belong to the original ability, not separate modes.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Ability, Amount, Comparison, Condition, DiceRange, DiceResultTable, DiceResultTest, DieSpec, Effect, PlayerRef, Sentence, Trigger } from "../ast.ts";
import type { Token } from "../lexer.ts";
import { wordNumber } from "../vocab.ts";
import { Cursor } from "./cursor.ts";

interface DiceGrammar {
  parseAmount(c: Cursor): Amount | null;
  parseCount(c: Cursor): Amount | null;
  parsePlayerRef(c: Cursor): PlayerRef | null;
}
interface DiceConditionGrammar {
  parseComparison(c: Cursor): Comparison | null;
  parsePlayerRef(c: Cursor): PlayerRef | null;
}
type RollEffect = Extract<Effect, { effect: "roll-dice" }>;

function integer(c: Cursor): number | null {
  return c.attempt(c => {
    const negative = c.punct("-");
    const value = c.number();
    return value !== null && Number.isSafeInteger(value) ? negative ? -value : value : null;
  });
}

function parseRangeTerm(c: Cursor): DiceRange | null {
  return c.attempt(c => {
    const value = integer(c);
    if (value === null) return null;
    const closed = c.attempt((c): DiceRange | null => {
      if (!c.punct("—") && !c.punct("-")) return null;
      const maximum = integer(c);
      return maximum !== null && maximum >= value ? { range: "closed", minimum: value, maximum } : null;
    });
    if (closed) return closed;
    if (c.punct("+") || c.words("or", "higher") || c.words("or", "greater") || c.words("or", "more"))
      return { range: "at-least", value };
    if (c.words("or", "less") || c.words("or", "lower") || c.words("or", "fewer"))
      return { range: "at-most", value };
    return { range: "exact", value };
  });
}

export function parseDiceRange(c: Cursor): DiceRange | null {
  return c.attempt(c => {
    const first = parseRangeTerm(c);
    if (!first) return null;
    const ranges = [first];
    while (c.word("or") !== null) {
      const next = parseRangeTerm(c);
      if (!next) return null;
      ranges.push(next);
    }
    return ranges.length === 1 ? first : { range: "any-of", ranges };
  });
}

function die(c: Cursor): DieSpec | null {
  return c.attempt(c => {
    if (c.words("the", "planar", "die")) return { die: "planar" };
    if (c.word("d") !== null) {
      const sides = c.number();
      return sides !== null && Number.isSafeInteger(sides) && sides > 0 ? { die: "numeric", sides } : null;
    }
    const sides = c.attempt(c => {
      const count = c.number();
      if (count !== null && c.punct("-") && c.word("sided") !== null) return count;
      const word = c.anyWord();
      if (!word || !word.value.endsWith("-sided")) return null;
      return wordNumber(word.value.slice(0, -6)) ?? null;
    });
    if (sides === null || !Number.isSafeInteger(sides) || sides <= 0 || c.word("die", "dice") === null) return null;
    return { die: "numeric", sides };
  });
}

function dieAndCount(c: Cursor, g: DiceGrammar): { die: DieSpec; count: Amount } | null {
  return c.attempt(c => {
    if (c.isWord("the")) {
      const kind = die(c);
      return kind?.die === "planar" ? { die: kind, count: { amount: "fixed", value: 1 } } : null;
    }
    const count = g.parseCount(c);
    const kind = die(c);
    if (!count || !kind || count.amount === "fixed" && (!Number.isSafeInteger(count.value) || count.value < 0)) return null;
    return { die: kind, count };
  });
}

export function parseDiceRoll(c: Cursor, g: DiceGrammar, options?: { previousActor?: PlayerRef; allowRollAgain?: boolean }): Effect | null {
  return c.attempt(c => {
    const explicit = g.parsePlayerRef(c);
    const optional = !!explicit && c.word("may") !== null;
    const verb = c.word("roll", "rolls");
    if (!verb || optional && verb !== "roll") return null;
    const who = explicit ?? (verb === "roll" && options?.previousActor?.player !== "they" ? { player: "you" as const } : options?.previousActor);
    if (!who) return null;
    if (c.word("again") !== null) {
      if (!options?.allowRollAgain) return c.fail("a lexically enclosing dice results table");
      return { effect: "roll-again", who, of: { roll: "enclosing-table" }, ...(optional ? { optional: true } : {}) };
    }
    const specification = dieAndCount(c, g);
    if (!specification) return null;
    const effect: RollEffect = { effect: "roll-dice", who, ...specification, ...(optional ? { optional: true } : {}) };
    // An instruction's numeric adjustment is not an add-mana effect. Only
    // one explicitly rolled die has an unambiguous scalar result here.
    while (c.attempt(c => c.word("and") !== null && c.isWord("add", "subtract") ? true : null)) {
      const operation = c.word("add", "subtract") as "add" | "subtract";
      if (effect.die.die !== "numeric" || effect.count.amount !== "fixed" || effect.count.value !== 1) return null;
      const amount = g.parseAmount(c);
      if (!amount) return null;
      (effect.adjustments ??= []).push({ operation, appliesTo: "single-result", amount });
    }
    const ignore = c.attempt((c): RollEffect["ignore"] | null => {
      if (!c.words("and", "ignore")) return null;
      if (c.words("all", "but", "the", "highest", "roll")) return { ignore: "all-but-highest" };
      if (c.word("the") !== null && c.word("lower", "lowest") !== null && c.word("roll") !== null)
        return { ignore: "lowest", count: 1 };
      return null;
    });
    if (ignore) {
      if (effect.die.die !== "numeric") return null;
      effect.ignore = ignore;
    }
    if (optional && (c.isPunct(",") || c.isWord("and", "then"))) return null;
    return effect;
  });
}

/** These expressions preserve a required roll binding, never the most recent
 * numeric amount. The context may be an instruction, trigger or table row. */
export function parseDiceResultAmount(c: Cursor): Amount | null {
  return c.attempt(c => {
    if (c.words("the", "total", "of", "those", "results") || c.words("the", "total", "of", "the", "results"))
      return { amount: "dice-result", roll: { roll: "contextual" }, measure: "total" };
    if (c.words("the", "result")) return { amount: "dice-result", roll: { roll: "contextual" }, measure: "result" };
    return null;
  });
}

export function parseDiceCondition(c: Cursor, g: DiceConditionGrammar): Condition | null {
  return c.attempt(c => {
    const scalar = c.attempt((c): Condition | null => {
      const value = parseDiceResultAmount(c) ?? (c.words("the", "roll")
        ? { amount: "dice-result" as const, roll: { roll: "contextual" as const }, measure: "result" as const } : null);
      if (!value || c.word("is", "was") === null) return null;
      const range = c.attempt(parseDiceRange);
      if (range) return rangeCondition(value, range);
      const comparison = g.parseComparison(c);
      return comparison && { condition: "compare", value, comparison };
    });
    if (scalar) return scalar;
    const any = c.attempt((c): Condition | null => {
      if (!c.words("any", "of", "those", "results") || c.word("was", "is") === null) return null;
      const range = parseDiceRange(c);
      return range && { condition: "dice-result", roll: { roll: "contextual" }, test: { basis: "final", range }, quantifier: "any" };
    });
    if (any) return any;
    const who = g.parsePlayerRef(c);
    if (!who || c.word("rolled") === null) return null;
    const natural = c.word("natural") !== null;
    const range = parseDiceRange(c);
    return range && { condition: "dice-result", roll: { roll: "contextual" }, who, test: { basis: natural ? "natural" : "final", range }, quantifier: "single" };
  });
}

function rangeCondition(value: Amount, range: DiceRange): Condition {
  switch (range.range) {
    case "exact": return { condition: "compare", value, comparison: { op: "eq", value: { amount: "fixed", value: range.value } } };
    case "at-least": case "at-most": return { condition: "compare", value, comparison: { op: range.range === "at-least" ? "ge" : "le", value: { amount: "fixed", value: range.value } } };
    case "closed": return { condition: "all-of", conditions: [rangeCondition(value, { range: "at-least", value: range.minimum }), rangeCondition(value, { range: "at-most", value: range.maximum })] };
    case "any-of": return { condition: "any-of", conditions: range.ranges.map(part => rangeCondition(value, part)) };
  }
}

export function parseDiceTrigger(c: Cursor, g: DiceGrammar): Trigger | null {
  return c.attempt(c => {
    if (c.word("when", "whenever") === null) return null;
    const who = g.parsePlayerRef(c);
    if (!who || c.word("roll", "rolls") === null) return null;
    if (c.words("one", "or", "more", "dice")) return { trigger: "roll-dice", who, grouping: "one-or-more" };
    if (c.words("a", "die")) return { trigger: "roll-dice", who };
    const highest = c.attempt((c): DiceResultTest | null => c.word("a") !== null && c.possessive("die") !== null
      && c.words("highest", "natural", "result") ? { basis: "natural", highest: true } : null);
    if (highest) return { trigger: "roll-dice", who, result: highest };
    const specification = c.attempt(c => dieAndCount(c, g));
    if (specification) {
      if (specification.die.die !== "numeric" || specification.count.amount !== "fixed" || specification.count.value !== 1) return null;
      return { trigger: "roll-dice", who, die: specification.die };
    }
    c.word("a");
    const natural = c.word("natural") !== null;
    const range = parseDiceRange(c);
    return range && { trigger: "roll-dice", who, result: { basis: natural ? "natural" : "final", range } };
  });
}

function intervals(range: DiceRange): [number, number][] {
  switch (range.range) {
    case "exact": return [[range.value, range.value]];
    case "closed": return [[range.minimum, range.maximum]];
    case "at-least": return [[range.value, Infinity]];
    case "at-most": return [[-Infinity, range.value]];
    case "any-of": return range.ranges.flatMap(intervals);
  }
}

export function parseDiceTableRows(rows: Token[][], parseBody: (c: Cursor) => Sentence[] | null): DiceResultTable | null {
  if (!rows.length) return null;
  const parsed: DiceResultTable["rows"] = [];
  const seen: [number, number][] = [];
  for (const tokens of rows) {
    const c = new Cursor(tokens);
    c.punct("•");
    const range = parseDiceRange(c);
    if (!range || (!c.punct("|") && !c.punct("—"))) return null;
    const spans = intervals(range);
    for (const span of spans) {
      if (seen.some(previous => previous[0] <= span[1] && span[0] <= previous[1])) return null;
      seen.push(span);
    }
    // Exclamatory title headings are explicit flavor metadata (e.g. Treasure
    // Chest's "Trapped! —"). Requiring title case AND the bang/em dash
    // delimiter avoids treating ordinary imperative prose as a heading.
    const label = c.attempt(c => {
      const words: string[] = [];
      while (c.peek()?.kind === "word") {
        const token = c.peek();
        if (token?.kind !== "word" || token.possessive || !/^[A-Z][A-Za-z'-]*$/.test(token.raw)) break;
        words.push(token.raw); c.next();
      }
      return words.length && c.punct("!") && c.punct("—") ? `${words.join(" ")}!` : null;
    });
    const effects = parseBody(c);
    if (!effects?.length || !c.done()) return null;
    parsed.push({ range, effects, ...(label ? { label } : {}) });
  }
  return { rows: parsed };
}

function containsRoll(value: unknown): number {
  if (Array.isArray(value)) return value.reduce((sum: number, entry) => sum + containsRoll(entry), 0);
  if (!value || typeof value !== "object") return 0;
  const object = value as Record<string, unknown>;
  return (object.effect === "roll-dice" ? 1 : 0) + Object.values(object).reduce((sum: number, entry) => sum + containsRoll(entry), 0);
}

/** Initial attachment requires the only roll to be the header's direct last
 * instruction. Thus preceding choices/moves retain order, while tables with
 * later header actions or variable definitions cannot acquire guessed scope. */
export function attachDiceTable(ability: Ability, table: DiceResultTable): Ability | null {
  if (!("effects" in ability) || containsRoll(ability.effects) !== 1) return null;
  const last = ability.effects.at(-1);
  const roll = last?.steps.at(-1);
  let roller = roll?.effect === "roll-dice" ? roll.who : undefined;
  while (roller?.player === "filtered") roller = roller.base;
  if (!last || last.condition || last.otherwise || roll?.effect !== "roll-dice" || roll.table
    || roll.die.die !== "numeric" || roll.forEach || roll.duration || roll.optional
    || roller?.player === "each-player" || roller?.player === "each-opponent") return null;
  // A bare table dispatches on one retained result. Multiple results require
  // explicit per-result/total/choice instructions, as on Celebr-8000; never
  // silently sum dice or select the first one.
  if (roll.count.amount !== "fixed" || roll.count.value < 1) return null;
  const retained = roll.ignore?.ignore === "all-but-highest" ? 1
    : roll.count.value - (roll.ignore?.ignore === "lowest" ? 1 : 0);
  if (retained !== 1) return null;
  return { ...ability, effects: [...ability.effects.slice(0, -1), {
    ...last, steps: [...last.steps.slice(0, -1), { ...roll, table }],
  }] };
}
