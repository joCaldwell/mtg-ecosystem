// "Can be the targets of spells and abilities … as though they didn't have
// hexproof/shroud." CR 609.4 (the pretense holds only for choosing targets),
// 702.11b–e (hexproof, including "hexproof from"), 702.18a (shroud).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, ObjectRef, PlayerRef, StaticEffect, TargetingPretense } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseObjectRef, parsePlayerRef } from "./refs.ts";

function parseSubject(c: Cursor): ObjectRef | PlayerRef | null {
  return c.attempt((c): ObjectRef | PlayerRef | null => {
    if (c.words("your", "opponents")) return { player: "each-opponent" };
    const what = parseObjectRef(c);
    // The rule covers every matching permanent, or this one; it never selects.
    return what && ["each", "self", "this"].includes(what.ref) ? what : null;
  });
}

function parsePretense(c: Cursor): TargetingPretense | null {
  return c.attempt((c): TargetingPretense | null => {
    const first = parseSubject(c);
    if (!first) return null;
    const what = [first];
    while (c.attempt(c => {
      if (c.word("and") === null) return null;
      const next = parseSubject(c);
      if (!next) return null;
      what.push(next);
      return true;
    })) { /* every printed subject is retained */ }
    const plural = what.length > 1 || "player" in first || first.ref === "each";
    if (!c.words("can", "be", "the", plural ? "targets" : "target", "of", "spells", "and", "abilities")) return null;
    let by: PlayerRef | undefined;
    if (c.words("you", "control")) by = { player: "you" };
    else if (c.words("controlled", "by")) {
      const who = parsePlayerRef(c);
      if (!who) return null;
      by = who;
    }
    if (!c.words("as", "though", plural ? "they" : "it", "didn't", "have")) return null;
    const lacking = c.word("hexproof", "shroud") as "hexproof" | "shroud" | null;
    return lacking && { what, lacking, ...(by ? { by } : {}) };
  });
}

/** Whole-line static rule. */
export function parseTargetingStatic(c: Cursor): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    const pretense = parsePretense(c);
    if (!pretense) return null;
    c.punct(".");
    if (!c.done()) return c.fail("complete targeting rule");
    return { static: "targeting-as-though", ...pretense };
  });
}

/** Resolving form; its duration is attached by the effect grammar. */
export function parseTargetingEffect(c: Cursor): Effect[] | null {
  const pretense = parsePretense(c);
  return pretense && [{ effect: "targeting-as-though", ...pretense }];
}
