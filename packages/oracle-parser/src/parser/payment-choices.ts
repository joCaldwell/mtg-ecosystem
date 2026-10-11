// CR 118.12a: "Do X unless a player does Y" offers Y as an optional
// resolution payment; it is not a check for an earlier unrelated action.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Condition } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseCostList } from "./costs.ts";
import { parsePlayerRef } from "./refs.ts";

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
    const costCursor = new Cursor([
      { ...verb, value: PAYMENT_VERBS[verb.value], raw: PAYMENT_VERBS[verb.value] },
      ...c.tokens.slice(c.pos + 1),
    ]);
    const cost = parseCostList(costCursor);
    if (!cost || cost.some(item => item.cost === "tap-self" || item.cost === "untap-self")) return null;
    c.pos += costCursor.pos;
    return { condition: "pays", who, cost };
  });
}
