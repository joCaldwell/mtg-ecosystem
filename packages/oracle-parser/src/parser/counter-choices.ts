// "Put your choice of a +1/+1, first strike, or trample counter on it" /
// "your choice of a reach counter, a vigilance counter, or a trample counter":
// the putting player chooses one printed counter kind as the instruction is
// carried out (CR 608.2d) and puts one counter of that kind; the other kinds
// are not put. Each option is the instruction "put a <kind> counter on it".
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { CounterSpec, Effect, ObjectRef, PlayerRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

const FUNCTION_WORDS = new Set(["and", "or", "a", "an", "any", "no", "of", "its", "their", "your", "the", "this", "that",
  "those", "each", "all", "other", "another", "on", "from", "with", "to", "target", "kind", "choice"]);

/** One counter kind without its noun: "+1/+1", "first strike", "trample". */
function parseKind(c: Cursor): CounterSpec | null {
  return c.attempt((c): CounterSpec | null => {
    const sign = c.punct("+") ? "+" : c.punct("-") ? "-" : null;
    if (sign) {
      const power = c.number();
      if (power === null || !c.punct("/")) return null;
      const sign2 = c.punct("+") ? "+" : c.punct("-") ? "-" : null;
      const toughness = sign2 ? c.number() : null;
      return toughness === null ? null : { type: `${sign}${power}/${sign2}${toughness}` };
    }
    const words: string[] = [];
    while (words.length < 3 && !c.isWord("counter", "counters", "or")) {
      const next = c.peek();
      if (next?.kind !== "word" || next.possessive || FUNCTION_WORDS.has(next.value)) break;
      c.next();
      words.push(next.value);
    }
    return words.length ? { type: words.join(" ") } : null;
  });
}

export function parseCounterChoice(c: Cursor, who: PlayerRef, parseObjectRef: (c: Cursor) => ObjectRef | null): Effect | null {
  return c.attempt((c): Effect | null => {
    if (who.player !== "you" || !c.words("your", "choice", "of")) return null;
    const items: { article: boolean; kind: CounterSpec; noun: boolean }[] = [];
    for (let final = false; ;) {
      const article = c.word("a", "an") !== null;
      const kind = parseKind(c);
      if (!kind) return null;
      items.push({ article, kind, noun: c.word("counter") !== null });
      if (final) break;
      if (c.punct(",")) final = c.word("or") !== null;
      else if (c.word("or") !== null) final = true;
      else return null;
    }
    // Either every option repeats "a … counter", or the options share the
    // first article and the final noun ("a +1/+1, first strike, or trample counter").
    const repeated = items.every(item => item.article && item.noun);
    const shared = items.every((item, i) => item.article === (i === 0) && item.noun === (i === items.length - 1));
    if (!repeated && !shared) return null;
    if (new Set(items.map(item => item.kind.type)).size !== items.length) return null;
    if (c.word("on") === null) return null;
    const on = parseObjectRef(c);
    if (!on) return null;
    return { effect: "choice", who, options: items.map(({ kind: counter }): Effect => ({ effect: "put-counters", on, counter, count: { amount: "fixed", value: 1 } })) };
  });
}
