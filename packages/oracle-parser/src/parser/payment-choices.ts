// CR 118.12a: "Do X unless a player does Y" offers Y as an optional
// resolution payment; it is not a check for an earlier unrelated action.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Condition, Cost } from "../ast.ts";
import type { Token } from "../lexer.ts";
import { Cursor } from "./cursor.ts";
import { isManaSymbol } from "../symbols.ts";
import { parseCostList } from "./costs.ts";
import { isPluralPlayerRef, parseCounterSpec, parseEachFilter, parseObjectRef, parsePlayerRef } from "./refs.ts";
import { parseEachUnit } from "./counter-quantities.ts";
import { parsePartySize } from "./quantities.ts";

const PAYMENT_VERBS: Record<string, string> = {
  pay: "pay", pays: "pay", sacrifice: "sacrifice", sacrifices: "sacrifice",
  discard: "discard", discards: "discard", exile: "exile", exiles: "exile",
  reveal: "reveal", reveals: "reveal", return: "return", returns: "return",
  remove: "remove", removes: "remove", tap: "tap", taps: "tap",
  untap: "untap", untaps: "untap", mill: "mill", mills: "mill",
};

export function parseUnlessPayment(c: Cursor): Extract<Condition, { condition: "pays" }> | null {
  return c.attempt(c => {
    const who = parsePlayerRef(c);
    if (!who) return null;
    const verb = c.peek();
    if (verb?.kind !== "word" || verb.possessive || !PAYMENT_VERBS[verb.value]) return null;
    // Cost grammar uses imperative action names; retain the original tokens
    // and provenance while giving it the equivalent conjugated verb form.
    // "sacrifices a creature of their choice": the payer chooses what they
    // sacrifice or discard as they pay (CR 701.21a, 701.9b), so the phrase
    // naming the payer restates the payment; any other chooser fails.
    const rest = withoutPayerChoice(c.tokens.slice(c.pos + 1), who.player === "you" ? "your" : "their");
    const costCursor = new Cursor([
      { ...verb, value: PAYMENT_VERBS[verb.value], raw: PAYMENT_VERBS[verb.value] },
      ...rest.tokens,
    ]);
    const cost = parseCostList(costCursor);
    if (!cost || cost.some(item => item.cost === "tap-self" || item.cost === "untap-self")) return null;
    // Resume at the first unconsumed token, past a phrase right after the payment.
    const consumed = costCursor.pos - 1;
    c.pos += 1 + (consumed < rest.tokens.length ? rest.original[consumed] : c.tokens.length - c.pos - 1);
    // "pays {1} for each card in your graveyard": one total mana payment.
    const scalable = cost.length === 1 && cost[0].cost === "mana" && !cost[0].symbols.includes("X");
    const eachUnit = (c: Cursor) => c.words("for", "each") ? parsePartySize(c, "singular") ?? parseEachUnit(c, { parseCounterSpec, parseFilter: parseEachFilter, parseObjectRef, parsePlayerRef, isPluralPlayerRef }) : null;
    // "pays {2} plus an additional {1} for each Faerie you control": the
    // printed mana and the scaled mana are one total payment (CR 118.12a).
    const additional = scalable ? c.attempt((c): Cost | null => {
      if (!c.words("plus", "an", "additional")) return null;
      const symbols: string[] = [];
      for (let token = c.peek(); token?.kind === "symbol"; token = c.peek()) {
        if (!isManaSymbol(token.value) || token.value === "X") return null;
        symbols.push(token.value); c.pos++;
      }
      const multiplier = symbols.length ? c.attempt(eachUnit) : null;
      return multiplier && { cost: "mana", symbols, multiplier };
    }) : null;
    if (additional) return { condition: "pays", who, cost: [...cost, additional] };
    const multiplier = scalable ? c.attempt(eachUnit) : null;
    return { condition: "pays", who, cost, ...(multiplier ? { multiplier } : {}) };
  });
}

/** The tokens without each "of <payer's> choice", with every kept token's
 * index among the original tokens. */
function withoutPayerChoice(tokens: Token[], payer: "your" | "their"): { tokens: Token[]; original: number[] } {
  const kept: Token[] = [];
  const original: number[] = [];
  const isWord = (index: number, value: string) => {
    const token = tokens[index];
    return token?.kind === "word" && !token.possessive && token.value === value;
  };
  for (let i = 0; i < tokens.length; i++) {
    if (isWord(i, "of") && isWord(i + 1, payer) && isWord(i + 2, "choice")) { i += 2; continue; }
    // "sacrifices a creature or discards a card": the payer's conjugated
    // alternative is the same action as the imperative cost word.
    const token = tokens[i], previous = kept.at(-1);
    const alternative = token.kind === "word" && !token.possessive && PAYMENT_VERBS[token.value] && token.value !== PAYMENT_VERBS[token.value]
      && previous?.kind === "word" && !previous.possessive && previous.value === "or";
    kept.push(alternative ? { ...token, value: PAYMENT_VERBS[token.value], raw: PAYMENT_VERBS[token.value] } : token);
    original.push(i);
  }
  return { tokens: kept, original };
}
