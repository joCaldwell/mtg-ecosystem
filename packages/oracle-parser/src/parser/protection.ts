// CR 702.16n/p narrow the attachment SBA consequence of a particular granted
// protection ability. They do not exempt future attachments or targeting.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { GainedAbility, ObjectRef, ProtectionAttachmentException, StaticModifier } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

/** A quoted ability has its own resolving instruction and is not inspected
 * here. Direct grants from continuous static effects have no resolution at
 * which the represented fresh choice could be made. */
export function hasFreshProtectionChoice(abilities: GainedAbility[]): boolean {
  return abilities.some(ability => ability.gained === "keyword-choice" || ability.gained === "keyword" &&
    ability.keyword.from?.some(scope => scope.scope === "fresh-choice"));
}

/** Called at the period after a continuous modifier group. No mutation occurs
 * on failure; a rider can bind only one explicitly granted protection ability.
 * Snapshot references remain in the granting effect's lexical scope. */
export function parseProtectionAttachmentException(c: Cursor, what: ObjectRef,
  modifiers: StaticModifier[]): StaticModifier[] | null {
  return c.attempt(c => {
    if (what.ref !== "enchanted" || what.noun !== "creature") return null;
    const grants = modifiers.flatMap((modifier, modifierIndex) => modifier.modifier === "abilities"
      ? modifier.abilities.flatMap((ability, abilityIndex) => ability.gained === "keyword" && ability.keyword.keyword === "protection"
        ? [{ ability, modifierIndex, abilityIndex }] : []) : []);
    if (grants.length !== 1 || grants[0].ability.protectionAttachmentException) return null;
    if (!c.punct(".") || !c.words("this", "effect", "doesn't", "remove")) return null;
    let exception: ProtectionAttachmentException;
    if (c.words("this", "aura")) exception = { exception: "source-aura", aura: { ref: "this", noun: "aura" } };
    else if (c.word("auras") !== null) {
      if (c.word("and") !== null) {
        if (!c.words("equipment", "you", "control", "that", "are", "already", "attached", "to", "it")) return null;
        exception = { exception: "already-attached", filter: {
          anyOf: [{ subtypes: ["Aura"] }, { subtypes: ["Equipment"] }], control: "you",
        }, to: what, snapshot: "when-protection-effect-begins" };
      } else exception = { exception: "auras" };
    } else return null;
    c.punct(".");
    if (!c.done()) return null;
    const grant = grants[0];
    return modifiers.map((modifier, modifierIndex) => modifier.modifier !== "abilities" || modifierIndex !== grant.modifierIndex
      ? modifier : { ...modifier, abilities: modifier.abilities.map((ability, abilityIndex) => abilityIndex !== grant.abilityIndex
        ? ability : { ...grant.ability, protectionAttachmentException: exception }) });
  });
}
