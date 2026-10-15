// Rules-defined actions that change an object's or the game's status:
// manifest / cloak / manifest dread, turning face up or down, detain,
// suspect, prepare, flip, day/night, and exerting as an attack cost. Each is a closed
// keyword action with typed parameters, never a raw-text escape.
// CR 701.35 (detain), 701.40 (manifest), 701.43 (exert), 701.58 (cloak),
// 701.60 (suspect), 701.62 (manifest dread), 708 (face-down), 710 (flip),
// 722 (preparation cards), 731 (day and night):
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Ability, Amount, Effect, FaceDownCharacteristics, ObjectRef, PlayerRef, Sentence, StaticEffect } from "../ast.ts";
import { CARD_TYPES, canonicalSubtypeName, singularize } from "../vocab.ts";
import type { Cursor } from "./cursor.ts";
import { parseAmount, parseEqualTo, parseObjectRef, parseZoneRef } from "./refs.ts";
import { parseBecomeStatusAction } from "./become-status.ts";

export interface StatusActionDeps {
  /** Shared actor grammar: bare imperative = you; a conjugated verb keeps its
   * stated (or coordinated) actor. */
  actor(c: Cursor, imperative: string, conjugated: string): PlayerRef | null;
}

/** Plain grammatical number of an object reference, used only to check
 * "it/they" agreement with a face-down definition. */
function isSingularRef(ref: ObjectRef): boolean {
  if (["self", "this", "it", "enchanted", "equipped"].includes(ref.ref)) return true;
  if (ref.ref === "that") return !ref.plural;
  if (ref.ref === "related") return ref.number === "singular";
  if (ref.ref === "target" || ref.ref === "filter")
    return ref.count.amount === "fixed" && ref.count.value === 1 && !ref.upTo;
  return false;
}

/** Manifest and cloak act on cards (CR 701.40a, 701.58a), never on permanents
 * already on the battlefield. Only references that denote cards in a
 * non-battlefield zone, or an established card pool, are accepted. */
function denotesCards(ref: ObjectRef): boolean {
  switch (ref.ref) {
    case "library-cards": return true;
    case "zone-cards": return true;
    case "them": return true;
    case "that": return ref.filter.cls === "card" || ref.filter.zone !== undefined;
    case "from-pool": return denotesCards(ref.pool);
    case "filter": case "each": case "all":
      return ref.filter.cls === "card" && ref.filter.zone !== undefined && ref.filter.zone.zone !== "battlefield";
    default: return false;
  }
}

function parseManifestAction(c: Cursor, deps: StatusActionDeps): Effect | null {
  return c.attempt((c): Effect | null => {
    const manifest = c.attempt(c => deps.actor(c, "manifest", "manifests"));
    const cloak = manifest ? null : c.attempt(c => deps.actor(c, "cloak", "cloaks"));
    const who = manifest ?? cloak;
    if (!who) return null;
    if (manifest && c.word("dread") !== null) {
      // CR 701.62a. "twice" and "X times" repeat the entire procedure.
      let times: Amount | undefined;
      if (c.word("twice") !== null) times = { amount: "fixed", value: 2 };
      else {
        const count = c.attempt((c): Amount | null => {
          const amount = parseAmount(c);
          return amount && c.word("times") !== null ? amount : null;
        });
        if (count) times = count;
      }
      return { effect: "manifest-dread", who, ...(times ? { times } : {}) };
    }
    // "a number of cards from the top of your library equal to ..."
    const counted = c.attempt((c): ObjectRef | null => {
      if (!c.words("a", "number", "of", "cards", "from", "the", "top", "of")) return null;
      const of = parseZoneRef(c);
      if (of?.zone !== "library" || of.position) return null;
      const count = parseEqualTo(c);
      return count && { ref: "library-cards", position: "top", count, of };
    });
    const what = counted ?? parseObjectRef(c);
    if (!what || !denotesCards(what)) return null;
    return { effect: manifest ? "manifest" : "cloak", who, what };
  });
}

/** CR 708.2a: a face-down permanent's characteristics are only those the
 * effect lists. "turn ... face down" lists none unless a following "It's a
 * 2/2 ..." sentence does (folded by foldFaceDownCharacteristics). */
function parseTurnFaceAction(c: Cursor, deps: StatusActionDeps): Effect | null {
  return c.attempt((c): Effect | null => {
    const who = deps.actor(c, "turn", "turns");
    if (!who) return null;
    // Both word orders occur: "turn X face up" and "turn face up all cards ...".
    const leading = c.attempt((c): "up" | "down" | null => c.words("face", "up") ? "up" : null);
    const what = parseObjectRef(c);
    if (!what) return null;
    let face = leading;
    if (!face) {
      if (!c.words("face")) return null;
      face = c.word("up", "down") as "up" | "down" | null;
      if (!face) return null;
    }
    return { effect: "turn-face", what, face, ...(who.player === "you" ? {} : { who }) };
  });
}

function parseSuspectAction(c: Cursor): Effect | null {
  return c.attempt((c): Effect | null => {
    if (c.word("suspect") === null) return null;
    const what = parseObjectRef(c);
    return what && { effect: "suspect", what };
  });
}

/** "[X] is/are no longer suspected", "it's/they're no longer suspected" and
 * "[have] X become no longer suspected" (CR 701.60a). */
function parseUnsuspectAction(c: Cursor): Effect | null {
  return c.attempt((c): Effect | null => {
    let what: ObjectRef | null = null;
    const contracted = c.word("it's", "they're");
    if (contracted) what = { ref: contracted === "it's" ? "it" : "them" };
    else {
      what = parseObjectRef(c);
      if (!what) return null;
      if (c.word("is", "are", "become", "becomes") === null) return null;
    }
    return what && c.words("no", "longer", "suspected") ? { effect: "unsuspect", what } : null;
  });
}

/** "[X] becomes prepared / unprepared" (CR 722.3a-b). */
function parsePrepareAction(c: Cursor): Effect | null {
  return c.attempt((c): Effect | null => {
    const what = parseObjectRef(c);
    if (!what || c.word("becomes", "become") === null) return null;
    const to = c.word("prepared", "unprepared");
    return to ? { effect: to === "prepared" ? "prepare" : "unprepare", what } : null;
  });
}

function parseDetainAction(c: Cursor): Effect | null {
  return c.attempt((c): Effect | null => {
    if (c.word("detain") === null) return null;
    const what = parseObjectRef(c);
    return what && { effect: "detain", what };
  });
}

function parseDayNightAction(c: Cursor): Effect | null {
  return c.attempt((c): Effect | null => {
    if (!c.words("it", "becomes")) return null;
    const to = c.word("day", "night");
    return to ? { effect: "set-day-night", to: to as "day" | "night" } : null;
  });
}

/** CR 710: only a flip card's own permanent flips. "Flip a coin" is a
 * different action (CR 705) and never matches an object reference. */
function parseFlipAction(c: Cursor): Effect | null {
  return c.attempt((c): Effect | null => {
    if (c.word("flip") === null) return null;
    const what = parseObjectRef(c);
    if (!what || !["self", "this", "it"].includes(what.ref)) return null;
    return { effect: "flip", what };
  });
}

// CR 701.24a: "shuffle that pile" randomizes the face-down pile just exiled.
function parseShufflePile(c: Cursor, deps: StatusActionDeps): Effect | null {
  return c.attempt((c): Effect | null => {
    const who = deps.actor(c, "shuffle", "shuffles");
    if (!who || !c.words("that", "pile")) return null;
    // Only the "..., then manifest/cloak those cards" continuation is
    // represented; "shuffle that pile and put it back ..." is not.
    const afterShuffle = c.pos;
    const then = c.punct(",") && c.words("then");
    c.pos = afterShuffle;
    return then ? { effect: "shuffle-pile", who } : null;
  });
}

export function parseStatusAction(c: Cursor, deps: StatusActionDeps): Effect[] | null {
  const effect = parseManifestAction(c, deps) ?? parseTurnFaceAction(c, deps) ?? parseSuspectAction(c) ??
    parseUnsuspectAction(c) ?? parsePrepareAction(c) ?? parseBecomeStatusAction(c) ?? parseDetainAction(c) ?? parseDayNightAction(c) ?? parseFlipAction(c) ?? parseShufflePile(c, deps);
  return effect ? [effect] : null;
}

// ---------------------------------------------------------------------------
// Face-down characteristics ("They're 2/2 Cyberman artifact creatures.")
// ---------------------------------------------------------------------------

function parseFaceDownCharacteristics(c: Cursor): FaceDownCharacteristics | null {
  return c.attempt((c): FaceDownCharacteristics | null => {
    const spec: FaceDownCharacteristics = { types: [], subtypes: [] };
    c.word("a", "an");
    const stats = c.attempt(c => {
      const power = c.number();
      if (power === null || !c.punct("/")) return null;
      const toughness = c.number();
      return toughness === null ? null : { power, toughness };
    });
    if (stats) {
      spec.power = { amount: "fixed", value: stats.power };
      spec.toughness = { amount: "fixed", value: stats.toughness };
    }
    for (;;) {
      const t = c.peek();
      if (!t || t.kind !== "word" || t.possessive) break;
      const type = singularize(t.value);
      if (CARD_TYPES.has(type)) { c.next(); spec.types.push(type); continue; }
      const subtype = canonicalSubtypeName(t.raw);
      if (!subtype || spec.types.length) break;
      c.next(); spec.subtypes.push(subtype);
    }
    return spec.types.length ? spec : null;
  });
}

type FaceDownProducer = Extract<Effect, { effect: "turn-face" } | { effect: "move-zone" }>;

function faceDownProducer(step: Effect | undefined): FaceDownProducer | null {
  if (step?.effect === "turn-face" && step.face === "down" && !step.characteristics) return step;
  if (step?.effect === "move-zone" && step.entry?.faceDown && !step.entry.faceDown.characteristics) return step;
  return null;
}

/** CR 708.2: the sentence "It's a 2/2 Cyberman artifact creature." completes
 * the characteristics of the object just turned or put face down; it is not
 * a separate characteristic-setting effect. It folds only into the single
 * preceding face-down instruction whose number agrees with the pronoun. */
export function foldFaceDownCharacteristics(c: Cursor, previous: Sentence): boolean {
  if (previous.steps.length !== 1 || previous.otherwise || previous.bindings) return false;
  const producer = faceDownProducer(previous.steps[0]);
  if (!producer || producer.forEach || producer.duration) return false;
  return c.attempt(c => {
    const pronoun = c.word("it's", "they're");
    if (!pronoun) return null;
    const singular = pronoun === "it's";
    if (isSingularRef(producer.what) !== singular) return null;
    const characteristics = parseFaceDownCharacteristics(c);
    if (!characteristics) return null;
    if (!c.punct(".") && !c.done()) return null;
    if (producer.effect === "turn-face") producer.characteristics = characteristics;
    else producer.entry!.faceDown!.characteristics = characteristics;
    return true;
  }) === true;
}

/** A stated characteristic directly after a face-turning instruction is that
 * instruction's face-down definition (CR 708.2) or nothing: it must never be
 * read as a free-standing characteristic-setting effect when it fails to fold. */
export function followsFaceInstruction(previous: Sentence): boolean {
  return previous.steps.some(step => step.effect === "turn-face" || faceDownProducer(step) !== null);
}

// ---------------------------------------------------------------------------
// Exert (CR 701.43)
// ---------------------------------------------------------------------------

/** "You may exert this creature as it attacks." is an optional cost to attack
 * (CR 701.43d, 508.1g); a "When you do" ability printed in the same
 * paragraph is linked to it and triggers only if the cost was paid.
 * The reflexive body keeps symbolic references ("it" is the exerted object). */
export function parseExertOnAttack(c: Cursor, parseBody: (c: Cursor) => Sentence[] | null): Extract<Ability, { kind: "exert-on-attack" }> | null {
  return c.attempt(c => {
    if (!c.words("you", "may", "exert")) return null;
    const what = parseObjectRef(c);
    if (!what || (what.ref !== "self" && what.ref !== "this")) return null;
    if (!c.words("as", "it", "attacks") || !c.punct(".")) return null;
    if (c.done()) return { kind: "exert-on-attack", what };
    if (!c.words("when", "you", "do") || !c.punct(",")) return null;
    const reflexive = parseBody(c);
    return reflexive && c.done() ? { kind: "exert-on-attack", what, reflexive } : null;
  });
}

// ---------------------------------------------------------------------------
// "... destroyed this way can't be regenerated" (CR 608.2c, 701.19c)
// ---------------------------------------------------------------------------

/** The rider names the objects destroyed by the preceding destruction. It is
 * accepted only when its single card type is certainly satisfied by every
 * destroyed object: "Destroy all lands or all creatures. Creatures destroyed
 * this way ..." protects only the creatures, which one flag cannot express. */
export function matchesDestroyedThisWay(what: ObjectRef, destroys: Extract<Effect, { effect: "destroy" }>[]): boolean {
  if (what.ref !== "each" && what.ref !== "related") return false;
  const { relation: filterRelation, ...rest } = what.filter;
  const relation = what.ref === "related" ? what.relation : filterRelation;
  if (relation?.relation !== "event-result" || relation.event !== "destroy" || relation.scope !== "this-way" ||
      relation.who || relation.period) return false;
  if (what.ref === "related" ? filterRelation !== undefined : false) return false;
  if (Object.keys(rest).length !== 1 || rest.types?.length !== 1) return false;
  const type = rest.types[0];
  return destroys.every(({ what: destroyed }) => {
    if (destroyed.ref === "target" || destroyed.ref === "filter" || destroyed.ref === "each" || destroyed.ref === "all" || destroyed.ref === "that") {
      const f = destroyed.filter;
      return f.types?.includes(type) === true && !f.anyOf && !f.allOf;
    }
    return false;
  });
}

// ---------------------------------------------------------------------------
// "As [this permanent] is turned face up, ..." (CR 708.11)
// ---------------------------------------------------------------------------

export function parseAsTurnedFaceUp(c: Cursor, parseBody: (c: Cursor) => Sentence[] | null): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    if (c.word("as") === null) return null;
    const what = parseObjectRef(c);
    if (!what || (what.ref !== "self" && what.ref !== "this")) return null;
    const alsoEnters = c.attempt(c => c.words("enters", "or") ? true : null) === true;
    if (!c.words("is", "turned", "face", "up") || !c.punct(",")) return null;
    const effects = parseBody(c);
    return effects && c.done() ? { static: "as-turned-face-up", what, effects, ...(alsoEnters ? { alsoEnters: true as const } : {}) } : null;
  });
}

/** "As this creature transforms into ~, …" (CR 701.27e): `~` is the face
 * that carries the text, as in the matching trigger event. */
export function parseAsTransforms(c: Cursor, parseBody: (c: Cursor) => Sentence[] | null): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    if (c.word("as") === null) return null;
    const what = parseObjectRef(c);
    if (!what || (what.ref !== "self" && what.ref !== "this")) return null;
    if (c.word("transforms") === null || c.word("into") === null || c.peek()?.kind !== "selfref") return null;
    c.pos++;
    if (!c.punct(",")) return null;
    const effects = parseBody(c);
    return effects && c.done() ? { static: "as-transforms", what, into: { face: "source-face" }, effects } : null;
  });
}
