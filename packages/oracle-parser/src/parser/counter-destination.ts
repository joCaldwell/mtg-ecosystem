// CR 608.2c names this exact look-ahead: "Counter target spell. If that
// spell is countered this way, put it on top of its owner's library instead
// of into its owner's graveyard." The later sentence modifies where the
// counter event itself puts the spell (701.6a); it is neither a standing
// replacement effect nor a later instruction that moves a graveyard card.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { CounteredSpellDestination, ObjectFilter, Sentence } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseFilter, parseZoneRef } from "./refs.ts";

function countedObject(c: Cursor): boolean {
  return c.word("it") !== null || c.words("that", "card") || c.words("that", "spell");
}

/** Folds the rider into the single preceding counter instruction and
 * consumes it. The subject must be that instruction's one targeted spell;
 * an indefinite subject restricts which countered spells are moved. */
export function foldCounterDestination(c: Cursor, previous: Sentence): boolean {
  const step = previous.steps.length === 1 ? previous.steps[0] : undefined;
  if (!step || step.effect !== "counter" || step.countered || step.optional || step.forEach || previous.otherwise || previous.bindings) return false;
  const what = step.what;
  if (what.ref !== "target" || what.filter.cls !== "spell" || what.upTo || what.count.amount !== "fixed" || what.count.value !== 1) return false;
  const destination = c.attempt((c): CounteredSpellDestination | null => {
    if (c.word("if") === null) return null;
    let only: ObjectFilter | undefined;
    if (!c.words("that", "spell")) {
      if (c.word("a", "an") === null) return null;
      const filter = parseFilter(c);
      if (!filter || filter.cls !== "spell") return null;
      only = filter;
    }
    if (!c.words("is", "countered", "this", "way") || !c.punct(",")) return null;
    let result: CounteredSpellDestination;
    let putting = false;
    if (c.word("exile") !== null) {
      if (!countedObject(c)) return null;
      result = { to: { zone: "exile" } };
      putting = true;
    } else {
      if (c.word("put") === null || !countedObject(c) || c.word("on", "into", "onto") === null) return null;
      const to = parseZoneRef(c);
      if (!to || to.zone === "graveyard" || to.zone === "stack") return null;
      if (to.zone === "battlefield") {
        // A countered card has no default controller on the battlefield
        // other than its owner; only the printed controller is supported.
        if (!c.words("under", "your", "control")) return null;
        result = { to, controller: "you" };
      } else {
        if (to.owner !== "its-owner") return null;
        result = { to };
      }
    }
    if (!c.words("instead", "of")) return null;
    if (putting && (c.word("putting") === null || c.word("it") === null)) return null;
    if (c.word("into") === null) return null;
    const replaced = parseZoneRef(c);
    // "That player" is the owner just named by the new destination.
    if (replaced?.zone !== "graveyard" || replaced.position
      || !(replaced.owner === "its-owner" || replaced.owner === "that-player" && result.to.owner === "its-owner")) return null;
    if (!c.punct(".") && !c.done()) return null;
    return only ? { ...result, only } : result;
  });
  if (!destination) return false;
  step.countered = destination;
  return true;
}
