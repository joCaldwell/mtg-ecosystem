// Token characteristics are copiable values (CR 111.3, 707.9). Entry state
// is separate: tapped/attacking do not become copied characteristics.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, BecomeSpec, CopyModification, GainedAbility, ObjectRef, PlayerRef,
  PredefinedTokenName, SpecifiedTokenSpec, TokenSpec } from "../ast.ts";
import { CARD_TYPES, COLORS, SUPERTYPES, canonicalSubtypeName, singularize } from "../vocab.ts";
import { Cursor } from "./cursor.ts";
import { isPluralPlayerRef, parseAmount, parseObjectRef, parsePlayerRef } from "./refs.ts";

type AbilityParser = (c: Cursor) => GainedAbility[] | null;
type CharacteristicParser = (c: Cursor) => BecomeSpec | null;
type EntryState = { tapped?: true; attacking?: true; attackingDefender?: PlayerRef | ObjectRef; blocking?: ObjectRef };
const PREDEFINED: PredefinedTokenName[] = ["Treasure", "Food", "Gold", "Walker", "Shard", "Clue", "Blood", "Powerstone", "Incubator", "Map", "Junk", "Lander", "Mutagen", "Vibranium", "Heartwood"];

function parseEntryState(c: Cursor, prefix: boolean): EntryState | null {
  return c.attempt(c => {
    const state: EntryState = {};
    const first = c.word("tapped", "attacking", "blocking");
    if (first === null || (prefix && first === "blocking")) return null;
    const note = (word: "tapped" | "attacking" | "blocking"): boolean => {
      if (word === "blocking") { if (state.blocking) return false; const blocked = blockedCreature(c); if (!blocked) return false; state.blocking = blocked; return true; }
      if (state[word]) return false;
      state[word] = true;
      // CR 508.4: an effect may specify what the entering creature attacks.
      if (word === "attacking" && !prefix) { const defender = attackedDefender(c); if (defender) state.attackingDefender = defender; }
      return true;
    };
    if (!note(first as "tapped" | "attacking" | "blocking")) return null;
    if (c.isWord("and")) {
      const next = c.peek(1);
      if (next?.kind === "word" && !next.possessive && (next.value === "tapped" || next.value === "attacking" || (next.value === "blocking" && !prefix))) {
        c.next(); c.next();
        if (!note(next.value)) return null;
      } else if (prefix || !next || (next.kind === "punct" && next.value === ".")) return null;
    }
    // Tapped-and-blocking or attacking-and-blocking are never both stated.
    if (state.attacking && state.blocking) return null;
    return state;
  });
}

/** "attacking that player|opponent": one named defending player (CR 508.4). */
function attackedDefender(c: Cursor): PlayerRef | ObjectRef | undefined {
  return c.attempt(c => {
    const player = parsePlayerRef(c);
    if (player) return isPluralPlayerRef(player) ? null : player;
    return null;
  }) ?? undefined;
}

/** "blocking that creature": the attacking creature it blocks (CR 509.4). */
function blockedCreature(c: Cursor): ObjectRef | null {
  return c.attempt(c => {
    const ref = parseObjectRef(c);
    return ref && singleton(ref) ? ref : null;
  });
}

function singleton(ref: ObjectRef): boolean {
  if (ref.ref === "all" || ref.ref === "each" || ref.ref === "them" || ref.ref === "zone-cards" || ref.ref === "rest") return false;
  if (ref.ref === "related") return ref.number === "singular";
  if ("count" in ref) return ref.count?.amount === "fixed" && ref.count.value === 1;
  return ref.ref !== "any-target";
}

function parseCopyModification(c: Cursor, parseAbilities: AbilityParser, parseCharacteristics: CharacteristicParser): CopyModification | null {
  return c.attempt(c => {
    if (c.words("it's", "not", "legendary") || c.words("it", "isn't", "legendary")
      || c.words("they're", "not", "legendary") || c.words("they", "aren't", "legendary")
      // The created token named by its noun instead of a pronoun.
      || c.words("the", "token", "isn't", "legendary") || c.words("the", "tokens", "aren't", "legendary")
      || c.words("the", "copy", "isn't", "legendary"))
      return { modification: "not-legendary" };
    const abilities = c.attempt(c => {
      if (!c.words("it", "has") && !c.words("they", "have") && c.word("has", "have") === null
        && !c.words("the", "token", "has") && !c.words("the", "tokens", "have")) return null;
      const abilities = parseAbilities(c);
      return abilities && { modification: "abilities" as const, abilities };
    });
    if (abilities) return abilities;
    if (!c.words("it's") && !c.words("it", "is") && !c.words("they're") && !c.words("they", "are")
      && !c.words("the", "token", "is") && !c.words("the", "tokens", "are")) return null;
    c.word("a", "an");
    const spec = parseCharacteristics(c);
    if (!spec) return null;
    if (c.words("in", "addition", "to")) {
      if (c.word("its", "their") === null || c.word("other") === null) return null;
      const inAddition = c.word("types", "colors");
      if (inAddition !== "types" && inAddition !== "colors") return null;
      if (inAddition === "types" && !spec.types?.length && !spec.subtypes?.length) return null;
      if (inAddition === "colors" && !spec.colors?.length) return null;
      spec.inAddition = inAddition;
    }
    return { modification: "characteristics", spec };
  });
}

function parseCopy(c: Cursor, parseAbilities: AbilityParser, parseCharacteristics: CharacteristicParser): TokenSpec | null {
  return c.attempt(c => {
    if (c.word("token", "tokens") === null) return null;
    if (!c.words("that's", "a", "copy", "of") && !c.words("that", "is", "a", "copy", "of")
      && !c.words("that", "are", "copies", "of")) return null;
    // A second relative clause can describe the new token or its model.
    // Only the explicit coordination in "a copy of that card and that's
    // tapped" establishes the former here. Keep it out of the model filter;
    // an uncoordinated predicate needs a separately supported scope grammar.
    let relative = -1;
    for (let offset = 0; ; offset++) {
      const next = c.peek(offset);
      if (!next || next.kind === "punct") break;
      if (next.kind !== "word") continue;
      const following = c.peek(offset + 1);
      if (next.value === "that's" || (next.value === "that" && following?.kind === "word"
        && ["is", "are"].includes(following.value))) { relative = offset; break; }
    }
    let copyOf: ObjectRef | null;
    if (relative >= 0) {
      const connector = c.peek(relative - 1);
      if (relative < 1 || connector?.kind !== "word" || connector.possessive || connector.value !== "and") return null;
      const source = new Cursor(c.tokens.slice(c.pos, c.pos + relative - 1));
      copyOf = parseObjectRef(source);
      if (!copyOf || !source.done()) return null;
      c.pos += relative - 1;
    } else copyOf = parseObjectRef(c);
    // A copy needs one model object per token. In particular, the card name
    // "Seven Dwarves" must not become a selector for seven Dwarf objects.
    if (!copyOf || !singleton(copyOf)) return null;
    // "a copy of that permanent attached to X" creates the token attached to X;
    // it must not be read as a description of the copied original.
    if (copyOf.ref === "that" && copyOf.filter.attachedTo) return c.fail("represented attachment of a token copy");
    // Likewise "a copy of the exiled card attacking that opponent" creates the
    // token attacking that player (CR 508.4); a token spec has no defender.
    if ((copyOf.ref === "that" || copyOf.ref === "related") && copyOf.filter?.attacking?.length)
      return c.fail("represented defender of a token copy entering attacking");
    const token: Extract<TokenSpec, { copyOf: ObjectRef }> = { copyOf };
    if (!c.attempt(c => c.punct(",") && c.word("except") !== null ? true : null)) return token;
    const first = parseCopyModification(c, parseAbilities, parseCharacteristics);
    if (!first) return null;
    token.modifications = [first];
    for (;;) {
      const connector = c.pos;
      const comma = c.punct(",");
      const and = c.word("and") !== null;
      if (!comma && !and) break;
      const modification = parseCopyModification(c, parseAbilities, parseCharacteristics);
      if (modification) { token.modifications.push(modification); continue; }
      // These subjects/verbs are still describing the copy exception. Never
      // leak an unsupported exception into an ordinary subsequent effect.
      if (c.isWord("it", "it's", "they", "they're", "the", "has", "have", "gains", "loses", "is", "are")
        || c.done() || c.isPunct(".")) return null;
      c.pos = connector;
      break;
    }
    return token;
  });
}

function parseSpecified(c: Cursor, parseAbilities: AbilityParser): SpecifiedTokenSpec | null {
  return c.attempt(c => {
    const spec: SpecifiedTokenSpec = { colors: [], supertypes: [], types: [], subtypes: [], keywords: [] };
    const parseStats = (c: Cursor) => c.attempt(c => {
      const value = (c: Cursor): Amount | null => {
        const n = c.number();
        if (n !== null) return { amount: "fixed", value: n };
        return c.word("x") !== null ? { amount: "x" } : null;
      };
      const power = value(c);
      if (!power || !c.punct("/")) return null;
      const toughness = value(c);
      if (!toughness) return null;
      spec.power = power; spec.toughness = toughness;
      return true;
    });
    parseStats(c);
    let colorless = false;
    let previousNominal = false;
    let serialColors = false;
    for (;;) {
      // Named legendary tokens use "a legendary 3/4 ..." (Stangg).
      // P/T belongs to the token definition, before its colors and types.
      if (!spec.power && !spec.colors.length && !spec.types.length && !spec.subtypes.length && parseStats(c)) continue;
      // Serial color lists ("red, green, and white"): a comma only joins
      // two colors, or a color and the list's final "and <color>".
      if (c.isPunct(",") && spec.colors.length && !spec.types.length && !spec.subtypes.length && !spec.supertypes.length
        && c.attempt(c => {
          c.next();
          const following = c.isWord("and") ? c.peek(1) : c.peek();
          return following?.kind === "word" && COLORS.has(following.value) ? true : null;
        })) { previousNominal = true; serialColors = true; continue; }
      const t = c.peek();
      if (!t || t.kind !== "word" || t.possessive) return null;
      if (serialColors && t.value !== "and" && !COLORS.has(t.value)) return null;
      if (t.value === "token" || t.value === "tokens") {
        if (!previousNominal) return null;
        c.next(); break;
      }
      if (t.value === "and") {
        if (!previousNominal) return null;
        c.next(); previousNominal = false; serialColors = false; continue;
      }
      if (COLORS.has(t.value)) {
        if (colorless) return null;
        c.next(); spec.colors.push(t.value as SpecifiedTokenSpec["colors"][number]); previousNominal = true; continue;
      }
      if (t.value === "colorless") {
        if (spec.colors.length || colorless) return null;
        colorless = true; c.next(); previousNominal = true; continue;
      }
      if (SUPERTYPES.has(t.value)) { c.next(); spec.supertypes.push(t.value); previousNominal = true; continue; }
      const singular = singularize(t.value);
      if (CARD_TYPES.has(singular)) { c.next(); spec.types.push(singular); previousNominal = true; continue; }
      if (c.words("time", "lord")) { spec.subtypes.push("Time Lord"); previousNominal = true; continue; }
      const subtype = canonicalSubtypeName(t.raw);
      if (!subtype) return null;
      c.next(); spec.subtypes.push(subtype); previousNominal = true;
    }
    if (!spec.types.length) return null;
    const parseName = (): boolean => {
      if (c.word("named") === null) return true;
      if (spec.name) return false;
      const name = c.next();
      if (name?.kind !== "card-name") return false;
      spec.name = name.value;
      return true;
    };
    // CR 111.4: the explicitly specified name is a copiable characteristic,
    // independent of the subtype list. Oracle places it before or after the
    // ability clause (e.g. Jungle Patrol's "with defender named Wood").
    if (!parseName()) return null;
    if (c.word("with") !== null) {
      const abilities = parseAbilities(c);
      if (!abilities) return null;
      for (const ability of abilities) {
        if (ability.gained === "keyword") spec.keywords.push(ability.keyword);
        else (spec.abilities ??= []).push(ability);
      }
    }
    if (!parseName()) return null;
    // CR 105.2, 111.3: "that's all colors" defines the token's colors in a
    // trailing clause instead of the adjective list.
    if (c.attempt(c => (c.words("that's") || c.words("that", "is") || c.words("that", "are")) && c.words("all", "colors") ? true : null)) {
      if (spec.colors.length || colorless) return null;
      spec.colors = ["white", "blue", "black", "red", "green"];
    }
    return spec;
  });
}

/** The callbacks share effects.ts's quoted-ability and characteristic grammar
 * without importing that module or conflating copy exceptions with effects. */
export function parseTokenSpec(c: Cursor, parseAbilities: AbilityParser, parseCharacteristics: CharacteristicParser): TokenSpec | null {
  return c.attempt(c => {
    const prefix = c.isWord("tapped", "attacking") ? parseEntryState(c, true) : {};
    if (!prefix) return null;
    const token = parseCopy(c, parseAbilities, parseCharacteristics) ?? c.attempt((c): TokenSpec | null => {
      const name = c.anyWord();
      const canonical = name && PREDEFINED.find(value => value.toLowerCase() === name.value);
      if (!canonical || c.word("token", "tokens") === null) return null;
      return { predefined: canonical };
    }) ?? parseSpecified(c, parseAbilities);
    if (!token) return null;
    const suffix = c.attempt(c => {
      // Copy suffixes use the explicit coordinating relative clause; otherwise
      // a relative clause could instead qualify the model object being copied.
      if ("copyOf" in token && c.word("and") === null) return null;
      if (!c.words("that's") && !c.words("that", "is") && !c.words("that", "are")) return null;
      return parseEntryState(c, false);
    });
    if (suffix && ((suffix.tapped && prefix.tapped) || (suffix.attacking && prefix.attacking))) return null;
    return { ...token, ...prefix, ...(suffix ?? {}) };
  });
}

/** "Create a black Zombie creature token. Its power is equal to that card's
 * power and its toughness is equal to that card's toughness." The sentence
 * completes the created token's definition (CR 111.3): its power and
 * toughness are set as it is created, like "an X/X … token, where X is …". */
export function parseTokenStatsSentence(c: Cursor): { power: Amount; toughness: Amount } | null {
  return c.attempt(c => {
    if (!c.words("its", "power", "is", "equal", "to")) return null;
    const power = parseAmount(c);
    if (!power || !c.words("and", "its", "toughness", "is", "equal", "to")) return null;
    const toughness = parseAmount(c);
    if (!toughness || (!c.punct(".") && !c.done())) return null;
    return { power, toughness };
  });
}
