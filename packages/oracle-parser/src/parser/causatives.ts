// "[Player] may have [someone or something] [act]": the first player only
// decides (CR 608.2d); the instruction keeps its own actor or damage source.
// "You may have …" is read by the step grammar itself.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, PlayerRef } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { performedBy } from "./player-actors.ts";

type Damage = Extract<Effect, { effect: "damage" }>;

/** The deciding player's own pronoun as a damage recipient ("deal 4 damage
 * to them") is that player; the object grammar alone reads "them" as
 * objects. Only a single decider gives the pronoun this antecedent. */
function recipientIsDecider(damage: Damage): Damage | null {
  if (damage.to.to === "them")
    return { ...damage, to: { to: "player", ref: { player: "they" } } };
  return damage;
}

/** A token created without a stated creator is created by you (the
 * create-token node omits `who` then), so "have you create …" is yours; so
 * is a zone move with no stated mover ("have you put that card into your
 * hand"). */
function actsFor(effect: Effect, actor: PlayerRef): boolean {
  if (actor.player === "you" && (effect.effect === "create-token" || effect.effect === "move-zone") && !effect.who) return true;
  return performedBy([effect], actor);
}

/** After "[player] may", at "have". The subject is either a player, who
 * must then perform every parsed instruction, or an object dealing damage. */
export function parseOtherPlayerCausative(
  c: Cursor, decider: PlayerRef, step: (c: Cursor) => Effect[] | null, player: (c: Cursor) => PlayerRef | null,
): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    if (c.word("have") === null) return null;
    const start = c.pos;
    const actor = c.attempt(player);
    c.pos = start;
    const effects = step(c);
    if (!effects) return null;
    if (actor) return effects.every(effect => actsFor(effect, actor)) ? effects : c.fail("instruction performed by the player named after 'have'");
    const damage = effects.map(effect => effect.effect === "damage" ? recipientIsDecider(effect) : null);
    if (damage.some(effect => effect === null)) return c.fail("player or damage source after 'have'");
    if (["each-player", "each-opponent", "group"].includes(decider.player) &&
        damage.some((effect, index) => effect !== effects[index])) return c.fail("single antecedent for 'them'");
    return damage as Effect[];
  });
}
