// Player references and players derived from objects.
import { isDeepStrictEqual } from "node:util";
import type { PlayerRef } from "../../ast.ts";
import type { Analyzer } from "./analyzer.ts";
import { child, combine, type Candidate, type Context } from "./context.ts";

/** Players defined relative to "you": your opponents (CR 102.2), or every
 * player other than you. */
export function relativePlayers(this: Analyzer, relation: "opponents" | "other-players", path: string, context: Context): void {
  const expression = { implicit: relation } as const;
  if (context.barrier) { this.unresolved(expression, path, "unsupported-scope", context.barrier, ["102.2"]); return; }
  const id = this.node({ id: this.id(path, relation), path, node: "derived-player", scope: context.ability, role: relation, of: context.you, rules: ["102.2", "109.5"] });
  this.bound(expression, path, id, ["102.2"]);
}
export function player(this: Analyzer, expression: PlayerRef, path: string, context: Context): Candidate | undefined {
  const target = expression.player === "target-player" || expression.player === "target-opponent";
  const count = target ? this.boundTargetCount(expression, context) : undefined;
  const scope = target ? { ...context, announcementOperand: true } : context;
  for (const [key, value] of Object.entries(expression))
    if (key !== "count" || !count) this.walk(value, child(path, key), scope);
  if (context.barrier) {
    this.unresolved(expression, path, "unsupported-scope", context.barrier, ["109.5"]);
    return undefined;
  }
  if (expression.player === "filtered") {
    if (context.exchangeEntrant && context.targetLegalityQuery &&
        isDeepStrictEqual(expression, { player: "filtered", base: { player: "each-player" }, filter: {
          otherThan: { player: "controller", of: { ref: "trigger-event-object", event: "enter" } },
        } })) return this.selection(expression, path, context, false);
    this.unresolved(expression, path, "unsupported-scope", "Player eligibility requires a per-candidate predicate scope; the base target or quantifier does not remove these restrictions.", ["115.1", "608.2c"]);
    return undefined;
  }
  if (expression.player === "you") return this.bound(expression, path, context.you, ["109.5"]);
  if (expression.player === "those-players" && context.unexportedSecretChoices) {
    this.unresolved(expression, path, "unsupported-scope",
      "The private choice cohort and its outcome recipients are local to that instruction.", ["608.2c", "608.2d"]);
    return undefined;
  }
  if (expression.player === "chosen-player") {
    this.unresolved(expression, path, "unsupported-reference", "The chosen player requires its exact announcement, resolution or linked pre-entry choice; no new selection is implied.", ["601.2c", "608.2d", "607.1c", "607.2d"]);
    return undefined;
  }
  if (expression.player === "the-player" && context.definiteEventPlayer &&
      context.players.every(player => player.id === context.definiteEventPlayer?.id || this.weakPlayers.has(player.id)))
    return this.bound(expression, path, context.definiteEventPlayer.id, ["106.12a", "102.1", "504.1", "603.2c"]);
  if (expression.player === "group" || expression.player === "the-player") {
    this.unresolved(expression, path, "unsupported-reference", expression.player === "group"
      ? "Grouped players require occurrence-preserving participant, event multiplicity and aggregate result bindings."
      : "This definite player requires its exact contextual event or choice role; proximity is not proof of identity.", ["608.2c", "608.2e", "608.2f"]);
    return undefined;
  }
  if (expression.player === "they" || expression.player === "that-player") {
    // Within an instruction, "they" is its acting player ("Target opponent
    // sacrifices a creature they control").
    if (context.actor) return this.bound(expression, path, context.actor.id, ["608.2k"]);
    // A player named only through an object ("its owner's hand", "a land an
    // opponent controls") is meant only when no player is named outright.
    const strong = context.players.filter(player => !this.weakPlayers.has(player.id));
    const found = strong.length ? strong : context.players;
    const owners = new Set<string>();
    const distinct = found.filter(player => {
      const node = this.result.nodes.find(candidate => candidate.id === player.id);
      const owned = node?.node === "derived-player" && node.role === "owner" ? node.of
        : node?.node === "selection" && "player" in node.expression && node.expression.player === "owner"
          ? this.at(child(node.path, "of"))?.id : undefined;
      if (!owned) return true;
      if (owners.has(owned)) return false;
      owners.add(owned);
      return true;
    });
    if (distinct.length === 1 && this.targetedPlayerGroups.has(distinct[0].id)) {
      this.unresolved(expression, path, "unsupported-scope", "A targeted player group requires a separate continuation member scope.", ["608.2f"]);
      return undefined;
    }
    return this.antecedent(expression, path, distinct, "unknown", distinct.length < found.length ? ["108.3", "608.2k"] : ["608.2k"]);
  }
  if (expression.player === "that-player-or-controller") {
    this.unresolved(expression, path, "unsupported-reference", "This player depends on whether one earlier object-or-player antecedent is a player or an object; that shared antecedent must be bound first.", ["608.2c"]);
    return undefined;
  }
  if (expression.player === "enchanted-player") {
    // CR 303.4b, 303.4m: the player the Aura is attached to, not its controller.
    if (context.sourceMoved) {
      this.unresolved(expression, path, "unsupported-reference", "After a source zone change, the attachment relation needs the source's last-known information.", ["400.7", "608.2h"]);
      return undefined;
    }
    return this.bound(expression, path, this.attachedTo(path, context), ["303.4b", "303.4m"]);
  }
  if (expression.player === "defending-player" && !context.sourceMoved) {
    // CR 508.5: for an ability of an attacking creature, or one that names
    // an attacking creature, the player that creature is attacking. Only
    // one such creature can fix it; for several, each has its own (508.5a).
    const attacker = context.attacker ?? (context.attackingSource ? { id: context.source, cardinality: "one" as const, domain: "object" as const } : undefined);
    if (attacker?.cardinality === "one") {
      const id = this.id(path, "defending");
      this.node({ id, path, node: "derived-player", scope: context.ability, role: "defending", of: attacker.id, rules: ["506.2", "508.5"] });
      return this.bound(expression, path, id, ["508.5"]);
    }
  }
  if (expression.player === "replacement-event-player" || expression.player === "prevention-event-player" || expression.player === "defending-player") {
    this.unresolved(expression, path, "unsupported-reference", "This player requires a bound replacement or combat event.", ["508.1", "614"]);
    return undefined;
  }
  return this.selection(expression, path, context,
    expression.player === "target-player" || expression.player === "target-opponent");
}
/** Players targeted anywhere in the instruction at `path` ("target
 * player's library", "creatures target opponent controls"). */
export function targetPlayersUnder(this: Analyzer, path: string, context: Context): Candidate[] {
  const found: Candidate[] = [];
  for (const reference of this.result.references) {
    if (reference.binding !== "bound" || !reference.path.startsWith(`${path}/`) && reference.path !== path) continue;
    const node = this.result.nodes.find(entry => entry.id === reference.target);
    if (node?.node === "selection" && node.timing === "stack-announcement" && "player" in node.expression && node.cardinality === "one" && node.id !== context.you)
      found.push({ id: node.id, cardinality: "one", domain: "player" });
  }
  return combine([], found);
}
/** The player one object is described by ("a land an opponent controls",
 * "target card from an opponent's graveyard"), who "that player" may then
 * mean: that object's controller or owner (CR 108.3, 404.2). */
export function relationPlayer(this: Analyzer, object: Candidate, path: string, context: Context): Candidate | undefined {
  const filter = object.description;
  if (!filter || object.cardinality !== "one" || object.domain !== "object") return undefined;
  const role = filter.control === "opponent" ? "controller"
    : filter.own === "opponent" || filter.zone?.owner === "an-opponent" ? "owner" : undefined;
  if (!role) return undefined;
  const id = this.id(path, `${role}-of-described`);
  if (!this.result.nodes.some(node => node.id === id))
    this.node({ id, path, node: "derived-player", scope: context.ability, role, of: object.id, rules: role === "owner" ? ["108.3", "404.2"] : ["108.4"] });
  this.weakPlayers.add(id);
  return { id, cardinality: "one", domain: "player" };
}
