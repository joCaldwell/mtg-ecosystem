// "Create a token that's a copy of target creature. The token enters tapped
// and attacking." / "Put target creature card onto the battlefield under your
// control. It enters tapped and attacking that player.": the following
// sentence states how the object put onto the battlefield by the directly
// preceding instruction enters. It modifies that same entry event, exactly
// like "create a tapped and attacking token" / "onto the battlefield tapped
// and attacking that player" (CR 122.6, 508.4, 608.2c); it is not a later
// instruction and not a lasting replacement for other objects.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, ObjectRef, Sentence } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseBattlefieldEntryDetails } from "./entry.ts";

type Subject = { number: "singular" | "plural"; noun: "pronoun" | "token" | "name" };

/** `name` is the name of the token the preceding instruction creates, if any:
 * "create "Ragavan", a legendary … token. Ragavan enters tapped …". */
function parseSubject(c: Cursor, name: string | undefined): Subject | null {
  return c.attempt((c): Subject | null => {
    if (c.word("it") !== null) return { number: "singular", noun: "pronoun" };
    if (c.word("they") !== null) return { number: "plural", noun: "pronoun" };
    if (c.words("the", "token")) return { number: "singular", noun: "token" };
    if (c.words("the", "tokens")) return { number: "plural", noun: "token" };
    if (!name) return null;
    const literal = c.peek();
    if (literal?.kind === "card-name") return literal.value === name && c.next() ? { number: "singular", noun: "name" } : null;
    const words = name.split(" ");
    for (const word of words) {
      const token = c.next();
      if (token?.kind !== "word" || token.possessive || token.raw !== word) return null;
    }
    return { number: "singular", noun: "name" };
  });
}

function singular(ref: ObjectRef): boolean | null {
  if (ref.ref === "it" || ref.ref === "self" || ref.ref === "this") return true;
  if (ref.ref === "them" || ref.ref === "all" || ref.ref === "each") return false;
  // "Up to one target card": "it" is that card if one was chosen.
  if ((ref.ref === "target" || ref.ref === "filter") && ref.count.amount === "fixed") return ref.count.value === 1 ? true : null;
  return null;
}

/** Folds the sentence into the last instruction of the previous sentence and
 * consumes it; returns false (cursor unchanged) when it is not that rider. */
export function foldEntrySentence(c: Cursor, previous: Sentence): boolean {
  const step: Effect | undefined = previous.steps.at(-1);
  if (!step || previous.otherwise || step.forEach || step.forEachVote) return false;
  const start = c.pos;
  const subject = parseSubject(c, step.effect === "create-token" && "name" in step.token ? step.token.name : undefined);
  if (!subject || c.word(subject.number === "singular" ? "enters" : "enter") === null) { c.pos = start; return false; }
  const details = parseBattlefieldEntryDetails(c);
  const entry = details?.entry;
  const supported = details && !details.controller && !details.from && (details.tapped || entry?.attacking)
    && (!entry || Object.keys(entry).every(key => key === "attacking"));
  if (!supported || (!c.isPunct(".") && !c.done())) { c.pos = start; return false; }
  if (step.effect === "move-zone" && step.to.zone === "battlefield" && subject.noun === "pronoun"
      && singular(step.what) === (subject.number === "singular")
      && !(details.tapped && step.tapped) && !(entry?.attacking && step.entry?.attacking)) {
    if (details.tapped) step.tapped = true;
    if (entry?.attacking) step.entry = { ...step.entry, attacking: entry.attacking };
  } else if (step.effect === "create-token" && step.count.amount === "fixed"
      && (step.count.value === 1) === (subject.number === "singular")
      // Created tokens record entering attacking without a named defender.
      && !(entry?.attacking && "defender" in entry.attacking)
      && !(details.tapped && step.token.tapped) && !(entry?.attacking && step.token.attacking)) {
    if (details.tapped) step.token.tapped = true;
    if (entry?.attacking) step.token.attacking = true;
  } else { c.pos = start; return false; }
  c.punct(".");
  return true;
}
