// Shared reference grammar: amounts, filters, object/player refs, zones,
// durations, conditions, counters. Everything here is strictly typed and
// backtracking — a function either returns a complete node or null.

import type {
  Amount, Color, CombatDefender, Comparison, Condition, CounterSpec, Duration, GroupConstraint, ObjectFilter,
  ObjectRef, ObjectRelation, PlayerFilter, PlayerRef, PropertyConstraint, TargetSelection, ZoneRef,
} from "../ast.ts";
import type { Token } from "../lexer.ts";
import {
  CARD_TYPES, COLORS, OBJECT_CLASSES, STATUS_WORDS, SUPERTYPES, ZONES,
  canonicalSubtypeName, singularize, wordNumber,
} from "../vocab.ts";
import { isManaSymbol } from "../symbols.ts";
import { Cursor } from "./cursor.ts";
import { parseCharacteristicRelationQualifier } from "./characteristic-relations.ts";
import { parseConditionExpression } from "./conditions.ts";
import { parseBlockHistoryQualifier, parseCombatActionHistoryPredicate, parseDamageHistoryPredicate } from "./history-filters.ts";
import { parseDiceResultAmount } from "./dice.ts";
import { hasCombatHistory, parseAggregateAmount } from "./aggregates.ts";
import { parseQuantity } from "./quantities.ts";
import { parseComparativePlayerQualifier } from "./comparatives.ts";
import { parseGroupConstraint, validSelectionCount } from "./selection-constraints.ts";
import { parseCardExtraction as parseCardExtractionReference } from "./card-selections.ts";
import { parseBareTargets, parseEachOf, parseTheOther } from "./quantified-selections.ts";
import { parseVoteCountAmount, parseVoteLeaders } from "./votes.ts";
import { parseSpeedAmount } from "./keyword-actions.ts";
// Keyword-name matching is invoked only while parsing, after module setup.
// The shared registry also supplies keywords.ts; no duplicate keyword list.
import { parseKeywordName } from "./keywords.ts";
import { parseOrThatObjectsController, parseOrThatPlayer } from "./player-refs.ts";

/** Mint an identity from the actual target word, never the cursor offset of a
 * sliced phrase or the structure of its filter. CR 601.2c distinguishes target
 * occurrences even when their filters are identical.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
export function targetSelectionProvenance(token: Token | undefined): TargetSelection {
  return token?.kind === "word" && !token.possessive && ["target", "targets"].includes(token.value) && token.provenance
    ? { selectionId: `${token.provenance.sourceId}:${token.provenance.index}` } : {};
}

// ---------------------------------------------------------------------------
// Amounts
// ---------------------------------------------------------------------------

/** "a"/"an"/"three"/"3"/"X" — counting words used before nouns. */
export function parseCount(c: Cursor): Amount | null {
  return c.attempt((c): Amount | null => {
    const n = c.number();
    if (n !== null) return { amount: "fixed", value: n };
    if (c.word("a", "an") !== null) return { amount: "fixed", value: 1 };
    if (c.word("x") !== null) return { amount: "x" };
    const t = c.peek();
    if (t?.kind === "word" && !t.possessive) {
      const wn = wordNumber(t.value);
      if (wn !== undefined) {
        c.pos++;
        return { amount: "fixed", value: wn };
      }
    }
    return c.fail("count");
  });
}

/** Amount in effect position: "3", "X", "twice X", "half your life total, rounded up" … */
export function parseAmount(c: Cursor): Amount | null {
  return c.attempt(c => {
    const amount = parseAmountTerm(c);
    return amount ? parseAmountAdjustment(c, amount) : null;
  });
}

/** Arithmetic retains its operands; repeated additions/subtractions associate
 * left-to-right. Multipliers bind to one term before this additive suffix. */
export function parseAmountAdjustment(c: Cursor, initial: Amount): Amount | null {
  return c.attempt(c => {
    let result = initial;
    for (;;) {
      const operator = c.word("plus", "minus");
      if (!operator) return result;
      const other = parseAmountTerm(c);
      if (!other) return null;
      result = { amount: operator as "plus" | "minus", a: result, b: other };
    }
  });
}

function parseAmountTerm(c: Cursor): Amount | null {
  return c.attempt((c): Amount | null => {
    if (c.punct("-")) {
      const value = c.number();
      return value === null ? null : { amount: "fixed", value: -value };
    }
    if (c.words("twice")) {
      const of = parseAmountTerm(c);
      if (!of) return null;
      return { amount: "twice", of };
    }
    if (c.words("half")) {
      const of = parseAmountTerm(c);
      if (!of) return null;
      c.punct(",");
      // CR 107.1: fractions must not acquire an invented rounding rule.
      // Oracle instructions state the direction when division can occur.
      if (!c.words("rounded")) return null;
      const round = c.word("up", "down") as "up" | "down" | null;
      if (!round) return null;
      return { amount: "half", of, round };
    }
    const preventionQuantity = c.attempt((c): Amount | null => {
      c.word("the");
      return c.words("damage", "prevented", "this", "way")
        ? { amount: "prevention-event-quantity", quantity: "prevented" } : null;
    });
    if (preventionQuantity) return preventionQuantity;
    if (c.words("that", "much") || c.words("that", "many")) return { amount: "that-much" };
    const voteCount = parseVoteCountAmount(c) ?? parseSpeedAmount(c);
    if (voteCount) return voteCount;
    const diceResult = parseDiceResultAmount(c);
    if (diceResult) return diceResult;
    if (c.words("that", "number")) return { amount: "that-number" };
    const quantity = parseQuantity(c, { parseObjectRef, parsePlayerRef, parsePossessivePlayer, isPluralPlayerRef });
    if (quantity) return quantity;
    const attr = c.attempt(parseAttributeAmount);
    if (attr) return attr;
    const extremum = parseAttributeExtremum(c);
    if (extremum) return extremum;
    const aggregate = parseAggregateAmount(c, { parseObjectRef });
    if (aggregate) return aggregate;
    if (c.attempt((c) => (c.words("the", "number", "of") ? true : null))) {
      const counters = c.attempt((c): Amount | null => {
        // Counter kinds are distinct from the number of counters (CR 122.1).
        if (c.isWord("different", "kinds", "types", "each", "of")) return null;
        const counter = c.isWord("counter", "counters") ? "any" : parseCounterSpec(c);
        if (!counter || c.word("counter", "counters") === null) return null;
        if (c.word("on") !== null) {
          const on = parsePlayerRef(c) ?? parseObjectRef(c);
          return on && { amount: "counter-count", counter, on };
        }
        // CR 122.1: player counters belong to that player, including
        // experience, poison, and energy counters. "Have" is current state;
        // past-tense possession needs an explicit history representation.
        const on = parsePlayerRef(c);
        // "Each player has" denotes separately evaluated values, not an
        // implicitly summed total. Wait for an explicit per-player binding.
        return on && !isPluralPlayerRef(on) && c.word("have", "has") !== null ? { amount: "counter-count", counter, on } : null;
      });
      if (counters) return counters;
      const of = parseFilter(c);
      if (!of || hasCombatHistory(of)) return null;
      return { amount: "count", of };
    }
    const base = c.attempt(parseCount);
    if (base) return base;
    return c.fail("amount");
  });
}

/** CR 107.2: numeric information, including maxima/minima, remains symbolic
 * until evaluated. This is a value comparison, never an arbitrary choice of
 * one object with that value (ties are preserved by the consuming filter).
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
function parseAttributeExtremum(c: Cursor): Extract<Amount, { amount: "attribute-extremum" }> | null {
  return c.attempt(c => {
    c.word("the");
    const adjective = c.word("greatest", "highest", "least", "lowest");
    if (!adjective) return null;
    const attribute = c.words("mana", "value") ? "mana-value" : c.word("power", "toughness");
    if (attribute !== "mana-value" && attribute !== "power" && attribute !== "toughness") return null;
    if (c.word("among") === null) return null;
    const among = parseFilter(c);
    return among && { amount: "attribute-extremum", extremum: adjective === "greatest" || adjective === "highest" ? "maximum" : "minimum", attribute, among };
  });
}

/** Possessive object phrases retain their referenced object, including target
 * identity when present. Only one explicitly singular object is supported;
 * plural possessors need distributive semantics, not an arbitrary singleton.
 * CR 109.4 and 608.2h keep controller information dependent on that object.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
export function parsePossessiveObject(c: Cursor): ObjectRef | null {
  return c.attempt(c => {
    if (c.selfrefPossessive()) return { ref: "self" };
    if (c.word("its") !== null) return { ref: "it" };
    if (!c.isWord("this", "that", "target", "enchanted", "equipped", "the")) return null;
    // Split at the first possessive head only. Parsing a later possessive
    // would accidentally absorb a nested player's qualifier as the possessor.
    let offset = 0;
    while (true) {
      const token = c.peek(offset);
      if (!token || token.kind === "punct") return null;
      if (token.kind === "word" && token.possessive) break;
      offset++;
    }
    const head = c.peek(offset);
    if (head?.kind !== "word") return null;
    const tokens = c.tokens.slice(c.pos, c.pos + offset);
    tokens.push({ ...head, possessive: false });
    const subject = new Cursor(tokens);
    const of = parseObjectRef(subject);
    if (!of || !subject.done()) return null;
    const singular = of.ref === "this" || of.ref === "enchanted" || of.ref === "equipped"
      || (of.ref === "that" && !of.plural && head.value === singularize(head.value))
      || (of.ref === "related" && of.number === "singular")
      || (of.ref === "target" && of.count.amount === "fixed" && of.count.value === 1 && !of.upTo
        && head.value === singularize(head.value));
    if (!singular) return null;
    c.pos += offset + 1;
    return of;
  });
}

/** "its power", "that creature's power", "enchanted player's life total" */
function parseAttributeAmount(c: Cursor): Amount | null {
  return c.attempt((c): Amount | null => {
    const life = c.attempt((c): Amount | null => {
      const of = parsePossessivePlayer(c);
      if (of && c.words("starting", "life", "total")) return { amount: "starting-life-total", of };
      return of && c.words("life", "total") ? { amount: "life-total" as const, of } : null;
    });
    if (life) return life;
    const of = parsePossessiveObject(c);
    if (!of) return null;
    const attr = c.words("mana", "value")
      ? "mana-value"
      : (c.word("power", "toughness") as "power" | "toughness" | null);
    if (!attr) return null;
    return { amount: "attribute", of, attribute: attr };
  });
}

/** "equal to <amount>" tail used by damage/life effects. */
export function parseEqualTo(c: Cursor): Amount | null {
  return c.attempt((c): Amount | null => {
    if (!c.words("equal", "to")) return null;
    return parseAmount(c);
  });
}

// ---------------------------------------------------------------------------
// Comparisons
// ---------------------------------------------------------------------------

/** "3 or greater", "2 or less", "X", "less than 4", "exactly 2" */
export function parseComparison(c: Cursor): Comparison | null {
  return c.attempt((c): Comparison | null => {
    if (c.words("at", "least")) {
      const value = parseAmount(c);
      return value && { op: "ge", value };
    }
    if (c.words("at", "most")) {
      const value = parseAmount(c);
      return value && { op: "le", value };
    }
    if (c.words("less", "than", "or", "equal", "to")) {
      const value = parseAmount(c);
      return value && { op: "le", value };
    }
    if (c.words("greater", "than", "or", "equal", "to")) {
      const value = parseAmount(c);
      return value && { op: "ge", value };
    }
    if (c.words("equal", "to")) {
      const value = parseAmount(c);
      return value && { op: "eq", value };
    }
    if (c.words("less", "than") || c.words("fewer", "than")) {
      const v = parseAmount(c);
      return v && { op: "lt", value: v };
    }
    if (c.words("greater", "than") || c.words("more", "than")) {
      const v = parseAmount(c);
      return v && { op: "gt", value: v };
    }
    if (c.words("exactly")) {
      const v = parseAmount(c);
      return v && { op: "eq", value: v };
    }
    const v = c.attempt(parseAmount);
    if (!v) return c.fail("comparison");
    if (c.attempt((c) => (c.word("or") !== null && c.word("greater", "more") !== null ? true : null)))
      return { op: "ge", value: v };
    if (c.attempt((c) => (c.word("or") !== null && c.word("less", "fewer") !== null ? true : null)))
      return { op: "le", value: v };
    return { op: "eq", value: v };
  });
}

// ---------------------------------------------------------------------------
// Zones
// ---------------------------------------------------------------------------

/** "the battlefield", "your graveyard", "its owner's hand", "a graveyard" … */
export function parseZoneRef(c: Cursor): ZoneRef | null {
  return c.attempt((c): ZoneRef | null => {
    const positioned = c.attempt((c): ZoneRef | null => {
      c.word("the");
      const position = c.word("top", "bottom");
      if (!position || c.word("of") === null) return null;
      const zone = parseZoneRef(c);
      if (zone?.zone !== "library" || zone.position) return null;
      return { ...zone, position: position as "top" | "bottom" };
    });
    if (positioned) return positioned;
    if (c.words("the", "battlefield")) return { zone: "battlefield" };
    if (c.words("the", "command", "zone") || c.words("command", "zone")) return { zone: "command" };
    let owner: ZoneRef["owner"];
    const dependent = c.attempt(c => {
      const who = parsePossessivePlayer(c);
      // Retain the established compact spelling for "its owner's" below.
      if (who?.player === "owner" && who.of.ref === "it") return null;
      return who && ["controller", "owner", "enchanted-player", "chosen-player", "defending-player"].includes(who.player) ? who : null;
    });
    const quantified = dependent ? null : parseQuantifiedPersonalZone(c);
    if (quantified) return quantified;
    if (dependent) owner = dependent;
    else if (c.word("your") !== null) owner = "your";
    else if (c.word("their") !== null || c.word("its") !== null) {
      // "their owner's hand" / "its owner's graveyard"
      if (c.possessive("owner", "owners") !== null) owner = "its-owner";
      else owner = "their";
    } else if (c.attempt((c) => (c.word("an") !== null && c.possessive("opponent") !== null ? true : null)))
      owner = "an-opponent";
    else if (c.attempt((c) => (c.word("each") !== null && c.possessive("opponent") !== null ? true : null)))
      owner = "each-opponent";
    else if (c.attempt((c) => (c.word("each") !== null && c.possessive("player") !== null ? true : null)))
      owner = "each-player";
    else if (c.attempt((c) => (c.word("that") !== null && c.possessive("player") !== null ? true : null)))
      owner = "that-player";
    else if (c.attempt((c) => (c.word("the") !== null && c.possessive("player") !== null ? true : null)))
      owner = { player: "the-player" };
    else if (c.isWord("target")) {
      const targeted = parsePossessivePlayer(c);
      if (!targeted || (targeted.player !== "target-player" && targeted.player !== "target-opponent")) return null;
      owner = targeted;
    }
    else if (c.word("a", "an", "any") !== null) owner = "any";
    else if (c.word("the") !== null) owner = undefined;
    const zoneWord = c.anyWord();
    if (!zoneWord) return null;
    const zone = singularize(zoneWord.value);
    if (!ZONES.has(zone)) return c.fail("zone");
    return { zone: zone as ZoneRef["zone"], owner };
  });
}

/** CR 400.1: each player has their own library, hand and graveyard, so a
 * quantified zone phrase quantifies over those players. Pooled ("all
 * graveyards"), distributive ("each graveyard") and same-zone ("a single
 * graveyard") readings stay distinct; grammatical number must agree.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
function parseQuantifiedPersonalZone(c: Cursor): ZoneRef | null {
  return c.attempt((c): ZoneRef | null => {
    let owner: ZoneRef["owner"];
    let plural: boolean;
    if (c.word("all") !== null) { owner = "all-players"; plural = true; }
    else if (c.word("each") !== null) { owner = "each-player"; plural = false; }
    else if (c.words("a", "single")) { owner = "any-single"; plural = false; }
    else if (c.word("your") !== null && c.possessive("opponents") !== null) { owner = "all-opponents"; plural = true; }
    else return null;
    const zone = c.word(...(plural ? ["graveyards", "libraries", "hands"] : ["graveyard", "library", "hand"]));
    return zone ? { zone: singularize(zone) as ZoneRef["zone"], owner } : null;
  });
}

// ---------------------------------------------------------------------------
// Filters
// ---------------------------------------------------------------------------

type NominalClass = "type" | "class" | "subtype" | "player";

/** Canonical subtype form: singularized, first letter capitalized ("Elves" → "Elf"). */
export function canonicalSubtype(raw: string): string {
  const subtype = canonicalSubtypeName(raw);
  if (!subtype) throw new Error(`Not a recognized subtype: ${raw}`);
  return subtype;
}

function classifyNominal(value: string, raw: string): { cls: NominalClass; word: string } | null {
  const singular = singularize(value);
  if (CARD_TYPES.has(singular)) return { cls: "type", word: singular };
  if (OBJECT_CLASSES.has(singular)) return { cls: "class", word: singular };
  if (singular === "player" || singular === "opponent") return { cls: "player", word: singular };
  const subtype = canonicalSubtypeName(raw);
  if (subtype) return { cls: "subtype", word: subtype };
  return null;
}

/** May these two nominal classes appear in one or-list? ("creature or player") */
function orCompatible(a: NominalClass, b: NominalClass): boolean {
  if (a === "class" || b === "class") return a === b;
  return true;
}

/** Repeated plural card heads under one enclosing count denote a union of
 * candidate classes, not independently counted selections. Keep prenominal
 * restrictions in their own branch; trailing qualifiers apply to the union.
 * Circuitous Route/Map the Frontier use "basic land cards and/or Gate cards".
 * CR 608.2c; https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
function parseRepeatedCardUnionHead(c: Cursor): ObjectFilter | null {
  return c.attempt(c => {
    const heads: { start: number; end: number }[] = [];
    let finalJoin = false;
    for (;;) {
      const start = c.pos;
      for (;;) {
        const token = c.peek();
        if (token?.kind !== "word" || token.possessive) return null;
        c.pos++;
        if (token.value === "cards") break;
        // Independent articles/counts are selection slots, not predicates.
        if (["a", "an", "another", "target", "each", "all"].includes(token.value)) return null;
      }
      heads.push({ start, end: c.pos });
      const next = c.attempt(c => {
        const comma = c.punct(",");
        const join = parseLogicalJoin(c);
        if (!comma && !join) return null;
        if (join && join !== "and-or") return null;
        if (join) finalJoin = true;
        // An effect-list comma after the final union member belongs outside.
        if (finalJoin && !join) return null;
        return true;
      });
      if (!next) break;
    }
    if (heads.length < 2 || !finalJoin) return null;
    const branches: ObjectFilter[] = [];
    for (const head of heads) {
      const sub = new Cursor(c.tokens.slice(head.start, head.end));
      const filter = parseFilterExpression(sub, false);
      if (!filter || !sub.done() || filter.cls !== "card") return null;
      const { cls: _cls, ...branch } = filter;
      branches.push(branch);
    }
    return { cls: "card", anyOf: branches };
  });
}

/** Nashi: "legendary and/or enchantment cards" unions different kinds of
 * characteristics. Adjacent "legendary enchantment cards" stays conjunctive. */
function parseMixedCardAttributeUnionHead(c: Cursor): ObjectFilter | null {
  return c.attempt(c => {
    const atom = (c: Cursor): ObjectFilter | null => {
      const word = c.anyWord();
      if (!word) return null;
      if (SUPERTYPES.has(word.value)) return { supertypes: [word.value] };
      if (CARD_TYPES.has(word.value)) return { types: [word.value] };
      return null;
    };
    const first = atom(c);
    if (!first || parseLogicalJoin(c) !== "and-or") return null;
    const second = atom(c);
    if (!second || !!first.types === !!second.types || c.word("cards") === null) return null;
    return { cls: "card", anyOf: [first, second] };
  });
}

/** An OR can coordinate modifiers of a shared head ("instant or sorcery
 * cards") or independently qualified noun phrases ("legendary creature or
 * creature with mana value one"). Repeating the head, an article, or a
 * prenominal qualifier starts a new predicate term. A compound whose scope
 * cannot be distinguished from a shared suffix is deliberately unsupported.
 * CR 608.2c requires the complete English instruction to retain its meaning.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
/** Preserve the lexical conjunction until its grammatical role is known.
 * A union of candidate predicates is not a list of independent choices. */
function parseLogicalJoin(c: Cursor): "and" | "or" | "and-or" | null {
  return c.attempt(c => {
    if (c.word("and") !== null) {
      if (!c.punct("/")) return "and";
      return c.word("or") !== null ? "and-or" : null;
    }
    return c.word("or") !== null ? "or" : null;
  });
}

/** A relative predicate can enumerate singular categories with repeated
 * articles: Lovisa's "that's a Barbarian, a Warrior, or a Berserker".
 * Require the explicit final OR before treating any intermediate comma as
 * union; repeated articles in ordinary selections still denote separate slots. */
function hasArticleNominalOrTail(c: Cursor): boolean {
  const probe = new Cursor(c.tokens.slice(c.pos));
  for (;;) {
    if (probe.word("a", "an") === null) return false;
    const nominal = probe.peek();
    if (nominal?.kind !== "word" || nominal.possessive || !classifyNominal(nominal.value, nominal.raw)) return false;
    probe.pos++;
    const comma = probe.punct(",");
    const join = parseLogicalJoin(probe);
    if (join === "or") return true;
    if (join || !comma) return false;
  }
}

function nominalOrScope(c: Cursor, previous: { cls: NominalClass; word: string }): "shared" | "branch" | "ambiguous" {
  const start = c.isPunct(",") ? 1 : 0;
  const probe = new Cursor(c.tokens.slice(c.pos + start));
  const join = parseLogicalJoin(probe);
  if (join !== "or" && join !== "and-or") return "shared";
  const afterJoin = start + probe.pos;
  const first = c.peek(afterJoin);
  if (first?.kind !== "word" || first.possessive) return "shared";
  if (["a", "an", "another", "other", "colorless", "multicolored", "monocolored", "activated", "triggered", "static", "mana", "loyalty"].includes(first.value)
    || COLORS.has(first.value) || SUPERTYPES.has(first.value) || STATUS_WORDS.has(first.value)
    || first.value.startsWith("non")) return "branch";
  const nouns: { cls: NominalClass; word: string }[] = [];
  for (let offset = afterJoin; ; offset++) {
    const token = c.peek(offset);
    if (token?.kind !== "word" || token.possessive) break;
    const nominal = classifyNominal(token.value, token.raw);
    if (!nominal) break;
    nouns.push(nominal);
  }
  if (nouns.some(noun => noun.cls === previous.cls && noun.word === previous.word)) return "branch";
  // A class head can qualify either card type; a type/class head can qualify
  // either subtype. A second alternative with further same-level modifiers
  // needs explicit grouping instead of intersecting them with both branches.
  if (nouns.slice(1).some(noun => noun.cls !== "class" && !(previous.cls === "subtype" && noun.cls === "type"))) return "ambiguous";
  return "shared";
}

function hasFilterScope(filter: ObjectFilter, key: "zone" | "control" | "controlledBy" | "own" | "cast" | "fromAmong" | "relation" | "attachedTo" | "blocking" | "blockedBy" | "abilitySource"): boolean {
  return filter[key] !== undefined || (filter.allOf ?? []).some(term => hasFilterScope(term, key))
    || (filter.anyOf ?? []).some(term => hasFilterScope(term, key));
}

function joinFilterBranches(first: ObjectFilter, second: ObjectFilter): ObjectFilter | null {
  // "legendary creatures or artifact creatures you control" does not state
  // unambiguously whether the final control restriction covers both branches.
  // Do not turn it into either unrestricted legendary creatures or a shared
  // controller without a separately supported grammar for that scope.
  for (const key of ["zone", "control", "controlledBy", "own", "cast", "fromAmong", "relation", "attachedTo", "blocking", "blockedBy", "abilitySource"] as const) {
    if (!hasFilterScope(first, key) && hasFilterScope(second, key)) return null;
  }
  // Leading "another/other" excludes the source from the whole selection,
  // including both alternatives in Isamaru and Yoshimaru's trigger.
  const { other, ...term } = first;
  return { ...(other ? { other } : {}), anyOf: [term, second] };
}

/** CR 113.3, 605.1 and 606.1: ability category, mana-ability status,
 * loyalty-ability status, and source identity are separate constraints. */
function parseAbilityHead(c: Cursor): ObjectFilter | null {
  return c.attempt(c => {
    const f: ObjectFilter = { cls: "ability" };
    const mana = c.word("mana", "nonmana", "non-mana");
    if (mana) f.manaAbility = mana === "mana";
    // CR 702.142a, 702.177a, 702.49, 702.193: abilities named by their keyword.
    const keyword = mana ? null : c.word("boast", "exhaust", "ninjutsu", "power-up");
    if (keyword) {
      if (c.word("ability", "abilities") === null) return null;
      f.abilityKeyword = keyword;
      return f;
    }
    const kind = c.word("activated", "triggered", "static");
    if (kind) {
      const kinds = [kind as "activated" | "triggered" | "static"];
      let connector: "and" | "or" | undefined;
      let mixed = false;
      while (c.attempt(c => {
        const join = c.word("or", "and") as "or" | "and" | null;
        if (!join) return null;
        const next = c.word("activated", "triggered", "static");
        if (!next) return null;
        if (connector && connector !== join) { mixed = true; return null; }
        connector = join;
        kinds.push(next as "activated" | "triggered" | "static");
        return true;
      })) { /* category lists select alternative ability kinds */ }
      const head = c.word("ability", "abilities");
      if (mixed || !head || (connector === "and" && head !== "abilities")) return null;
      if (kinds.length === 1) f.abilityKind = kinds[0];
      else f.anyOf = kinds.map(abilityKind => ({ abilityKind }));
      return f;
    }
    if (c.word("loyalty") !== null) f.loyaltyAbility = true;
    if ((!mana && !f.loyaltyAbility) || c.word("ability", "abilities") === null) return null;
    return f;
  });
}

/** A coordinated qualifier after a complete relational operand can modify
 * either that operand or the outer subject. Keep that scope unsupported;
 * prenominal conjunctions ("a red and green creature") remain unambiguous. */
function parseRelationObject(c: Cursor): ObjectRef | null {
  return c.attempt(c => {
    const start = c.pos;
    const object = parseObjectRef(c);
    if (!object) return null;
    // These predicates test membership, not a new chosen group or a
    // universal relationship. Explicit cardinalities/universals require a
    // separate relation quantifier; do not erase them into membership.
    const first = c.tokens[start];
    if (first?.kind === "word" && first.value === "each" && object.ref !== "each") return null;
    if (object.ref === "all" || (object.ref === "each" && first?.kind === "word" && first.value === "each")
      || ("upTo" in object && object.upTo)
      || ("count" in object && object.count && (object.count.amount !== "fixed" || object.count.value !== 1))) return null;
    for (let end = start + 1; end < c.pos; end++) {
      const token = c.tokens[end];
      if (token.kind !== "word" || token.possessive || token.value !== "and") continue;
      const before = new Cursor(c.tokens.slice(start, end));
      if (parseObjectRef(before) && before.done()) return null;
    }
    return object;
  });
}

/** The relation is directed from each matching object to the referenced
 * object(s); a filter/set operand means at least one member must be related.
 * CR 701.3/303.4 (attachment), 509.1g-h (blocking vs being blocked). */
function parseObjectRelationPredicate(c: Cursor): ObjectFilter | null {
  return c.attempt(c => {
    if (c.words("attached", "to")) {
      const attachedTo = parsePlayerRef(c) ?? parseRelationObject(c);
      return attachedTo && { attachedTo };
    }
    if (c.word("attacking") !== null) {
      // CR 508.1b / 506.4c: the player or permanent each matching creature
      // is attacking; printed alternatives are retained separately.
      const defender = (c: Cursor): CombatDefender | null => c.attempt((c): CombatDefender | null => {
        const who = parsePlayerRef(c);
        if (who) return isPluralPlayerRef(who) ? null : { defender: "player", who };
        const what = parseRelationObject(c);
        // A bare plural pronoun may stand for players ("attacking them").
        return what && what.ref !== "them" ? { defender: "object", what } : null;
      });
      const firstDefender = defender(c);
      if (!firstDefender) return null;
      const attacking = [firstDefender];
      while (c.attempt(c => {
        if (c.word("or") === null) return null;
        const next = defender(c);
        if (!next) return null;
        attacking.push(next);
        return true;
      })) { /* e.g. you or a planeswalker you control */ }
      return { attacking };
    }
    const first = c.word("blocking", "blocked");
    if (first) {
      const alternatives = [first];
      const joined = c.attempt(c => {
        if (c.word("or") === null) return null;
        c.word("being");
        const second = c.word("blocking", "blocked");
        return second && second !== first ? second : null;
      });
      if (joined) alternatives.push(joined);
      if (alternatives[alternatives.length - 1] === "blocked" && c.word("by") === null) return null;
      const object = parseRelationObject(c);
      if (!object) return null;
      const predicates = alternatives.map(direction => direction === "blocking" ? { blocking: object } : { blockedBy: object });
      return predicates.length === 1 ? predicates[0] : { anyOf: predicates };
    }
    const inverse = c.attempt(c => {
      if (c.words("it's", "blocking")) return { ref: "it" as const };
      if (!c.isWord("this", "that", "it") && c.peek()?.kind !== "selfref") return null;
      const object = parseObjectRef(c);
      return object && c.word("is") !== null && c.word("blocking") !== null ? object : null;
    });
    return inverse && { blockedBy: inverse };
  });
}

/**
 * Structured card/object filter: "tapped artifact", "nonbasic land",
 * "instant or sorcery card in your graveyard", "Elves you control", …
 * Consumes at least one nominal (type/class/subtype) or fails.
 */
export function parseFilter(c: Cursor): ObjectFilter | null {
  return parseFilterExpression(c, false);
}

/** A relative predicate may be adjective-only ("that's white or blue").
 * Ordinary object selections still require a noun. */
function parseFilterExpression(c: Cursor, predicate: boolean): ObjectFilter | null {
  return c.attempt((c): ObjectFilter | null => {
    const filterStart = c.pos;
    const repeated = parseRepeatedCardUnionHead(c) ?? parseMixedCardAttributeUnionHead(c);
    const f: ObjectFilter = repeated ?? {};
    let sawNominal = !!repeated;
    let independentAlternative = false;

    const addColor = (color: Color) => {
      (f.colors ??= []).push(color);
    };

    // Prenominal + nominal loop.
    outer: for (;;) {
      // Comma between prenominal qualifiers: "noncreature, nonland permanent".
      if (c.isPunct(",") && !sawNominal) {
        // A comma separates qualifiers; it never begins a noun phrase.
        if (c.pos === filterStart) break;
        const consumed = c.attempt((c) => {
          c.punct(",");
          const nxt = c.peek();
          if (nxt?.kind !== "word" || nxt.possessive) return null;
          const w = nxt.value;
          const qualifier =
            w.startsWith("non") || COLORS.has(w) || SUPERTYPES.has(w) || STATUS_WORDS.has(w);
          return qualifier ? true : null;
        });
        if (consumed) continue;
        break;
      }
      const t = c.peek();
      if (!t || t.kind !== "word" || t.possessive) break;
      const v = t.value;

      // In "a land enchanted player controls", enchanted starts the
      // controller reference, not an adjective on the land. Likewise an
      // attached object's possessive controller must reach the qualifier.
      const following = c.peek(1);
      if (sawNominal && ["enchanted", "equipped"].includes(v) && following?.kind === "word"
        && (following.value === "player" || following.possessive)) break;
      if (sawNominal && v === "other" && following?.kind === "word" && following.value === "than") break;

      if (sawNominal && (v === "blocking" || v === "blocked" || v === "attacking")) {
        // A postnominal blocking relation carries an object operand. The
        // adjective-only form is parsed only if this complete relation fails.
        const position = c.pos;
        const relation = parseObjectRelationPredicate(c);
        c.pos = position;
        if (relation) break;
      }
      const ability = parseAbilityHead(c);
      if (ability) {
        if (f.cls && f.cls !== "ability") return c.fail("unambiguous ability selector");
        Object.assign(f, ability);
        sawNominal = true;
        if (c.isWord("or")) independentAlternative = true;
        continue;
      }
      if (v === "other" || v === "another") {
        c.pos++;
        f.other = true;
        continue;
      }
      if (v === "commander" || v === "commanders") {
        c.pos++;
        f.commander = true;
        sawNominal = true;
        continue;
      }
      if (SUPERTYPES.has(v)) {
        c.pos++;
        (f.supertypes ??= []).push(v);
        continue;
      }
      // CR 700.6: historic is legendary, artifact, or Saga.
      if (v === "historic") {
        c.pos++;
        (f.allOf ??= []).push({ anyOf: [{ supertypes: ["legendary"] }, { types: ["artifact"] }, { subtypes: ["Saga"] }] });
        continue;
      }
      // CR 702.33d: a kicked spell.
      if (v === "kicked" && following?.kind === "word" && !following.possessive && (following.value === "spell" || following.value === "spells")) {
        c.pos++;
        f.kicked = true;
        continue;
      }
      if (STATUS_WORDS.has(v)) {
        c.pos++;
        // CR 506.5: attacking/blocking alone is a status of its own.
        const alternatives = [(v === "attacking" || v === "blocking") && c.word("alone") !== null ? `${v}-alone` : v];
        let connector: "and" | "or" | null = null;
        let mixed = false;
        while (c.attempt(c => {
          const comma = c.punct(",");
          const lexical = parseLogicalJoin(c);
          if (!comma && !lexical) return null;
          const nextConnector = lexical === "and-or" ? "or" : lexical;
          const next = c.anyWord();
          if (!next || !STATUS_WORDS.has(next.value)) return null;
          if (nextConnector && connector && connector !== nextConnector) { mixed = true; return null; }
          if (nextConnector) connector = nextConnector;
          alternatives.push(next.value);
          return true;
        })) { /* e.g. attacking or blocking, not both simultaneously */ }
        if (mixed) return c.fail("unambiguous status conjunction");
        if (connector !== "or") (f.status ??= []).push(...alternatives);
        else (f.allOf ??= []).push({ anyOf: alternatives.map(status => ({ status: [status] })) });
        continue;
      }
      if (COLORS.has(v)) {
        c.pos++;
        const colors: Color[] = [v as Color];
        let join: string | undefined;
        for (;;) {
          const next = c.attempt((c) => {
            const comma = c.punct(",");
            const lexical = parseLogicalJoin(c);
            const connector = lexical === "and-or" ? "or" : lexical;
            if (!comma && connector === null) return null;
            const color = c.anyWord();
            if (!color || !COLORS.has(color.value)) return null;
            return { connector, color: color.value as Color };
          });
          if (!next) break;
          if (next.connector) {
            if (join && join !== next.connector) return c.fail("unambiguous color conjunction");
            join = next.connector;
          }
          colors.push(next.color);
        }
        if (colors.length > 1 && !join) return c.fail("explicit color conjunction");
        if (join === "or") (f.allOf ??= []).push({ anyOf: colors.map(color => ({ colors: [color] })) });
        else colors.forEach(addColor);
        continue;
      }
      if (v === "colorless") { c.pos++; f.colorless = true; continue; }
      if (v === "multicolored") { c.pos++; f.multicolored = true; continue; }
      if (v === "monocolored") { c.pos++; f.monocolored = true; continue; }
      if (v === "nontoken") { c.pos++; f.nonToken = true; continue; }
      if (v.startsWith("non")) {
        const rest = v.startsWith("non-") ? v.slice(4) : v.slice(3);
        const restSingular = singularize(rest);
        if (CARD_TYPES.has(restSingular)) {
          c.pos++;
          (f.nonTypes ??= []).push(restSingular);
          continue;
        }
        if (COLORS.has(rest)) {
          c.pos++;
          (f.nonColors ??= []).push(rest as Color);
          continue;
        }
        if (SUPERTYPES.has(rest)) {
          c.pos++;
          (f.nonSupertypes ??= []).push(rest);
          continue;
        }
        if (v.startsWith("non-") && canonicalSubtypeName(t.raw.slice(4))) {
          c.pos++;
          (f.nonSubtypes ??= []).push(canonicalSubtype(rest));
          continue;
        }
      }

      if (c.words("time", "lord") || c.words("time", "lords")) {
        (f.subtypes ??= []).push("Time Lord");
        sawNominal = true;
        continue;
      }
      const nominal = classifyNominal(v, t.raw);
      if (nominal) {
        // "player" only heads a filter when part of an or-union
        // ("target player or planeswalker") — bare players are PlayerRefs.
        if (nominal.cls === "player") {
          const nxt = c.peek(1);
          if (!(nxt?.kind === "word" && nxt.value === "or")) break;
        }
        c.pos++;
        sawNominal = true;
        const record = (n: { cls: NominalClass; word: string }) => {
          if (n.cls === "type") (f.types ??= []).push(n.word);
          else if (n.cls === "subtype") (f.subtypes ??= []).push(n.word);
          else if (n.cls === "player") { if (n.word === "opponent") f.orOpponent = true; else f.orPlayer = true; }
          else {
            // CR 110.4a: a permanent card/spell has a permanent card type;
            // it is not itself a permanent outside the battlefield.
            // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
            if (f.cls === "permanent" && (n.word === "card" || n.word === "spell")) f.permanent = true;
            else if (f.cls && f.cls !== n.word) (f.allOf ??= []).push({ cls: f.cls });
            f.cls = n.word as ObjectFilter["cls"];
          }
        };
        const alternatives = [nominal];
        let ambiguousAlternative = false;
        let listJoined = false;
        const listStart = c.pos;
        while (c.attempt((c) => {
          const scope = nominalOrScope(c, nominal);
          if (scope !== "shared") {
            independentAlternative = scope === "branch";
            ambiguousAlternative = scope === "ambiguous";
            return null;
          }
          const comma = c.isPunct(",");
          if (comma) c.punct(",");
          const join = parseLogicalJoin(c);
          const plainAnd = join === "and" && !comma;
          if (!join && !comma) return null;
          // A shared nominal head unions types: "creature and land cards".
          // Repeated articles instead introduce separate selections:
          // "a creature card and a land card" must not become one OR choice.
          if (c.isWord("a", "an", "another") && (plainAnd || join === "and-or"
            || (comma && !(predicate && !join && hasArticleNominalOrTail(c))))) return null;
          if (!plainAnd) c.word("a", "an");
          const nt = c.peek();
          if (nt?.kind !== "word" || nt.possessive) return null;
          const n2 = classifyNominal(nt.value, nt.raw);
          if (!n2 || !orCompatible(nominal.cls, n2.cls)) return null;
          if (plainAnd) {
            const after = c.peek(1);
            const head = after?.kind === "word" && !after.possessive ? classifyNominal(after.value, after.raw) : null;
            const sharedHead = head?.cls === "class" || (nominal.cls === "subtype" && head?.cls === "type");
            const plural = t.value !== nominal.word.toLowerCase() && nt.value !== n2.word.toLowerCase();
            if (!sharedHead && !plural) return null;
          }
          c.pos++;
          alternatives.push(n2);
          if (join) listJoined = true;
          return true;
        })) { /* collect alternatives before assigning their logical scope */ }
        // A comma-separated list needs its closing conjunction. Otherwise the
        // comma ends this noun phrase: "you control an Island, creatures …".
        const closesLater = () => {
          const position = c.pos;
          c.punct(",");
          const join = parseLogicalJoin(c);
          c.pos = position;
          return join !== null;
        };
        if (alternatives.length > 1 && !listJoined && !closesLater()) {
          c.pos = listStart;
          alternatives.length = 1;
          ambiguousAlternative = false;
          independentAlternative = false;
        }
        if (ambiguousAlternative) return c.fail("unambiguous compound nominal alternatives");
        if (alternatives.length === 1) record(nominal);
        else if (alternatives.some(n => n.cls === "player")) {
          // Preserve the existing simple object-or-player union. More complex
          // player unions need their own reference grammar.
          if (alternatives.length !== 2 || alternatives.every(n => n.cls === "player")) return c.fail("simple object-or-player union");
          alternatives.forEach(record);
        } else {
          (f.allOf ??= []).push({ anyOf: alternatives.map(n =>
            n.cls === "type" ? { types: [n.word] } : n.cls === "class" ? { cls: n.word as ObjectFilter["cls"] } : { subtypes: [n.word] }) });
        }
        continue outer;
      }

      break;
    }

    const hasAdjective = f.colors || f.nonColors || f.colorless || f.multicolored || f.monocolored
      || f.supertypes || f.nonSupertypes || f.nonTypes || f.nonSubtypes || f.nonToken || f.status
      || f.allOf;
    if (!sawNominal && !(predicate && hasAdjective)) return c.fail("filter");

    // Postfix qualifiers, any order. Keep repeated constraints conjunctive:
    // assigning a second controller or zone would otherwise erase the first.
    let qualified = false;
    for (;;) {
      const qualifier = c.attempt(c => {
        // A connector belongs here only if a complete qualifier follows.
        const conjunction = c.word("and") !== null;
        const result = parseFilterQualifier(c, f);
        if (!result) return null;
        if (conjunction) {
          // "...and you control a Mountain" is another condition, whereas
          // "...and that player controls" qualifies this same object.
          const next = c.peek();
          if (c.isWord("a", "an", "another") || (next?.kind === "word" && classifyNominal(next.value, next.raw))) return null;
        }
        return result;
      });
      if (!qualifier) break;
      qualified = true;
      const keys = Object.keys(qualifier) as (keyof ObjectFilter)[];
      if ((qualifier.otherThan && f.other) || keys.some(key => f[key] !== undefined)) (f.allOf ??= []).push(qualifier);
      else Object.assign(f, qualifier);
    }

    // Independently repeated noun phrases retain their own qualifiers. The
    // earlier nominal OR handles shared suffixes ("instant or sorcery card").
    // Do not interpret general AND as intersection: it can enumerate sets.
    if (qualified || independentAlternative || predicate) {
      const alternative = c.attempt(c => {
        if (independentAlternative) c.punct(",");
        const join = parseLogicalJoin(c);
        if (join !== "or" && join !== "and-or") return null;
        if (join === "and-or" && c.isWord("a", "an", "another")) return null;
        c.word("a", "an");
        return parseFilterExpression(c, predicate);
      });
      if (alternative) return joinFilterBranches(f, alternative) ?? c.fail("explicit scope for alternative filter qualifiers");
      if (independentAlternative) return c.fail("complete alternative filter");
    }
    return f;
  });
}

function parseFilterQualifier(c: Cursor, head: ObjectFilter): ObjectFilter | null {
  return c.attempt((c): ObjectFilter | null => {
    const characteristicRelation = parseCharacteristicRelationQualifier(c, { parseObjectRef });
    if (characteristicRelation) return characteristicRelation;
    const combatHistory = parseCombatActionHistoryPredicate(c);
    if (combatHistory) return head.combatHistory ? null : { combatHistory };
    const blockHistory = parseBlockHistoryQualifier(c, { parseRelationObject, parsePlayerRef });
    if (blockHistory) return head.combatHistory ? null : blockHistory;
    const damageHistory = parseDamageHistoryPredicate(c, { parseRelationObject, parsePlayerRef });
    if (damageHistory) return { damageHistory };
    const predicate = c.attempt(c => {
      if (c.word("that's") === null && !c.words("that", "is") && !c.words("that", "are")) return null;
      if (c.word("both") !== null) {
        // "both red and white" states the conjunction of exactly two colors.
        const both = parseFilterExpression(c, true);
        return both && Object.keys(both).length === 1 && both.colors?.length === 2 ? both : null;
      }
      c.word("a", "an");
      return parseObjectRelationPredicate(c) ?? parseFilterExpression(c, true);
    });
    if (predicate) return predicate;
    const relation = parseObjectRelationPredicate(c);
    if (relation) return relation;
    const otherThan = c.attempt(c => {
      if (!c.words("other", "than")) return null;
      const ref = parseObjectRef(c);
      // An indefinite description can exclude a category rather than one
      // chosen object. Accept only explicit identity references here.
      return ref && (["self", "this", "it", "enchanted", "equipped", "target"].includes(ref.ref)
        || (ref.ref === "that" && !ref.plural)) ? ref : null;
    });
    if (otherThan) return { otherThan };
    if (head.cls === "ability") {
      const abilitySource = c.attempt(c => {
        const connector = c.word("of", "from");
        if (!connector) return null;
        const source = parseRelationObject(c);
        if (!source) return null;
        if (connector === "from" && !("filter" in source && source.filter.cls === "source")) return null;
        return source;
      });
      if (abilitySource) return { abilitySource };
      const nonMana = c.attempt(c => {
        if (!c.words("that", "isn't") && !c.words("that", "aren't") && !c.words("that", "is", "not") && !c.words("that", "are", "not")) return null;
        c.word("a");
        if (c.word("mana") === null || c.word("ability", "abilities") === null) return null;
        // Overwhelming Splendor: under negation, "mana abilities or loyalty
        // abilities" excludes both kinds. Never escape to a positive OR
        // branch after consuming only the first negated alternative.
        if (c.word("or") !== null) {
          c.word("a", "an");
          if (c.word("loyalty") === null || c.word("ability", "abilities") === null || c.isWord("or", "and")) return null;
          return { manaAbility: false, loyaltyAbility: false };
        }
        return { manaAbility: false };
      });
      if (nonMana) return nonMana;
    }
    const chosenColor = c.attempt(c => {
      if (!c.words("of", "the")) return null;
      const last = c.word("last") !== null;
      if (!c.words("chosen", "color")) return null;
      return { choice: "chosen-value" as const, category: "color" as const, which: last ? "last-chosen" as const : "chosen" as const };
    });
    if (chosenColor) return { chosenColor };
    const chosenType = c.attempt(c => {
      if (!c.words("of", "the", "chosen")) return null;
      const kind = c.word("card", "creature", "land", "artifact", "enchantment", "planeswalker") as NonNullable<ObjectFilter["chosenType"]>["kind"] | null;
      if (c.word("type") === null) return null;
      // CR 205.3e and 607.2d: the chosen value remains linked to its choice;
      // a creature head cannot tell us whether "type" meant a card type.
      return { choice: "chosen-type" as const, ...(kind ? { kind } : {}) };
    });
    if (chosenType) return { chosenType };
    const targets = c.attempt(c => {
      if (c.word("that") === null || c.word("target", "targets") === null) return null;
      return c.attempt(parsePlayerRef) ?? parseObjectRef(c);
    });
    if (targets) return { targets };
    const cast = c.attempt(c => {
      const by = parsePlayerRef(c);
      if (!by || c.word("cast", "casts") === null) return null;
      const from = c.word("from") !== null ? parseZoneRef(c) : undefined;
      if (from === null) return null;
      return { by, ...(from ? { from } : {}) };
    });
    if (cast) return { cast };
    if (c.word("named") !== null) {
      const name = c.peek();
      if (name?.kind !== "card-name") return null;
      c.pos++;
      return { name: { name: "literal", value: name.value } };
    }
    if (c.words("you", "control")) return { control: "you" };
    if (c.words("you", "don't", "control")) return { control: "not-you" };
    if (c.words("an", "opponent", "controls") || c.words("your", "opponents", "control")) return { control: "opponent" };
    if (c.words("that", "player", "controls")) return { control: "that-player" };
    if (c.words("you", "own")) return { own: "you" };
    if (c.words("you", "don't", "own")) return { own: "not-you" };
    // "with {X} in its mana cost" (CR 107.3, 202.1).
    const manaCostContains = c.attempt(c => {
      if (c.word("with") === null) return null;
      const token = c.peek();
      if (token?.kind !== "symbol" || token.value !== "X") return null;
      c.pos++;
      return c.words("in", "its", "mana", "cost") || c.words("in", "their", "mana", "costs") ? [token.value] : null;
    });
    if (manaCostContains) return { manaCostContains };
    const controlledBy = c.attempt(c => {
      const who = parsePlayerRef(c);
      if (!who || c.word("control", "controls") === null) return null;
      return who;
    });
    if (controlledBy) return { controlledBy };
    const fromAmong = c.attempt(c => {
      if (!c.words("from", "among")) return null;
      return parseObjectRef(c);
    });
    if (fromAmong) return { fromAmong };
    if (c.words("from", "outside", "the", "game")) return { outsideGame: true };
    const zone = c.attempt(c => {
      if (c.word("in", "from") === null) return null;
      return parseZoneRef(c);
    });
    if (zone) {
      // "from your hand or graveyard": the candidate is in either of that
      // player's zones; the shared possessive covers the bare second zone.
      const alternative = c.attempt((c): ZoneRef | null => {
        if (zone.position || !zone.owner || !["hand", "graveyard", "library"].includes(zone.zone)) return null;
        const join = parseLogicalJoin(c);
        if (join !== "or" && join !== "and-or") return null;
        const second = c.word("hand", "graveyard", "library") as "hand" | "graveyard" | "library" | null;
        if (!second || second === zone.zone || c.isWord("or", "and") || c.isPunct("/")) return null;
        return { zone: second, owner: zone.owner };
      });
      return alternative ? { anyOf: [{ zone }, { zone: alternative }] } : { zone };
    }
    const fromPool = c.attempt(c => {
      if (c.word("from") === null) return null;
      if (c.word("it") !== null) return { ref: "it" as const };
      if (c.word("them") !== null) return { ref: "them" as const };
      return null;
    });
    if (fromPool) return { fromAmong: fromPool };
    if (c.words("on", "the", "battlefield")) return { zone: { zone: "battlefield" } };
    if (c.words("that", "aren't", "on", "the", "battlefield") || c.words("that", "isn't", "on", "the", "battlefield"))
      return { outsideZone: { zone: "battlefield" } };
    const property = parsePropertyConstraint(c);
    if (property) return { properties: [property] };
    const eventRelation = parseObjectRelation(c);
    if (eventRelation) return { relation: eventRelation };
    return parseCounterConstraint(c) ?? parseAbilityConstraint(c, head.cls === "spell");
  });
}

/** CR 122.1: presence means at least one counter, absence means zero. Counter
 * kinds are kept separate and these constraints bind to each selected object.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
function parseCounterConstraint(c: Cursor, predicate = false): ObjectFilter | null {
  return c.attempt(c => {
    const connector = predicate ? c.word("has", "have", "had") : c.word("with", "without");
    if (!connector) return null;
    const absence = connector === "without" || c.word("no") !== null;
    let comparison: Comparison;
    if (absence) {
      c.word("any", "a", "an");
      const quantity = c.peek();
      if (quantity?.kind === "number" || (quantity?.kind === "word" && wordNumber(quantity.value) !== undefined)) return null;
      comparison = { op: "eq", value: { amount: "fixed", value: 0 } };
    } else if (c.word("a", "an", "any") !== null) {
      comparison = { op: "ge", value: { amount: "fixed", value: 1 } };
    } else {
      comparison = parseComparison(c) ?? { op: "ge", value: { amount: "fixed", value: 1 } };
    }
    // Parity and counter-kind cardinality require different semantic nodes.
    if (c.isWord("odd", "even", "different", "kinds", "types", "each", "of", "more", "fewer", "less", "at", "least", "most")) return null;
    const counter = c.isWord("counter", "counters") ? "any" : parseCounterSpec(c);
    if (!counter || c.word("counter", "counters") === null) return null;
    if (c.word("on") !== null && c.word("it", "them") === null) return null;
    return { counterConstraints: [{ counter, comparison }] };
  });
}

/** Bare keyword-name tests use the same closed registry as keyword abilities.
 * Under negation, "without flying or reach" requires both to be absent.
 * CR 702.1: a keyword can stand for one or more abilities.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
function parseAbilityConstraint(c: Cursor, spellHead = false): ObjectFilter | null {
  return c.attempt(c => {
    // "a spell that has convoke" states the same test as "with convoke".
    const mode = c.word("with", "without") ?? (spellHead && c.words("that", "has") ? "with" : null);
    if (!mode) return null;
    // "with a morph ability" / "with morph abilities" name the keyword ability.
    const indefinite = c.word("a", "an") !== null;
    const first = parseKeywordName(c);
    if (!first) return null;
    const noun = c.word("ability", "abilities");
    if (indefinite && noun !== "ability") return null;
    const names = [first];
    let join: "and" | "or" | undefined;
    for (;;) {
      const next = c.attempt(c => {
        const comma = c.punct(",");
        const lexical = parseLogicalJoin(c);
        const connector = lexical === "and-or" ? "or" : lexical;
        if (!comma && !connector) return null;
        const name = parseKeywordName(c);
        return name && { connector, name };
      });
      if (!next) break;
      if (next.connector) {
        if (join && next.connector !== join) return c.fail("unambiguous keyword list");
        join = next.connector;
      }
      names.push(next.name);
    }
    if (names.length > 1 && !join) return c.fail("explicit keyword-list conjunction");
    if (mode === "with") return join === "or" ? { anyOf: names.map(name => ({ withAbility: [name] })) } : { withAbility: names };
    // The unparenthesized negated-AND form is ambiguous; leave it unsupported.
    if (join === "and") return c.fail("unambiguous negated keyword list");
    return { withoutAbility: names };
  });
}

/** "with power 2 or less", "with mana value 3 or greater" */
function parsePropertyConstraint(c: Cursor, past = false): PropertyConstraint | null {
  return c.attempt((c): PropertyConstraint | null => {
    if (!c.words("with") && !c.words("of") && !c.words("that", "has") && !c.words("that", "have") && c.word("has", "have") === null && !(past && c.word("had") !== null)) return null;
    const extremum = parseAttributeExtremum(c);
    if (extremum) return { property: extremum.attribute, comparison: { op: "eq", value: extremum } };
    let property: PropertyConstraint["property"];
    if (c.words("mana", "value")) property = "mana-value";
    else {
      const w = c.word("power", "toughness");
      if (!w) return null;
      property = w as "power" | "toughness";
    }
    const comparison = parseComparison(c);
    if (!comparison) return null;
    return { property, comparison };
  });
}

// ---------------------------------------------------------------------------
// Player refs
// ---------------------------------------------------------------------------

/** A filtered target keeps the target selector from its base; eligibility
 * never changes when or by whom that target is chosen (CR 115.1). */
export function isTargetedPlayerRef(ref: PlayerRef): boolean {
  if (ref.player === "group") return ref.members.some(isTargetedPlayerRef);
  return ref.player === "filtered" ? isTargetedPlayerRef(ref.base)
    : ref.player === "target-player" || ref.player === "target-opponent";
}
export function isPluralPlayerRef(ref: PlayerRef): boolean {
  if (ref.player === "group") return true;
  return ref.player === "filtered" ? isPluralPlayerRef(ref.base)
    : ref.player === "each-player" || ref.player === "each-opponent";
}

/** Each candidate supplies the hand/life/control subject. Event outcomes
 * ("who didn't discard this way") require a result scope and stay unsupported. */
function parsePlayerQualifier(c: Cursor): PlayerFilter | null {
  return c.attempt(c => {
    const damageHistory = c.attempt(c => {
      const history = parseDamageHistoryPredicate(c, { parseRelationObject, parsePlayerRef });
      return history?.role === "recipient" ? history : null;
    });
    if (damageHistory) return { damageHistory };
    if (c.words("other", "than")) {
      const otherThan = parsePlayerBase(c);
      if (c.isWord("with", "who")) return null;
      return otherThan && { otherThan };
    }
    const comparative = parseComparativePlayerQualifier(c, { parseAmount, parseCount, parseFilter, parsePlayerRef });
    if (comparative) return comparative;
    const controls = c.attempt(c => {
      if (c.word("who") === null || c.word("control", "controls") === null) return null;
      let comparison: Comparison | null;
      if (c.word("no") !== null) comparison = { op: "eq", value: { amount: "fixed", value: 0 } };
      else if (c.word("a", "an") !== null) comparison = { op: "ge", value: { amount: "fixed", value: 1 } };
      else {
        const exact = c.isWord("exactly");
        comparison = parseComparison(c);
        // A bare plural count has context-sensitive English scope. Support
        // only an explicit bound or "exactly" until that grammar is modeled.
        if (comparison?.op === "eq" && !exact) return null;
      }
      const filter = comparison && parseFilter(c);
      return filter && comparison ? { controls: { filter, comparison } } : null;
    });
    if (controls) return controls;
    if (c.word("with") === null && !(c.word("who") !== null && c.word("has", "have") !== null)) return null;
    const comparison: Comparison | null = c.word("no") !== null
      ? { op: "eq", value: { amount: "fixed", value: 0 } } : parseComparison(c);
    if (!comparison) return null;
    if (c.word("life") !== null) return { lifeTotal: comparison };
    if (c.word("card", "cards") !== null && c.words("in", "hand")) return { handSize: comparison };
    const counter = c.isWord("counter", "counters") ? "any" : parseCounterSpec(c);
    if (counter && c.word("counter", "counters") !== null) return { counterConstraints: [{ counter, comparison }] };
    return null;
  });
}

export function parsePlayerRef(c: Cursor): PlayerRef | null {
  return c.attempt(c => {
    const base = parsePlayerBase(c);
    if (!base) return null;
    const filter: PlayerFilter = {};
    for (;;) {
      const qualifier = c.attempt(c => {
        c.word("and");
        return parsePlayerQualifier(c);
      });
      if (!qualifier) break;
      const keys = Object.keys(qualifier) as (keyof PlayerFilter)[];
      if (keys.some(key => filter[key] !== undefined)) (filter.allOf ??= []).push(qualifier);
      else Object.assign(filter, qualifier);
    }
    return Object.keys(filter).length ? { player: "filtered", base, filter } : base;
  });
}

function parsePlayerBase(c: Cursor): PlayerRef | null {
  return c.attempt((c): PlayerRef | null => {
    const target = c.peek();
    const chosen = parseChosenPlayer(c);
    if (chosen) return chosen;
    if (c.words("a", "player")) return { player: "a-player" };
    if (c.words("each", "player")) return { player: "each-player" };
    if (c.words("each", "opponent")) return { player: "each-opponent" };
    if (c.words("target", "player")) return { player: "target-player", ...targetSelectionProvenance(target) };
    if (c.words("target", "opponent")) return { player: "target-opponent", ...targetSelectionProvenance(target) };
    if (c.words("an", "opponent")) return { player: "an-opponent" };
    if (c.words("that", "player")) return parseOrThatObjectsController(c, parsePossessiveObject) ?? { player: "that-player" };
    if (c.words("the", "player")) {
      // Definite descriptions such as "the player with the most life" are
      // not this bare contextual reference and must retain their qualifier.
      if (c.isWord("who", "whose", "with", "to")) return null;
      return { player: "the-player" };
    }
    if (c.word("they") !== null) return { player: "they" };
    if (c.words("defending", "player")) return { player: "defending-player" };
    // CR 303.4b,e: an enchanted player is not this Aura's controller.
    if (c.words("enchanted", "player")) return { player: "enchanted-player" };
    if (c.word("you") !== null) return { player: "you" };
    const of = parsePossessiveObject(c);
    if (of) {
      const relation = c.word("controller", "owner");
      if (relation === "controller") {
        const dependent = parseOrThatPlayer(c, of);
        if (dependent) return dependent;
      }
      if (relation) return { player: relation as "controller" | "owner", of };
    }
    return c.fail("player");
  });
}

// ---------------------------------------------------------------------------
// Object refs
// ---------------------------------------------------------------------------

const THIS_NOUNS = new Set([
  "creature", "spell", "card", "permanent", "artifact", "enchantment", "land",
  "token", "aura", "equipment", "planeswalker", "vehicle", "class", "spacecraft", "battle", "saga", "dungeon",
]);

/** Only explicit action/selection slots opt in. Nested reference queries,
 * trigger participants, continuous subjects and historical predicates retain
 * parseObjectRef's grammar and cannot manufacture a selected group. */
export function parseObjectSelection(c: Cursor): ObjectRef | null {
  return parseObjectRefInternal(c, true);
}

/** Explicit search/reveal/card-move instruction slots only. Generic choose,
 * sacrifice/tap costs, target lists, triggers and queries do not opt in. */
export function parseCardExtraction(c: Cursor): ObjectRef | null {
  return parseCardExtractionReference(c, { parseSelection: parseObjectSelection });
}

export function parseObjectRef(c: Cursor): ObjectRef | null {
  return parseObjectRefInternal(c, false);
}

function parseSelectionFilter(c: Cursor, enabled: boolean): { filter: ObjectFilter; groupConstraints?: GroupConstraint[] } | null {
  return c.attempt(c => {
    const filter = parseFilter(c);
    if (!filter) return null;
    if (!enabled) return { filter };
    const groupConstraints: GroupConstraint[] = [];
    for (;;) {
      const group = c.attempt(c => {
        c.word("and");
        return parseGroupConstraint(c, parseComparison);
      });
      if (group) { groupConstraints.push(group); continue; }
      // After a collective constraint, later qualifiers still restrict each
      // candidate, e.g. "different names that each have mana value X or less".
      if (!groupConstraints.length) break;
      const qualifier = c.attempt((c): ObjectFilter | null => {
        const conjunction = c.word("and") !== null;
        const each = c.attempt(c => {
          if (!c.words("that", "each")) return null;
          const property = parsePropertyConstraint(c);
          return property && { properties: [property] };
        });
        const result = each ?? parseFilterQualifier(c, filter);
        if (!result) return null;
        if (conjunction) {
          const next = c.peek();
          if (c.isWord("a", "an", "another") || next?.kind === "word" && classifyNominal(next.value, next.raw)) return null;
        }
        return result;
      });
      if (!qualifier) break;
      const keys = Object.keys(qualifier) as (keyof ObjectFilter)[];
      if ((qualifier.otherThan && filter.other) || keys.some(key => filter[key] !== undefined))
        (filter.allOf ??= []).push(qualifier);
      else Object.assign(filter, qualifier);
    }
    return { filter, ...(groupConstraints.length ? { groupConstraints } : {}) };
  });
}

function parseObjectRefInternal(c: Cursor, groups: boolean): ObjectRef | null {
  return c.attempt((c): ObjectRef | null => {
    const eachOf = parseEachOf(c, c => parseObjectRefInternal(c, groups));
    if (eachOf) return eachOf;
    const leaders = parseVoteLeaders(c, parseFilter);
    if (leaders) return leaders;
    const selection = c.attempt((c): ObjectRef | null => {
      const upTo = c.words("up", "to");
      const count = parseAmount(c);
      if (!count || c.word("of") === null) return null;
      const pool = parseObjectRef(c);
      if (!pool || !validSelectionCount(count)) return null;
      const constraint = groups ? parseGroupConstraint(c, parseComparison) : null;
      return { ref: "from-pool", pool, count, ...(upTo ? { upTo: true } : {}), ...(constraint ? { groupConstraints: [constraint] } : {}) };
    });
    if (selection) return selection;
    const rest = c.attempt((c): ObjectRef | null => {
      if (!c.words("the", "rest")) return null;
      if (c.word("of") === null) return { ref: "rest" };
      const of = parseObjectRef(c);
      return of && { ref: "rest", of };
    });
    if (rest) return rest;
    const other = groups ? parseTheOther(c) : null;
    if (other) return other;
    const owned = c.attempt((c): ObjectRef | null => {
      const owner = parsePossessivePlayer(c);
      if (!owner) return null;
      // CR 903.3, 702.124e: ownership of the designation persists across zones;
      // singular "your commander" selects either one when both qualify.
      // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
      if (c.word("commander") !== null) return { ref: "commander", owner, count: { amount: "fixed", value: 1 } };
      const zone = c.word("graveyard", "hand", "library") as "graveyard" | "hand" | "library" | null;
      return zone && { ref: "zone-cards", zone, owner };
    });
    if (owned) return owned;
    // "The cards from/in your hand" is that zone's complete contents.
    const contents = c.attempt((c): ObjectRef | null => {
      if (!c.words("the", "cards") || c.word("from", "in") === null) return null;
      const owner = parsePossessivePlayer(c);
      const zone = owner && c.word("graveyard", "hand", "library") as "graveyard" | "hand" | "library" | null;
      return owner && zone ? { ref: "zone-cards", zone, owner } : null;
    });
    if (contents) return contents;
    // "All graveyards" as an object is every card in each player's graveyard,
    // the same set as "each player's graveyard" (CR 400.1, 404.1).
    if (c.words("all", "graveyards")) return { ref: "zone-cards", zone: "graveyard", owner: { player: "each-player" } };
    const chosen = parseChosenObjectRef(c);
    if (chosen) return chosen;
    const related = parseRelatedRef(c);
    if (related) return related;
    const library = c.attempt((c): ObjectRef | null => {
      if (c.word("the") === null) return null;
      const position = c.word("top", "bottom");
      if (!position) return null;
      const count = parseAmount(c) ?? { amount: "fixed", value: 1 };
      if (c.word("card", "cards") === null || c.word("of") === null) return null;
      const of = parseZoneRef(c);
      if (of?.zone !== "library" || of.position) return null;
      return { ref: "library-cards", position: position as "top" | "bottom", count, of };
    });
    if (library) return library;
    if (c.selfref()) return { ref: "self" };
    if (c.word("it", "itself") !== null) return { ref: "it" };
    if (c.word("them", "they") !== null) return { ref: "them" };

    const anyTarget = c.attempt((c): ObjectRef | null => {
      if (c.word("any") === null) return null;
      const token = c.peek();
      return c.word("target") !== null ? { ref: "any-target", ...targetSelectionProvenance(token) } : null;
    });
    if (anyTarget) return anyTarget;

    const chosenTargets = c.attempt((c): ObjectRef | null => {
      const count: Amount | null = c.words("any", "number", "of")
        ? { amount: "chosen", minimum: 0 }
        : c.words("one", "or", "two")
          ? { amount: "chosen", minimum: 1, maximum: { amount: "fixed", value: 2 } }
          : groups && c.words("one", "or", "more") ? { amount: "chosen", minimum: 1 } : null;
      if (!count) return null;
      const pluralTarget = c.peek();
      if (c.word("targets") !== null) return { ref: "any-target", count, ...targetSelectionProvenance(pluralTarget) };
      const other = c.word("other") !== null;
      const targetToken = c.peek();
      const target = c.word("target") !== null;
      const selected = parseSelectionFilter(c, groups);
      if (!selected) return null;
      if (other) selected.filter.other = true;
      return target ? { ref: "target", ...selected, count, ...targetSelectionProvenance(targetToken) } : { ref: "filter", ...selected, count };
    });
    if (chosenTargets) return chosenTargets;

    const thisRef = c.attempt((c): ObjectRef | null => {
      if (c.word("this") === null) return null;
      const noun = c.anyWord();
      if (!noun) return null;
      // Explicit source nouns can be subtypes too: "this Siege", "this
      // Case", or "this Saga". Require a closed, singular subtype, never an
      // arbitrary prose word or a plural selector (CR 201.5 / 205.3).
      const subtype = canonicalSubtypeName(noun.raw);
      if (!THIS_NOUNS.has(noun.value) && (!subtype || noun.value !== subtype.toLowerCase())) return null;
      return { ref: "this", noun: noun.value };
    });
    if (thisRef) return thisRef;

    const thatRef = c.attempt((c): ObjectRef | null => {
      const word = c.word("that", "those");
      if (!word) return null;
      // One established recipient in either domain (an earlier "any target"),
      // as in prevention.ts; never that permanent plus some other player.
      if (word === "that" && c.words("permanent", "or", "player")) return { ref: "that", filter: { cls: "permanent", orPlayer: true } };
      const filter = parseFilter(c);
      if (!filter) return null;
      return { ref: "that", filter, ...(word === "those" ? { plural: true } : {}) };
    });
    if (thatRef) return thatRef;

    const targetRef = c.attempt((c): ObjectRef | null => {
      const another = c.word("another") !== null;
      const upTo = c.words("up", "to");
      const count = c.attempt(parseCount) ?? { amount: "fixed" as const, value: 1 };
      const other = c.word("other") !== null;
      const token = c.peek();
      if (c.word("target") === null) return null;
      const selected = parseSelectionFilter(c, groups);
      if (!selected || !validSelectionCount(count)) return null;
      if (another || other) selected.filter.other = true;
      if (another && (upTo || count.amount !== "fixed" || count.value !== 1)) return null;
      return { ref: "target", ...selected, count, ...(upTo ? { upTo: true } : {}), ...targetSelectionProvenance(token) };
    });
    if (targetRef) return targetRef;
    const bareTargets = parseBareTargets(c, c => {
      const position = c.pos;
      const noun = parseFilter(c) !== null || c.isWord("player", "players", "opponent", "opponents");
      c.pos = position;
      return noun;
    });
    if (bareTargets) return bareTargets;

    const eachRef = c.attempt((c): ObjectRef | null => {
      if (c.word("each") === null) return null;
      const filter = parseFilter(c);
      if (!filter) return null;
      return { ref: "each", filter };
    });
    if (eachRef) return eachRef;

    const allRef = c.attempt((c): ObjectRef | null => {
      if (c.word("all") === null) return null;
      c.word("the");
      const filter = parseFilter(c);
      if (!filter) return null;
      return { ref: "all", filter };
    });
    if (allRef) return allRef;

    const equippedRef = c.attempt((c): ObjectRef | null => {
      const kind = c.word("equipped", "enchanted");
      if (!kind) return null;
      const noun = c.anyWord();
      if (!noun) return null;
      const subtype = canonicalSubtypeName(noun.raw);
      // "enchanted Forest" denotes this Aura's attached object (303.4b),
      // whereas "enchanted Forests" is a filter over multiple objects.
      if (!THIS_NOUNS.has(noun.value) && (!subtype || noun.value !== subtype.toLowerCase())) return null;
      return { ref: kind as "equipped" | "enchanted", noun: noun.value };
    });
    if (equippedRef) return equippedRef;

    const countedRef = c.attempt((c): ObjectRef | null => {
      const upTo = c.words("up", "to");
      const another = c.word("another") !== null;
      const count: Amount | null = another ? { amount: "fixed", value: 1 } : parseAmount(c);
      if (!count) return null;
      const selected = parseSelectionFilter(c, groups);
      if (!selected || !validSelectionCount(count)) return null;
      if (another) selected.filter.other = true;
      return { ref: "filter", ...selected, count, ...(upTo ? { upTo: true } : {}) };
    });
    if (countedRef) return countedRef;

    // Bare plural filter: "creatures you control" — semantically "each".
    const bare = parseFilter(c);
    if (bare) return { ref: "each", filter: bare };

    return c.fail("object");
  });
}

export function parsePossessivePlayer(c: Cursor): PlayerRef | null {
  return c.attempt(c => {
    const chosen = parseChosenPlayer(c, true);
    if (chosen) return chosen;
    const dependent = c.attempt(c => {
      const of = parsePossessiveObject(c);
      if (!of) return null;
      const relation = c.possessive("controller", "owner");
      return relation ? { player: relation as "controller" | "owner", of } : null;
    });
    if (dependent) return dependent;
    if (c.attempt(c => c.word("enchanted") !== null && c.possessive("player") !== null ? true : null))
      return { player: "enchanted-player" };
    if (c.attempt(c => c.word("defending") !== null && c.possessive("player") !== null ? true : null))
      return { player: "defending-player" };
    if (c.word("your") !== null) return { player: "you" };
    if (c.word("their") !== null) return { player: "they" };
    const target = c.peek();
    if (c.word("target") !== null) {
      if (c.possessive("player") !== null) return { player: "target-player", ...targetSelectionProvenance(target) };
      if (c.possessive("opponent") !== null) return { player: "target-opponent", ...targetSelectionProvenance(target) };
      return null;
    }
    if (c.word("that") !== null && c.possessive("player") !== null) return { player: "that-player" };
    if (c.word("the") !== null && c.possessive("player") !== null) return { player: "the-player" };
    if (c.word("an") !== null && c.possessive("opponent") !== null) return { player: "an-opponent" };
    if (c.word("each") !== null) {
      if (c.possessive("player") !== null) return { player: "each-player" };
      if (c.possessive("opponent") !== null) return { player: "each-opponent" };
    }
    return null;
  });
}

/** CR607.1c/607.2d includes both self-linked and separate printed choices.
 * Preserve the choice kind and recency; neither supplies its origin link.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
function parseChosenPlayer(c: Cursor, possessive = false): PlayerRef | null {
  return c.attempt(c => {
    if (c.word("the") === null) return null;
    const last = c.word("last") !== null;
    if (c.word("chosen") === null || (possessive ? c.possessive("player") : c.word("player")) === null) return null;
    return { player: "chosen-player", choice: { choice: "chosen-value", category: "player", which: last ? "last-chosen" : "chosen" } };
  });
}

const RESULT_EVENTS = {
  exiled: "exile", sacrificed: "sacrifice", discarded: "discard", revealed: "reveal", destroyed: "destroy",
} as const;

/** CR 607.1–2: "exiled with" is an explicit link, not an arbitrary object in
 * exile. Bare definite result references can be linked or within the same
 * resolution, so leave their contextual antecedent symbolic until binding.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
function parseObjectRelation(c: Cursor): ObjectRelation | null {
  return c.attempt(c => {
    const who = parsePlayerRef(c);
    const event = c.word("exiled", "sacrificed", "discarded", "revealed", "destroyed") as keyof typeof RESULT_EVENTS | null;
    if (!event) return null;
    let relation: ObjectRelation;
    if (c.words("this", "way")) relation = { relation: "event-result", event: RESULT_EVENTS[event], scope: "this-way" };
    else if (event === "exiled" && c.word("with") !== null) {
      const source = parseObjectRef(c);
      if (!source) return null;
      relation = { relation: "linked-exile", source };
    } else return null;
    if (who) relation.who = who;
    if (c.words("this", "turn")) relation.period = "this-turn";
    return relation;
  });
}

/** Number belongs to the outer noun phrase, never a nested qualifier's noun.
 * Card/spell/token/permanent is the head of "creature cards"; otherwise the
 * rightmost prenominal type/subtype is the head. Unknown heads stay rejected. */
function relatedNumber(c: Cursor, start: number, end: number): "singular" | "plural" | null {
  let number: "singular" | "plural" | null = null;
  for (let pos = start; pos < end; pos++) {
    const token = c.tokens[pos];
    if (token?.kind !== "word" || token.possessive) break;
    const nominal = classifyNominal(token.value, token.raw);
    if (nominal && nominal.cls !== "player") {
      number = token.value === nominal.word.toLowerCase() ? "singular" : "plural";
    } else if (!COLORS.has(token.value) && !SUPERTYPES.has(token.value) && !STATUS_WORDS.has(token.value)
      && !["other", "nonland", "noncreature", "nontoken", "colorless", "multicolored", "monocolored"].includes(token.value)) break;
  }
  return number;
}

/** CR601.2c, 608.2d and 607.2d keep announcement, resolution and linked
 * choices distinct. This lexical result reference captures none of those
 * automatically. It also never introduces a fresh target selection ID.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
function parseChosenObjectRef(c: Cursor): ObjectRef | null {
  return c.attempt(c => {
    if (c.word("the") === null) return null;
    const last = c.word("last") !== null;
    if (c.word("chosen") === null) return null;
    const start = c.pos;
    const filter = parseFilter(c);
    if (!filter || filter.orPlayer || c.isWord("type", "types", "name", "names")) return null;
    const number = relatedNumber(c, start, c.pos);
    return number ? { ref: "choice-result", number, filter, which: last ? "last-chosen" : "chosen", scope: "contextual" } : null;
  });
}

function parseRelatedRef(c: Cursor): ObjectRef | null {
  return c.attempt(c => {
    const definite = c.word("the") !== null;
    const event = definite ? c.word("exiled", "sacrificed", "discarded", "revealed", "destroyed") as keyof typeof RESULT_EVENTS | null : null;
    const start = c.pos;
    const parsed = parseFilter(c);
    if (!parsed) return null;
    const number = relatedNumber(c, start, c.pos);
    if (!number) return null;
    if (event) {
      // A second explicit relation would need its own separately bound scope.
      if (parsed.relation) return null;
      return { ref: "related", number, filter: parsed, relation: { relation: "event-result", event: RESULT_EVENTS[event], scope: "contextual" } };
    }
    if (!parsed.relation) return null;
    const { relation, ...filter } = parsed;
    return { ref: "related", number, filter, relation };
  });
}

// ---------------------------------------------------------------------------
// Durations
// ---------------------------------------------------------------------------

export function parseDuration(c: Cursor): Duration | null {
  return c.attempt((c): Duration | null => {
    // CR 611.2a and 511.2 distinguish beginning an end step, ending combat,
    // and cleanup of the next turn; these boundaries must not be collapsed.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    if (c.words("until", "the", "end", "of", "your", "next", "turn") || c.words("until", "end", "of", "your", "next", "turn"))
      return { duration: "end-of-your-next-turn" };
    if (c.words("until", "your", "next", "end", "step") || c.words("until", "the", "beginning", "of", "your", "next", "end", "step"))
      return { duration: "your-next-end-step" };
    if (c.words("until", "your", "next", "upkeep")) return { duration: "your-next-upkeep" };
    if (c.words("until", "end", "of", "combat")) return { duration: "end-of-combat" };
    if (c.words("until", "end", "of", "turn")) return { duration: "end-of-turn" };
    if (c.words("until", "your", "next", "turn")) return { duration: "your-next-turn" };
    if (c.words("this", "turn")) return { duration: "this-turn" };
    if (c.words("this", "combat")) return { duration: "this-combat" };
    if (c.words("for", "as", "long", "as") || c.words("as", "long", "as")) {
      const condition = parseCondition(c);
      if (!condition) return null;
      return { duration: "as-long-as", condition };
    }
    return c.fail("duration");
  });
}

// ---------------------------------------------------------------------------
// Conditions
// ---------------------------------------------------------------------------

export function parseCondition(c: Cursor, options?: { purpose?: "unless" }): Condition | null {
  return parseConditionExpression(c, {
    ...options,
    parseAmount, parseComparison, parseCount, parseCounterSpec, parseFilter, parseObjectRef,
    parsePlayerRef, parsePossessivePlayer, parseZoneRef, parseAtomic: parseAtomicCondition,
  });
}

function parseAtomicCondition(c: Cursor): Condition | null {
  return c.attempt((c): Condition | null => {
    const targeting = c.attempt((c): Condition | null => {
      const what = parseObjectRef(c);
      if (!what || c.word("target", "targets") === null) return null;
      const target = c.attempt(parsePlayerRef) ?? parseObjectRef(c);
      return target ? { condition: "targets", what, target } : null;
    });
    if (targeting) return targeting;
    const compared = c.attempt((c): Condition | null => {
      const value = parseAmount(c);
      if (!value || c.word("is", "are") === null) return null;
      const comparison = parseComparison(c);
      return comparison ? { condition: "compare", value, comparison } : null;
    });
    if (compared) return compared;
    const doesNotControl = c.attempt((c): Condition | null => {
      const who = parsePlayerRef(c);
      if (!who || c.word("don't", "doesn't") === null || c.word("control") === null) return null;
      const what = parseObjectRef(c);
      return what ? { condition: "not", of: { condition: "control", who, what } } : null;
    });
    if (doesNotControl) return doesNotControl;
    const counted = c.attempt((c): Condition | null => {
      if (!c.words("there", "are") && !c.words("there", "is")) return null;
      const comparison = parseComparison(c);
      if (!comparison) return null;
      const of = parseFilter(c);
      return of ? { condition: "count", of, comparison } : null;
    });
    if (counted) return counted;
    if (c.words("you", "do", "so") || c.words("you", "do")) return { condition: "you-do" };
    if (c.words("you", "don't")) return { condition: "you-dont" };
    if (c.words("it's", "your", "turn")) return { condition: "your-turn" };
    if (c.words("it's", "not", "your", "turn")) return { condition: "not-your-turn" };
    if (c.words("it", "is", "your", "turn")) return { condition: "your-turn" };

    const control = c.attempt((c): Condition | null => {
      let who: PlayerRef | null = null;
      if (c.word("you") !== null) who = { player: "you" };
      else if (c.words("an", "opponent")) who = { player: "an-opponent" };
      else if (c.words("that", "player")) who = { player: "that-player" };
      else if (c.words("defending", "player")) who = { player: "defending-player" };
      if (!who) return null;
      if (c.word("control", "controls") === null) return null;
      const counted = c.attempt((c): Condition | null => {
        const comparison = parseComparison(c);
        if (!comparison || comparison.op === "eq") return null;
        const of = parseFilter(c);
        return of ? { condition: "control-count", who, of, comparison } : null;
      });
      if (counted) return counted;
      const what = parseObjectRef(c);
      if (!what) return null;
      return { condition: "control", who, what };
    });
    if (control) return control;

    const controlled = c.attempt((c): Condition | null => {
      const who = parsePlayerRef(c);
      if (!who || c.word("controlled") === null) return null;
      // Only an already identified object has an earlier existence to ask
      // about; "controlled a creature" would be a history of some period.
      const what = parseObjectRef(c);
      if (!what || !(what.ref === "it" || (what.ref === "that" && !what.plural))) return null;
      return { condition: "control", who, what, time: "previous" };
    });
    if (controlled) return controlled;

    const life = c.attempt((c): Condition | null => {
      const who = parsePlayerRef(c);
      if (!who) return null;
      if (c.word("have", "has") === null) return null;
      const comparison = parseComparison(c);
      if (!comparison) return null;
      if (c.word("life") === null) return null;
      // "At least 15 life more than your starting life total" (CR 103.4)
      // raises the bound; it is not a second, separate comparison.
      if (c.words("more", "than")) {
        const of = parsePossessivePlayer(c);
        if (!of || comparison.op !== "ge" || !c.words("starting", "life", "total")) return null;
        return { condition: "life-total", who, comparison: { op: "ge",
          value: { amount: "plus", a: { amount: "starting-life-total", of }, b: comparison.value } } };
      }
      return { condition: "life-total", who, comparison };
    });
    if (life) return life;

    const pays = c.attempt((c): Condition | null => {
      // "Any player" offers the payment to every player; one paying suffices.
      const who: PlayerRef | null = c.words("any", "player") ? { player: "a-player" } : parsePlayerRef(c);
      if (!who) return null;
      if (c.word("pays") === null) return null;
      // Mana symbols or "N life".
      const symbols: string[] = [];
      let s;
      while ((s = c.attempt((c) => {
        const token = c.peek();
        return token?.kind === "symbol" && isManaSymbol(token.value) ? c.symbol() : c.fail("mana symbol");
      })) !== null) symbols.push(s);
      if (symbols.length > 0) return { condition: "pays", who, cost: [{ cost: "mana", symbols }] };
      const amount = parseAmount(c);
      if (amount && c.word("life") !== null)
        return { condition: "pays", who, cost: [{ cost: "pay-life", amount }] };
      return null;
    });
    if (pays) return pays;

    const hasProperty = c.attempt((c): Condition | null => {
      // Limit the subject cursor so a relative-reference filter cannot swallow
      // the predicate: "that creature has power 4" tests the referred creature.
      const offset = c.tokens.slice(c.pos).findIndex(t => t.kind === "word" && !t.possessive && ["has", "have", "had"].includes(t.value));
      if (offset <= 0) return null;
      const subject = new Cursor(c.tokens.slice(c.pos, c.pos + offset));
      const what = parseObjectRef(subject);
      if (!what || !subject.done()) return null;
      c.pos += offset;
      const previous = c.isWord("had");
      const property = parsePropertyConstraint(c, true);
      const filter = property ? { properties: [property] } : parseCounterConstraint(c, true);
      if (!filter) return null;
      return { condition: "is-filter", what, filter, ...(previous ? { time: "previous" as const } : {}) };
    });
    if (hasProperty) return hasProperty;

    const lacksCounters = c.attempt((c): Condition | null => {
      const offset = c.tokens.slice(c.pos).findIndex(t => t.kind === "word" && !t.possessive && ["doesn't", "don't"].includes(t.value));
      if (offset <= 0) return null;
      const subject = new Cursor(c.tokens.slice(c.pos, c.pos + offset));
      const what = parseObjectRef(subject);
      if (!what || !subject.done()) return null;
      c.pos += offset + 1;
      const filter = parseCounterConstraint(c, true);
      return filter && { condition: "not", of: { condition: "is-filter", what, filter } };
    });
    if (lacksCounters) return lacksCounters;

    // "Its power was 3 or greater" states "it had power 3 or greater".
    const pastAttribute = c.attempt((c): Condition | null => {
      const what = parsePossessiveObject(c);
      if (!what) return null;
      const property = c.words("mana", "value") ? "mana-value" : c.word("power", "toughness") as "power" | "toughness" | null;
      if (!property || c.word("was") === null) return null;
      const comparison = parseComparison(c);
      return comparison && { condition: "is-filter", what, filter: { properties: [{ property, comparison }] }, time: "previous" };
    });
    if (pastAttribute) return pastAttribute;

    const isFilter = c.attempt((c): Condition | null => {
      let what: ObjectRef | null = null;
      let time: "previous" | undefined;
      let negate = false;
      if (c.word("it's") !== null) { what = { ref: "it" }; negate = c.word("not") !== null; }
      else {
        what = parseObjectRef(c);
        if (!what) return null;
        const verb = c.word("is", "was", "isn't", "wasn't");
        if (verb === null) return null;
        if (verb === "was" || verb === "wasn't") time = "previous";
        negate = verb === "isn't" || verb === "wasn't" || c.word("not") !== null;
      }
      const wrap = (of: Condition): Condition => negate ? { condition: "not", of } : of;
      c.word("a", "an");
      const status = c.attempt((c) => {
        const w = c.anyWord();
        if (!w || !STATUS_WORDS.has(w.value)) return null;
        // CR 506.5: attacking/blocking alone is a status of its own.
        return (w.value === "attacking" || w.value === "blocking") && c.word("alone") !== null ? `${w.value}-alone` : w.value;
      });
      if (status) return wrap({ condition: "status", what, status, ...(time ? { time } : {}) });
      // A predicate may be adjective-only: "isn't legendary", "it's red".
      const filter = parseFilter(c) ?? parseFilterExpression(c, true);
      if (!filter) return null;
      return wrap({ condition: "is-filter", what, filter, ...(time ? { time } : {}) });
    });
    if (isFilter) return isFilter;

    const remains = c.attempt((c): Condition | null => {
      const what = parseObjectRef(c);
      if (!what) return null;
      if (c.word("remains", "remain") === null) return null;
      if (c.word("exiled") !== null) return { condition: "remains", what, zone: { zone: "exile" } };
      const status = c.attempt(c => {
        const word = c.anyWord();
        return word && STATUS_WORDS.has(word.value) ? word.value : null;
      });
      if (status) return { condition: "status", what, status };
      c.word("on", "in");
      const zone = parseZoneRef(c);
      if (!zone) return null;
      return { condition: "remains", what, zone };
    });
    if (remains) return remains;

    return c.fail("condition");
  });
}

// ---------------------------------------------------------------------------
// Counters
// ---------------------------------------------------------------------------

/** "+1/+1", "-1/-1", "charge", "first strike" … (the words before "counter"). */
export function parseCounterSpec(c: Cursor): CounterSpec | null {
  return c.attempt((c): CounterSpec | null => {
    const pt = c.attempt((c): string | null => {
      const s1 = c.punct("+") ? "+" : c.punct("-") ? "-" : null;
      if (!s1) return null;
      const n1 = c.number();
      if (n1 === null || !c.punct("/")) return null;
      const s2 = c.punct("+") ? "+" : c.punct("-") ? "-" : null;
      if (!s2) return null;
      const n2 = c.number();
      if (n2 === null) return null;
      return `${s1}${n1}/${s2}${n2}`;
    });
    if (pt) return { type: pt };

    // Named counter: words until "counter(s)". Allow up to three words.
    const words: string[] = [];
    for (let i = 0; i < 3; i++) {
      if (c.isWord("counter", "counters")) break;
      const w = c.anyWord();
      if (!w) return null;
      if (["and", "or", "a", "an", "any", "no", "different", "more", "fewer"].includes(w.value)) return null;
      words.push(w.value);
    }
    if (words.length === 0 || !c.isWord("counter", "counters")) return c.fail("counter type");
    return { type: words.join(" ") };
  });
}
