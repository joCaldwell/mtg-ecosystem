// CR 707.12, 707.12a, 112.2a: copying a card (not a spell or ability on the
// stack) creates the copy in the zone the card is in; the copy can then be
// cast from there. A copy that isn't cast ceases to exist (704.5e).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Effect, ObjectRef, PlayerRef, Sentence } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseAmount, parseObjectRef } from "./refs.ts";

type CardCopy = Extract<Effect, { effect: "copy-card" }>;

/** The printed description says the copied object is a card. */
function cardReference(what: ObjectRef): boolean {
  return "filter" in what && what.filter.cls === "card";
}

/** An untyped "it"/"them" is a card only when the copy is then cast: a copy
 * of a spell or ability is never cast (CR 707.10). */
function copyIsCastNext(c: Cursor): boolean {
  const probe = new Cursor(c.tokens.slice(c.pos));
  if (!probe.punct(".")) {
    probe.punct(",");
    if (probe.word("then") === null) probe.word("and");
  }
  probe.words("you", "may");
  if (probe.word("cast") === null) return false;
  return probe.words("the", "copy") || probe.words("the", "copies") || probe.words("any", "number", "of", "the", "copies");
}

function sourceIsOne(what: ObjectRef): boolean | undefined {
  switch (what.ref) {
    case "it": return true;
    case "them": case "each": case "all": return false;
    case "that": return !what.plural;
    case "related": case "choice-result": return what.number === "singular";
    case "filter": case "target": case "from-pool":
      return what.count.amount === "fixed" ? what.count.value === 1 : false;
    default: return undefined;
  }
}

export function parseCardCopy(c: Cursor, options: {
  actor: (c: Cursor, imperative: string, conjugated: string) => PlayerRef | null;
}): CardCopy | null {
  return c.attempt((c): CardCopy | null => {
    const who = options.actor(c, "copy", "copies");
    if (!who) return null;
    const what = parseObjectRef(c);
    if (!what) return null;
    let count: Amount = { amount: "fixed", value: 1 };
    if (c.word("twice") !== null) count = { amount: "fixed", value: 2 };
    else {
      const repetitions = c.attempt(c => {
        const amount = parseAmount(c);
        return amount && c.word("times") !== null ? amount : null;
      });
      if (repetitions) count = repetitions;
    }
    if (!cardReference(what) && !((what.ref === "it" || what.ref === "them") && copyIsCastNext(c)))
      return c.fail("card to copy");
    return { effect: "copy-card", who, what, count };
  });
}

/** "the copy" / "the copies" / "any number of the copies" as what is cast. */
export function parseCopyResultRef(c: Cursor): ObjectRef | null {
  return c.attempt((c): ObjectRef | null => {
    if (c.words("the", "copy")) return { ref: "copy-result", number: "singular" };
    if (c.words("the", "copies")) return { ref: "copy-result", number: "plural" };
    if (c.words("any", "number", "of", "the", "copies"))
      return { ref: "from-pool", pool: { ref: "copy-result", number: "plural" }, count: { amount: "chosen", minimum: 0 } };
    return null;
  });
}

function copyResultNumber(ref: ObjectRef | undefined): "singular" | "plural" | undefined {
  if (!ref) return undefined;
  if (ref.ref === "copy-result") return ref.number;
  if (ref.ref === "from-pool") return copyResultNumber(ref.pool);
  return undefined;
}

/** A cast of "the copy"/"the copies" needs the card copy made earlier in the
 * same instructions, and its grammatical number must be the number of copies
 * that instruction makes. Returns false when some cast has neither. */
export function castCopiesAreCreated(sentences: Sentence[]): boolean {
  let created: "singular" | "plural" | undefined;
  let valid = true;
  const visit = (value: unknown): void => {
    if (!value || typeof value !== "object") return;
    if (Array.isArray(value)) { value.forEach(visit); return; }
    const node = value as Record<string, unknown>;
    if (node.effect === "copy-card") {
      const copy = node as unknown as CardCopy;
      const one = sourceIsOne(copy.what) === true && copy.count.amount === "fixed" && copy.count.value === 1 && !copy.forEach;
      created = one ? "singular" : "plural";
    } else if (node.effect === "play-card") {
      const instruction = (node as unknown as Extract<Effect, { effect: "play-card" }>).instruction;
      const number = copyResultNumber(instruction.what) ?? copyResultNumber(instruction.fromAmong);
      if (number && number !== created) valid = false;
    }
    Object.values(node).forEach(visit);
  };
  visit(sentences);
  return valid;
}

/** "Copy the exiled card. You may cast the copy.": the second instruction
 * names the result of the first, so both belong to one body. */
export function isCardCopyCast(before: Effect, after: Effect): boolean {
  return before.effect === "copy-card" && after.effect === "play-card"
    && (copyResultNumber(after.instruction.what) ?? copyResultNumber(after.instruction.fromAmong)) !== undefined;
}
