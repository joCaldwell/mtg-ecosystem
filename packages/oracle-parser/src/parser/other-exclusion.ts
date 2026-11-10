import type { ObjectFilter, ObjectRef, StaticEffect } from "../ast.ts";

/** Bind the counted "other" object to the noun phrase it contrasts with.
 * Quantified anthems bind independently for each affected object (CR 611.3a).
 * Sliver Legion still affects remaining Slivers if it stops being a Sliver;
 * subtracting its source from each count would give the wrong result then.
 * https://api.scryfall.com/cards/87545415-2e27-4841-a3af-7653af2ba6d3/rulings
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
export function bindOtherExclusion(filter: ObjectFilter, subject: ObjectRef): ObjectFilter | null {
  // Counted pools joined by "and" ("each other Assassin you control and each
  // Assassin card in your graveyard"): "other" belongs to its own pool.
  if (!filter.other && filter.anyOf?.some(member => member.other)) {
    const anyOf: ObjectFilter[] = [];
    for (const member of filter.anyOf) {
      const bound = bindOtherExclusion(member, subject);
      if (!bound) return null;
      anyOf.push(bound);
    }
    return { ...filter, anyOf };
  }
  if (!filter.other) return filter;
  let otherThan: ObjectFilter["otherThan"];
  if (subject.ref === "self" || subject.ref === "this") {
    // A named source or "this creature" is the comparison object even when
    // an effect changes its types so it no longer qualifies for the count.
    otherThan = { ref: "self" };
  } else if (subject.ref === "enchanted" || subject.ref === "equipped") {
    if (filter.types?.includes(subject.noun)) {
      // Bravado: "enchanted creature ... each other creature" contrasts
      // creatures with the affected creature, not with the Aura.
      otherThan = { ref: "affected-object" };
    } else if (subject.ref === "enchanted" && filter.types?.length === 1 && filter.types[0] === "enchantment" && !filter.subtypes?.length) {
      // Ancestral Mask: the counted enchantments contrast with the Aura.
      // This binding remains the Aura even if the enchanted creature also
      // becomes an enchantment; changing types does not change antecedents.
      // https://api.scryfall.com/cards/c7a19fc5-20f3-48d2-8c12-e012d3b302e7
      otherThan = { ref: "self" };
    } else if (subject.ref === "equipped" &&
        ((filter.types?.length === 1 && filter.types[0] === "artifact" && !filter.subtypes?.length) ||
         (filter.subtypes?.length === 1 && filter.subtypes[0] === "Equipment" && !filter.types?.length))) {
      otherThan = { ref: "self" };
    } else return null;
  } else if ("filter" in subject) {
    const subjectFilter = subject.filter;
    const sameType = filter.types?.some(type => subjectFilter.types?.includes(type));
    const sameSubtype = filter.subtypes?.some(subtype => subjectFilter.subtypes?.includes(subtype));
    const sameClass = filter.cls !== undefined && filter.cls === subjectFilter.cls;
    if (!sameType && !sameSubtype && !sameClass) return null;
    otherThan = { ref: "affected-object" };
  } else return null;
  const { other: _other, ...bound } = filter;
  return { ...bound, otherThan };
}

/** Bind only continuous multiplier contexts whose affected object is known.
 * Lexical markers in unrelated condition/cost/event scopes remain explicitly
 * unresolved for later binding; this is not a general name-resolution pass. */
export function bindStaticOtherExclusions(effect: StaticEffect): StaticEffect | null {
  if (effect.static === "group") {
    const effects: StaticEffect[] = [];
    for (const child of effect.effects) {
      const bound = bindStaticOtherExclusions(child);
      if (!bound) return null;
      effects.push(bound);
    }
    return { ...effect, effects };
  }
  if (effect.static === "conditional") {
    const inner = bindStaticOtherExclusions(effect.effect);
    const otherwise = effect.otherwise ? bindStaticOtherExclusions(effect.otherwise) : undefined;
    if (!inner || (effect.otherwise && !otherwise)) return null;
    return { ...effect, effect: inner, ...(otherwise ? { otherwise } : {}) };
  }
  if (effect.static !== "modify") return effect;
  const modifiers = [];
  for (const modifier of effect.modifiers) {
    if (!modifier.forEach) { modifiers.push(modifier); continue; }
    const forEach = bindOtherExclusion(modifier.forEach, effect.what);
    if (!forEach) return null;
    modifiers.push({ ...modifier, forEach });
  }
  return { ...effect, modifiers };
}
