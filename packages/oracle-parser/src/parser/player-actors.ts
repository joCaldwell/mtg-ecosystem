import type { Condition, Effect, PlayerFilter, PlayerRef } from "../ast.ts";
import { Cursor } from "./cursor.ts";

/** Explicit plural actors belong to an instruction, not an eligibility
 * query or new collection of target slots. Keep occurrences even when two
 * roles refer to the same player (Angel of Destiny's redirected-damage
 * ruling). One group is one action, with rule-defined simultaneity/order.
 * CR101.4,121.2c/d,608.2e/f:
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
 * https://magic.wizards.com/en/news/feature/zendikar-rising-release-notes-2020-09-10 */
export function parsePlayerGroupActor(c: Cursor, player: (c: Cursor) => PlayerRef | null): PlayerRef | null {
  return c.attempt(c => {
    if (!c.words("you", "and")) return null;
    const second = player(c);
    // Other groups introduce independent choices, quantifiers, reciprocal
    // roles or optional per-player decisions; those need distinct scopes.
    if (!second || !["target-opponent", "that-player"].includes(second.player) || c.word("each") === null) return null;
    return { player: "group", members: [{ player: "you" }, second], multiplicity: "per-member-occurrence" };
  });
}

/** "[Player] may": the subject of another player's optional instruction.
 * That player both decides and acts (CR 608.2d, 118.12); "you may" keeps
 * its own production. */
export function parseOtherPlayerMay(c: Cursor, player: (c: Cursor) => PlayerRef | null): PlayerRef | null {
  return c.attempt(c => {
    const who = player(c);
    return who && who.player !== "you" && c.word("may") !== null ? who : null;
  });
}

/** Every instruction governed by another player's "may" must name that
 * player as its actor. An effect without an actor of its own would silently
 * be performed by the controller instead, so it is not accepted. */
export function performedBy(effects: Effect[], who: PlayerRef): boolean {
  const expected = JSON.stringify(who);
  return effects.every(effect => effect.effect === "simultaneous" ? performedBy(effect.steps, who)
    : "who" in effect && !!effect.who && JSON.stringify(effect.who) === expected);
}

/** CR 118.12: "If they do," / "If the player doesn't," asks whether the
 * player of the directly preceding instruction took its action. Only the
 * bare clause before its comma; a restated action is a different question.
 * "You" keeps the existing you-do / you-dont nodes. */
export function parsePrecedingActionOutcome(c: Cursor, player: (c: Cursor) => PlayerRef | null): Condition | null {
  return c.attempt((c): Condition | null => {
    const who = player(c);
    if (!who || !["they", "the-player", "that-player"].includes(who.player)) return null;
    const verb = c.word("do", "does", "don't", "doesn't");
    if (!verb || (who.player === "they") !== (verb === "do" || verb === "don't") || !c.isPunct(",")) return null;
    return { condition: "preceding-action", who, outcome: verb === "do" || verb === "does" ? "performed" : "not-performed" };
  });
}

/** "who does", "who didn't", "who can't" after a quantified player: the bare
 * auxiliary stands for the preceding instruction's action (CR 118.12). A
 * restated action ("who doesn't control …", "who didn't discard …") follows
 * the auxiliary with a base verb and is a different predicate, so the next
 * token must end the noun phrase: punctuation or the clause's finite verb. */
export function parsePrecedingActionQualifier(c: Cursor): PlayerFilter | null {
  return c.attempt((c): PlayerFilter | null => {
    if (c.word("who") === null) return null;
    const verb = c.word("do", "does", "did", "don't", "doesn't", "didn't", "can't");
    if (!verb) return null;
    const next = c.peek();
    const ends = !next || (next.kind === "punct" && (next.value === "," || next.value === "."))
      || (next.kind === "word" && !next.possessive && (next.value === "can't" || next.value === "may" || /[^s]s$/.test(next.value)));
    if (!ends) return null;
    return { precedingAction: verb === "can't" ? "unable" : verb.endsWith("n't") ? "not-performed" : "performed" };
  });
}
