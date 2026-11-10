// "… unless its controller has ~ deal 5 damage to them." CR 118.12a: the
// player may take the stated action instead; here that action is having a
// source deal damage to them, which is damage dealt, not a cost paid.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Condition, PlayerRef, Sentence } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { parseAmount, parseEqualTo, parseObjectRef, parsePlayerRef } from "./refs.ts";

type DamageOffer = Extract<Condition, { condition: "has-damage-dealt" }>;

export function parseUnlessDamage(c: Cursor): DamageOffer | null {
  return c.attempt(c => {
    const who = parsePlayerRef(c);
    if (!who || c.word("has", "have") === null) return null;
    return parseDamageOffer(c, who);
  });
}

/** "Any player may have ~ deal 5 damage to them. If no one does, <instruction>."
 * CR 118.12a states this is the same as "<instruction> unless any player has
 * ~ deal 5 damage to them", so it becomes that unless-condition on the one
 * fallback sentence. Every player may accept the offer; one suffices.
 * "Any opponent may have it deal 4 damage to them. If a player does,
 * <instruction>." is the positive form (CR 118.12): the instruction happens
 * if an opponent had the damage dealt. "It" there is the source of the
 * triggered ability's own event, kept symbolic. */
export function parseOfferedDamageFallback(c: Cursor, parseFallback: (c: Cursor) => Sentence | null): Sentence | null {
  return c.attempt(c => {
    if (c.word("any") === null) return null;
    const noun = c.word("player", "opponent");
    if (!noun || !c.words("may", "have")) return null;
    const condition = parseDamageOffer(c, { player: noun === "player" ? "a-player" : "an-opponent" }, true);
    if (!condition || !c.punct(".") || c.word("if") === null) return null;
    const kind = c.words("no", "one", "does") || c.words("no", "player", "does") ? "unless" : c.words("a", "player", "does") ? "if" : null;
    if (!kind || !c.punct(",")) return null;
    const fallback = parseFallback(c);
    if (!fallback || fallback.condition || fallback.otherwise) return null;
    return { ...fallback, condition, conditionKind: kind };
  });
}

function parseDamageOffer(c: Cursor, who: PlayerRef, allowIt = false): DamageOffer | null {
  return c.attempt(c => {
    const source = parseObjectRef(c);
    // Only the source itself is printed here; another object would need its
    // own antecedent.
    if (!source || (source.ref !== "self" && source.ref !== "this" && !(allowIt && source.ref === "it")) || c.word("deal") === null) return null;
    let amount = c.attempt(parseAmount);
    // "Them" is the deciding player (singular they), the only recipient.
    if (!c.words("damage", "to", "them")) return null;
    amount ??= parseEqualTo(c);
    return amount ? { condition: "has-damage-dealt", who, source, amount } : null;
  });
}
