// Several printed subjects of one resolving modifier: "Target creature and
// each other creature that shares a color with it get +1/+1 until end of
// turn", "~ and Drakes you control each get +X/+X", "You and permanents you
// control gain hexproof". Every subject receives the same modification at
// once (CR 608.2c, 611.2c); players and objects stay separate recipients
// (109.1, 113.1). https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Effect, GainedAbility, ObjectRef, PlayerRef, SignedAmount, StaticEffect, StaticModifier } from "../ast.ts";
import type { Cursor } from "./cursor.ts";
import { splitsSharedScope } from "./shared-scope.ts";

/** A modifier applies once to every object in the union of its subjects
 * (CR 611.2c, 611.3a), so separate per-subject effects are only faithful when
 * no object can be in two subjects. Subjects that are all filters become one
 * union filter. Otherwise every later subject must carry the printed
 * "other"/"another" contrast with the single objects named before it. A
 * possibly overlapping list ("~ and Drakes you control") needs a union
 * reference that the AST does not have, and is rejected. */
export function unionSubjects(c: Cursor, objects: ObjectRef[]): ObjectRef[] | null {
  if (objects.length < 2) return objects;
  if (objects.every((object): object is Extract<ObjectRef, { ref: "each" }> => object.ref === "each"))
    return [{ ref: "each", filter: { anyOf: objects.map(object => object.filter) } }];
  const single = (ref: ObjectRef): boolean => ["self", "this", "it", "enchanted", "equipped"].includes(ref.ref)
    || (ref.ref === "that" && !ref.plural)
    || (ref.ref === "target" && !ref.upTo && ref.count.amount === "fixed" && ref.count.value === 1);
  // CR 303.4d, 301.5c: an Aura can't enchant itself and an Equipment can't
  // equip itself, so the source is never its own enchanted/equipped object.
  const attachedToSource = objects.length === 2 && ["self", "this"].includes(objects[0].ref)
    && ["enchanted", "equipped"].includes(objects[1].ref);
  const disjoint = attachedToSource || objects.every((object, index) => index === 0 ? single(object)
    : "filter" in object && object.filter.other === true && objects.slice(0, index).every(single));
  return disjoint ? objects : c.fail("subjects that cannot overlap, or one union filter");
}

function sharesFreshChoice(abilities: GainedAbility[]): boolean {
  return abilities.some(ability => ability.gained === "keyword-choice"
    || ability.gained === "keyword" && ability.keyword.from?.some(scope => scope.scope === "fresh-choice"));
}

export interface SubjectListGrammar {
  parseObjectRef(c: Cursor): ObjectRef | null;
  parsePlayerRef(c: Cursor): PlayerRef | null;
  parsePtModifier(c: Cursor): { power: SignedAmount; toughness: SignedAmount } | null;
  parseGainedAbilities(c: Cursor): GainedAbility[] | null;
}

/** Two or more subjects joined by "and", an optional floating "each" (the
 * modifier already applies to every subject), then the plural verb. One
 * effect per recipient left by unionSubjects (players stay separate); the
 * caller groups several as simultaneous and shares a trailing duration. Only "get" (objects) and "gain" are read here; a bare
 * plural whose qualifier may belong to the previous noun is rejected. */
export function parseCoordinatedSubjects(c: Cursor, g: SubjectListGrammar): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    const subjects: (ObjectRef | PlayerRef)[] = [];
    for (;;) {
      const start = c.peek();
      const subject = c.attempt(c => {
        const player = g.parsePlayerRef(c);
        return player && c.isWord("and", "each", "gain") ? player : null;
      }) ?? g.parseObjectRef(c);
      if (!subject) return null;
      const previous = subjects.at(-1);
      if (previous && splitsSharedScope(previous, subject, start))
        return c.fail("represented shared qualifier of coordinated subjects");
      subjects.push(subject);
      if (c.word("and") === null) break;
    }
    if (subjects.length < 2) return null;
    c.word("each");
    const verb = c.word("get", "gain");
    if (!verb) return null;
    const players = subjects.filter((subject): subject is PlayerRef => "player" in subject);
    const objects = unionSubjects(c, subjects.filter((subject): subject is ObjectRef => "ref" in subject));
    if (!objects) return null;
    if (verb === "get") {
      const pt = g.parsePtModifier(c);
      if (!pt || players.length) return null;
      const pumps = objects.map((what): Effect => ({ effect: "pump", what, power: pt.power, toughness: pt.toughness }));
      // "... get +1/+0 and gain first strike": the list is the subject of
      // both verbs of the one instruction (CR 608.2c).
      const also = c.attempt(c => c.word("and") !== null && c.word("gain") !== null ? g.parseGainedAbilities(c) : null);
      if (!also) return pumps;
      if (sharesFreshChoice(also)) return c.fail("one shared choice for coordinated subjects");
      return [...pumps, ...objects.map((what): Effect => ({ effect: "gain-abilities", what, abilities: structuredClone(also) }))];
    }
    const abilities = g.parseGainedAbilities(c);
    if (!abilities) return null;
    // One printed choice ("protection from the color of your choice") is
    // shared by every recipient; one grant per subject would repeat it.
    if (sharesFreshChoice(abilities))
      return c.fail("one shared choice for coordinated subjects");
    return [
      ...players.map((who): Effect => ({ effect: "player-gain-abilities", who, abilities })),
      ...objects.map((what): Effect => ({ effect: "gain-abilities", what, abilities })),
    ];
  });
}

export interface StaticSubjectListGrammar {
  parseStaticSubject(c: Cursor): ObjectRef | null;
  parsePlayerRef(c: Cursor): PlayerRef | null;
  parseStaticModifiers(c: Cursor, what: ObjectRef): { modifiers: StaticModifier[]; condition?: unknown } | null;
  parseAmount(c: Cursor): Amount | null;
}

/** The continuous counterpart: "~ and other Knights you control have
 * flying", "This creature and enchanted creature each get +X/+X, where X is
 * ...", "You and Humans you control have hexproof". Each printed subject gets
 * its own continuous effect in one group, as "The same is true for ..." does
 * (CR 611.3a), after unionSubjects. The modifier text is read once per object subject so anything
 * it scopes to the affected object stays with that subject; a player subject
 * can only have the abilities (113.1b), so any other modifier rejects it. */
export function parseStaticSubjectList(c: Cursor, g: StaticSubjectListGrammar): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    const subjects: (ObjectRef | PlayerRef)[] = [];
    for (;;) {
      const start = c.peek();
      const subject = c.attempt(c => {
        const player = g.parsePlayerRef(c);
        return player && c.isWord("and", "each", "have") ? player : null;
      }) ?? g.parseStaticSubject(c);
      if (!subject || ("ref" in subject && subject.ref === "target")) return null;
      const previous = subjects.at(-1);
      if (previous && splitsSharedScope(previous, subject, start))
        return c.fail("represented shared qualifier of coordinated subjects");
      subjects.push(subject);
      if (c.word("and") === null) break;
    }
    const printed = subjects.filter((subject): subject is ObjectRef => "ref" in subject);
    if (subjects.length < 2 || !printed.length) return null;
    const objects = unionSubjects(c, printed);
    if (!objects) return null;
    if (c.isWord("each")) {
      const next = c.peek(1);
      if (next?.kind !== "word" || next.possessive || !["get", "have"].includes(next.value)) return null;
      c.pos++;
    }
    if (!c.isWord("get", "have")) return null;
    const start = c.pos;
    let end = -1;
    const parsed = new Map<ObjectRef, StaticModifier[]>();
    for (const what of objects) {
      c.pos = start;
      const group = g.parseStaticModifiers(c, what);
      // A trailing condition would need one stated scope for the whole list.
      if (!group || group.condition || (end >= 0 && c.pos !== end)) return null;
      end = c.pos;
      parsed.set(what, group.modifiers);
    }
    const binding = c.attempt(c => c.punct(",") && c.words("where", "x", "is") ? g.parseAmount(c) : null);
    c.punct(".");
    if (!c.done()) return null;
    const effects: StaticEffect[] = [];
    for (const subject of [...subjects.filter((subject): subject is PlayerRef => "player" in subject), ...objects]) {
      if ("ref" in subject) {
        effects.push({ static: "modify", what: subject, modifiers: parsed.get(subject)!,
          ...(binding ? { bindings: [{ variable: "x" as const, value: binding }] } : {}) });
        continue;
      }
      const modifiers = parsed.get(objects[0])!;
      const abilities: GainedAbility[] = [];
      for (const modifier of modifiers) {
        if (modifier.modifier !== "abilities" || Object.keys(modifier).some(key => key !== "modifier" && key !== "abilities")) return null;
        abilities.push(...modifier.abilities);
      }
      if (binding) return null;
      effects.push({ static: "player-abilities", who: subject, abilities: structuredClone(abilities) });
    }
    return effects.length === 1 ? effects[0] : { static: "group", effects };
  });
}

/** A subject that announces a fresh choice ("creatures of the creature type
 * of your choice") states that choice once (CR 608.2d). Repeating the subject
 * for a later elided verb would print the choice a second time, and the AST
 * has no identity that ties two fresh choices together, so such a subject is
 * never carried over to a coordinated verb. */
export function carriesFreshChoice(value: unknown): boolean {
  if (Array.isArray(value)) return value.some(carriesFreshChoice);
  if (!value || typeof value !== "object") return false;
  if ("choice" in value && value.choice === "fresh-choice") return true;
  return Object.values(value).some(carriesFreshChoice);
}
