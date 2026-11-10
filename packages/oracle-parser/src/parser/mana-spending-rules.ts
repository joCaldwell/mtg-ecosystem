// Static rules about how mana may be spent. CR 609.4b: spending mana "as
// though it were mana of any color/type" affects only how a cost may be
// paid; it changes neither the cost nor the mana actually spent.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Color, ManaSpendingUse, ObjectRef, PlayerRef, StaticEffect } from "../ast.ts";
import { CARD_TYPES, COLORS } from "../vocab.ts";
import { Cursor } from "./cursor.ts";
import { parseSpendingUse } from "./mana.ts";

type ManaSpending = Extract<StaticEffect, { static: "mana-spending" }>;

function parseColor(c: Cursor): Color | null {
  const token = c.peek();
  if (token?.kind !== "word" || token.possessive || !COLORS.has(token.value)) return null;
  c.next();
  return token.value as Color;
}

/** "pay the activation costs of this artifact's abilities": the same payments
 * as "activate abilities of" that one object (CR 602.2b). */
function parseOwnActivation(c: Cursor): ManaSpendingUse | null {
  return c.attempt((c): ManaSpendingUse | null => {
    if (!c.words("pay", "the", "activation", "costs", "of")) return null;
    let source: ObjectRef;
    if (c.selfrefPossessive()) source = { ref: "self" };
    else {
      if (c.word("this") === null) return null;
      const noun = c.possessive(...CARD_TYPES, "permanent");
      if (!noun) return null;
      source = { ref: "this", noun };
    }
    return c.word("abilities") !== null ? { use: "activate-abilities-of", source } : null;
  });
}

/** "You may spend [blue] mana as though it were mana of any color [to cast
 * … / to activate …]". The complete line is the rule; purposes joined by
 * "or to" are alternatives. */
export function parseManaSpendingRule(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    let who: PlayerRef;
    if (c.words("you", "may")) who = { player: "you" };
    else if (c.words("players", "may")) who = { player: "each-player" };
    else return null;
    if (c.word("spend") === null) return null;
    const color = parseColor(c);
    if (!c.words("mana", "as", "though", "it", "were")) return null;
    let as: ManaSpending["as"];
    if (c.words("mana", "of", "any")) {
      const kind = c.word("color", "type");
      if (!kind) return null;
      as = kind === "color" ? "any-color" : "any-type";
    } else {
      const other = parseColor(c);
      if (!other || other === color || c.word("mana") === null) return null;
      as = { color: other };
    }
    let uses: ManaSpendingUse[] | undefined;
    if (c.word("to") !== null) {
      const first = parseSpendingUse(c) ?? parseOwnActivation(c);
      if (!first) return null;
      uses = [first];
      while (c.attempt(c => {
        if (!c.words("or", "to")) return null;
        const next = parseSpendingUse(c);
        if (!next) return null;
        uses!.push(next);
        return true;
      })) { /* each printed purpose is retained */ }
    }
    c.punct(".");
    if (!c.done()) return c.fail("complete mana-spending rule");
    return { static: "mana-spending", who, ...(color ? { mana: { color } } : {}), as, ...(uses ? { uses } : {}) };
  });
}
