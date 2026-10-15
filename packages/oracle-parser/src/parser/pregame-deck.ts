// "Before you shuffle your deck to start the game, you may reveal this card
// from your deck and exile a creature card you drafted that isn't in your
// deck." A pregame action taken just before the deck is shuffled and becomes
// the library (CR 103.3); the drafted cards not in the deck are the rest of
// that player's card pool (905.1d, 100.4).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { ObjectRef, StaticEffect } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseObjectRef } from "./refs.ts";
import { wordNumber } from "../vocab.ts";

/** "an instant or sorcery card you drafted that isn't in your deck",
 * "three or more creature cards you drafted that aren't in your deck". */
function parseDraftedCards(c: Cursor): ObjectRef | null {
  return c.attempt((c): ObjectRef | null => {
    const end = c.tokens.findIndex((t, i) => i >= c.pos && t.kind === "word" && t.value === "you" &&
      c.tokens[i + 1]?.kind === "word" && (c.tokens[i + 1] as { value: string }).value === "drafted");
    if (end < 0) return null;
    // "N or more": the player chooses at least N such cards. The count word
    // is read with its plural description; "or more" makes it a minimum.
    const first = c.peek();
    const minimum = first?.kind === "word" && !first.possessive ? wordNumber(first.value) : undefined;
    const orMore = minimum !== undefined && minimum > 1 && c.tokens[c.pos + 1]?.kind === "word" &&
      (c.tokens[c.pos + 1] as { value: string }).value === "or" && (c.tokens[c.pos + 2] as { value?: string })?.value === "more";
    const head = new Cursor(orMore ? [c.tokens[c.pos], ...c.tokens.slice(c.pos + 3, end)] : c.tokens.slice(c.pos, end));
    const parsed = parseObjectRef(head);
    if (!parsed || !head.done() || parsed.ref !== "filter") return null;
    const ref: ObjectRef | null = orMore ? { ...parsed, count: { amount: "chosen", minimum: minimum! } } : parsed;
    if (!ref || ref.ref !== "filter" || ref.upTo || ref.filter.cls !== "card") return null;
    if (!orMore && !(ref.count.amount === "fixed" && ref.count.value === 1)) return null;
    c.pos = end + 2;
    const plural = orMore;
    if (!c.words("that", plural ? "aren't" : "isn't", "in", "your", "deck")) return null;
    return { ...ref, filter: { ...ref.filter, draftedNotInDeck: { drafter: { player: "you" } } } };
  });
}

export function parsePregameDeckReveal(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    if (!c.words("before", "you", "shuffle", "your", "deck", "to", "start", "the", "game") || !c.punct(",")) return null;
    if (!c.words("you", "may", "reveal")) return null;
    const what = parseObjectRef(c);
    if (!what || !(what.ref === "self" || (what.ref === "this" && what.noun === "card"))) return null;
    if (!c.words("from", "your", "deck", "and", "exile")) return null;
    const exiled = parseDraftedCards(c);
    if (!exiled) return null;
    c.punct(".");
    return c.done() ? { static: "pregame-deck-reveal", what, who: { player: "you" }, effects: [{ effect: "exile", what: exiled }] } : null;
  });
}
