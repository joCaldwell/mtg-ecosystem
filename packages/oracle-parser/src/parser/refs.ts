// Shared reference grammar: amounts, filters, object/player refs, zones,
// durations, conditions, counters. Everything here is strictly typed and
// backtracking — a function either returns a complete node or null.

import { parseLabeledPlayers } from "./friend-or-foe.ts";
import { parseDefiniteDescription } from "./definite-refs.ts";
import { parseIndependentFilterList } from "./filter-lists.ts";
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
import { parseAttachmentCondition } from "./attachment-conditions.ts";
import { parseBaseStatQualifier } from "./base-stats.ts";
import { parseBlockHistoryQualifier, parseCombatActionHistoryPredicate, parseDamageHistoryPredicate } from "./history-filters.ts";
import { parseDiceResultAmount } from "./dice.ts";
import { hasCombatHistory, parseAggregateAmount } from "./aggregates.ts";
import { parsePartySize, parseQuantity } from "./quantities.ts";
import { parseManaSymbolCount } from "./mana-symbol-counts.ts";
import { parseUnlockedDoorCount } from "./rooms.ts";
import { parseCountersPutRelation, parseDevouredRelation, parseKickCount } from "./history-units.ts";
import { parseAttackedPlayerCountAmount, parseOpponentCountAmount } from "./player-counts.ts";
import { parsePlayerExtremum } from "./player-extrema.ts";
import { parseComparativePlayerQualifier } from "./comparatives.ts";
import { parseDefiniteMostPlayer, parseMostPlayerQualifier } from "./player-most.ts";
import { parsePaidCosts } from "./paid-costs.ts";
import { parsePastControlQualifier } from "./past-control.ts";
import { parseGroupConstraint, validSelectionCount } from "./selection-constraints.ts";
import { parseCardExtraction as parseCardExtractionReference } from "./card-selections.ts";
import { isPluralObjectRef, parseBareTargets, parseEachOf, parseTheOther } from "./quantified-selections.ts";
import { parseExceptFor, parseOtherThanDescriptions } from "./filter-exceptions.ts";
import { barePluralPair } from "./plural-nouns.ts";
import { parseHistoryUnit, parseTimesCastUnit } from "./history-amounts.ts";
import { parseVoteCountAmount, parseVoteLeaders } from "./votes.ts";
import { parseSpeedAmount } from "./keyword-actions.ts";
import { deferredHalf } from "./half-amounts.ts";
import { parseConditionDifference } from "./condition-differences.ts";
import { insideEmblem } from "./emblem-scope.ts";
// Keyword-name matching is invoked only while parsing, after module setup.
// The shared registry also supplies keywords.ts; no duplicate keyword list.
import { parseKeywordName, parseLandwalkName } from "./keywords.ts";
import { parseEachOtherPlayer, parseOrThatObjectsController, parseOrThatPlayer, parseRelationalPlayer } from "./player-refs.ts";
import { parseNegatedPredicate } from "./negated-predicates.ts";
import { parseChosenColorRef, parseChosenNameQualifier, parseThatChoiceQualifier, parseValueOfChoiceQualifier } from "./choices.ts";
import { parseColorSetPredicate } from "./color-predicates.ts";
import { parseComparativeAttribute, parseSameController } from "./implicit-comparisons.ts";
import { parseZoneChangeRelation } from "./zone-history.ts";
import { parseCostTapperQualifier } from "./cost-tappers.ts";
import { parseParityQualifier } from "./parity.ts";
import { parseNumberOfSelection } from "./counted-selections.ts";
import { parseGraveyardTopCards } from "./graveyard-order.ts";
import { parseAllZoneUnion } from "./zone-unions.ts";
import { parsePooledFilter } from "./pooled-counts.ts";
import { parsePileRef } from "./piles.ts";
import { parsePrecedingActionQualifier } from "./player-actors.ts";
import { parseZoneChangeHistoryQualifier } from "./zone-history-filters.ts";
import { parseEventResultPlayerQualifier, parseSearchedPlayerQualifier } from "./event-outcomes.ts";
import { parseVotedForQualifier } from "./votes.ts";
import { parseStickerKind } from "./stickers.ts";

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
      // CR 107.1a: fractions must not acquire an invented rounding rule.
      // Oracle instructions state the direction when division can occur;
      // when it follows the whole instruction the step resolves it.
      const stated = c.attempt(c => { c.punct(","); return c.words("rounded") ? true : null; });
      if (!stated) return deferredHalf(of);
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
    const difference = parseConditionDifference(c);
    if (difference) return difference;
    if (c.words("that", "number")) return { amount: "that-number" };
    const chosenNumber = c.attempt((c): Amount | null => {
      if (c.word("the") === null) return null;
      const last = c.word("last") !== null;
      return c.words("chosen", "number") ? { amount: "chosen-number", which: last ? "last-chosen" : "chosen" } : null;
    });
    if (chosenNumber) return chosenNumber;
    const quantity = parseQuantity(c, { parseObjectRef, parsePlayerRef, parsePossessivePlayer, isPluralPlayerRef, parseChosenColor: c => parseChosenColorRef(c, parseFilter) });
    if (quantity) return quantity;
    const attr = c.attempt(parseAttributeAmount);
    if (attr) return attr;
    const extremum = parseAttributeExtremum(c);
    if (extremum) return extremum;
    const opponentCount = parseOpponentCountAmount(c, { parsePlayerRef, isPluralPlayerRef, parsePlayerQualifiers });
    if (opponentCount) return opponentCount;
    const attackedPlayers = parseAttackedPlayerCountAmount(c);
    if (attackedPlayers) return attackedPlayers;
    const playerExtremum = parsePlayerExtremum(c, { parseFilter });
    if (playerExtremum) return playerExtremum;
    const aggregate = parseAggregateAmount(c, { parseObjectRef });
    if (aggregate) return aggregate;
    // "The total number of" counts one pooled set ("cards in all players'
    // hands"); the pooling is stated by the counted description itself.
    if (c.attempt((c) => (c.words("the", "number", "of") || c.words("the", "total", "number", "of") ? true : null))) {
      const kicks = parseKickCount(c, parseObjectRef);
      if (kicks) return kicks;
      const party = parsePartySize(c, "plural");
      if (party) return party;
      const symbols = parseManaSymbolCount(c, parseFilter);
      if (symbols) return symbols;
      const doors = parseUnlockedDoorCount(c);
      if (doors) return doors;
      if (c.words("cards", "looked", "at", "while", "scrying", "this", "way")) return { amount: "event-quantity", event: "scry-looked-at", scope: "this-way" };
      const counters = c.attempt((c): Amount | null => {
        // Counter kinds are distinct from the number of counters (CR 122.1).
        if (c.isWord("different", "kinds", "types", "each", "of")) return null;
        const counter = c.isWord("counter", "counters") ? "any" : parseCounterSpec(c);
        if (!counter || c.word("counter", "counters") === null) return null;
        if (c.words("removed", "this", "way")) return { amount: "event-quantity", event: "counter-removal", counter, scope: "this-way" };
        if (c.word("on") !== null) {
          const on = parsePlayerRef(c) ?? parseObjectRef(c);
          // Counters exist on permanents and players; a following zone phrase
          // qualifies the enclosing selection, never this counted object.
          if (on && "ref" in on && on.ref === "that" && on.filter.zone) return c.fail("counted object without a zone qualifier");
          return on && { amount: "counter-count", counter, on };
        }
        // "Counters among permanents you control": the same total as the
        // counters on them; "among" always ranges over a group.
        if (c.word("among") !== null) {
          const on = parseObjectRef(c);
          return on && isPluralObjectRef(on) ? { amount: "counter-count", counter, on } : null;
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
      // "The number of spells you've cast this turn": completed actions.
      const history = parseHistoryUnit(c) ?? parseTimesCastUnit(c, true, { ref: "it" });
      if (history) return history;
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
    // "The greatest mana value of a commander you own" is the extremum over the
    // objects that description matches, like "among commanders you own".
    const viaAmong = c.word("among") !== null;
    if (!viaAmong && !c.words("of", "a") && !c.words("of", "an")) return null;
    const first = parseFilter(c);
    const among = first && viaAmong ? (parsePooledFilter(c, first, parseFilter, false) ?? first) : first;
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
    const singular = of.ref === "this" || of.ref === "enchanted" || of.ref === "equipped" || of.ref === "amassed-army"
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
    // "the power of the exiled card" states the same value as "the exiled
    // card's power"; only one singular object has a single such value.
    const stated = c.attempt((c): Amount | null => {
      if (c.word("the") === null) return null;
      const attribute = c.words("mana", "value") ? "mana-value" : (c.word("power", "toughness") as "power" | "toughness" | null);
      if (!attribute || c.word("of") === null) return null;
      const of = parseObjectRef(c);
      const singular = of && (of.ref === "self" || of.ref === "it" || of.ref === "this" || of.ref === "enchanted" || of.ref === "equipped" || of.ref === "amassed-army"
        || (of.ref === "that" && !of.plural) || ((of.ref === "related" || of.ref === "choice-result") && of.number === "singular")
        || (of.ref === "target" && of.count.amount === "fixed" && of.count.value === 1 && !of.upTo));
      return of && singular ? { amount: "attribute", of, attribute } : null;
    });
    if (stated) return stated;
    const of = parsePossessiveObject(c);
    if (!of) return null;
    // CR 208.4b: "its base power" ignores effects and counters that only modify it.
    const base = c.word("base") !== null;
    const stat = !base && c.words("mana", "value")
      ? "mana-value"
      : (c.word("power", "toughness") as "power" | "toughness" | null);
    if (!stat) return null;
    const attribute = base ? (stat === "power" ? "base-power" : stat === "toughness" ? "base-toughness" : null) : stat;
    return attribute && { amount: "attribute", of, attribute };
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
/** An unrounded "half X" compared against: the exact half (CR 107.1a
 * concerns generated numbers; a comparison generates none). A stated
 * rounding keeps the ordinary amount. */
function parseExactHalf(c: Cursor): Amount | null {
  return c.attempt((c): Amount | null => {
    if (c.word("half") === null) return null;
    const of = parseAmountTerm(c);
    if (!of || c.attempt(c => c.punct(",") && c.isWord("rounded") ? true : null) || c.isWord("rounded", "plus", "minus")) return null;
    return { amount: "half", of, round: "exact" };
  });
}

export function parseComparison(c: Cursor): Comparison | null {
  return c.attempt((c): Comparison | null => {
    const half = c.attempt((c): Comparison | null => {
      const op = c.words("less", "than", "or", "equal", "to") ? "le" : c.words("greater", "than", "or", "equal", "to") ? "ge"
        : c.words("less", "than") ? "lt" : c.words("greater", "than") ? "gt" : null;
      const value = op && parseExactHalf(c);
      return value ? { op, value } : null;
    });
    if (half) return half;
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

const PLURAL_PERSONAL_ZONES: Record<string, string> = { hand: "hands", graveyard: "graveyards", library: "libraries" };

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
    // "a player's graveyard": whichever player's it is (CR 400.1); a later
    // "that player" is that owner, so the player stays a named participant.
    else if (c.attempt((c) => (c.word("a") !== null && c.possessive("player") !== null ? true : null)))
      owner = { player: "a-player" };
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
    if (c.word("all") !== null) { owner = c.possessive("opponents") !== null ? "all-opponents" : (c.possessive("players"), "all-players"); plural = true; }
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
  // "permanent or suspended card": the status of a card (CR 702.62b).
  const afterFirst = c.peek(afterJoin + 1);
  if (first.value === "suspended" && afterFirst?.kind === "word" && (afterFirst.value === "card" || afterFirst.value === "cards")) return "branch";
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

/** "an artifact or non-Aura enchantment card from your graveyard", "a Dragon
 * creature card or an Ugin planeswalker card from your graveyard": the card
 * head and the zone phrase close the whole coordinated description, so each
 * alternative is a card in that zone. Only plain characteristic words may
 * precede the closing phrase; any other trailing qualifier ("with mana value
 * 3 or less") stays unsupported because its reach is not stated. */
const SHARED_CARD_KEYS = new Set(["types", "subtypes", "nonTypes", "nonSubtypes", "supertypes", "nonSupertypes", "colors", "nonColors", "cls", "zone"]);
function shareClosingCardZone(first: ObjectFilter, second: ObjectFilter): ObjectFilter {
  if (second.cls !== "card" || !second.zone || first.zone || (first.cls !== undefined && first.cls !== "card")) return first;
  const plain = (filter: ObjectFilter, ignore: string[]) => Object.keys(filter).every(key => SHARED_CARD_KEYS.has(key) || ignore.includes(key));
  if (!plain(first, ["other"]) || !plain(second, [])) return first;
  return { ...first, cls: "card", zone: second.zone };
}

function joinFilterBranches(first: ObjectFilter, second: ObjectFilter, articled = false): ObjectFilter | null {
  // "a Mount creature card or a Plains card from among them": the pool a
  // selection is made from is the source of every alternative, exactly as
  // in "an Aura or Equipment card from among them" (CR 608.2c).
  const pooled = hoistSelectionPool(first, [second], articled);
  if (pooled) {
    const joined = joinFilterBranches(first, pooled.members[0]);
    return joined && { ...joined, fromAmong: pooled.fromAmong };
  }
  first = shareClosingCardZone(first, second);
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

/** The last alternative's trailing "from among …" pool, removed from it,
 * when no other alternative names a pool of its own. */
export function hoistSelectionPool(first: ObjectFilter, members: ObjectFilter[], articled = false): { fromAmong: ObjectRef; members: ObjectFilter[] } | null {
  const last = members.at(-1)!;
  if (!last.fromAmong || hasFilterScope(first, "fromAmong") || members.slice(0, -1).some(member => hasFilterScope(member, "fromAmong"))) return null;
  const { fromAmong, ...rest } = last;
  if (hasFilterScope(rest, "fromAmong")) return null;
  // Without its own article, a last alternative's postnominal qualifier
  // ("land and/or legendary permanent cards with mana value X or less")
  // may describe every alternative; that scope stays unsupported.
  if (!articled && (Object.keys(rest) as (keyof ObjectFilter)[]).some(key => !PRENOMINAL_KEYS.has(key))) return null;
  return { fromAmong, members: [...members.slice(0, -1), rest] };
}
const PRENOMINAL_KEYS = new Set<keyof ObjectFilter>(["cls", "types", "subtypes", "supertypes", "colors", "nonColors", "colorless", "multicolored",
  "monocolored", "nonTypes", "nonSubtypes", "nonSupertypes", "nonToken", "status", "permanent", "allOf"]);

/** CR 113.3, 605.1 and 606.1: ability category, mana-ability status,
 * loyalty-ability status, and source identity are separate constraints. */
export function parseAbilityHead(c: Cursor): ObjectFilter | null {
  return c.attempt(c => {
    const f: ObjectFilter = { cls: "ability" };
    const mana = c.word("mana", "nonmana", "non-mana");
    if (mana) f.manaAbility = mana === "mana";
    // CR 702.142a, 702.177a, 702.49, 702.193: abilities named by their keyword.
    // 702.107 outlast, 702.128 embalm, 702.129 eternalize.
    const keywords = ["boast", "exhaust", "ninjutsu", "power-up", "outlast", "embalm", "eternalize"];
    const keyword = mana ? null : c.word(...keywords);
    if (keyword) {
      // "an eternalize or embalm ability": either keyword's ability.
      const alternatives = [keyword];
      for (let next = c.attempt(c => c.word("or") !== null ? c.word(...keywords) : null); next; next = c.attempt(c => c.word("or") !== null ? c.word(...keywords) : null))
        alternatives.push(next);
      if (c.word("ability", "abilities") === null) return null;
      if (alternatives.length === 1) f.abilityKeyword = keyword;
      else f.anyOf = alternatives.map(abilityKeyword => ({ abilityKeyword }));
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
export function parseObjectRelationPredicate(c: Cursor): ObjectFilter | null {
  return c.attempt(c => {
    if (c.words("attached", "to")) {
      const attachedTo = parsePlayerRef(c) ?? parseRelationObject(c);
      return attachedTo && { attachedTo };
    }
    if (c.word("attacking") !== null) {
      // CR 508.1b / 506.4c: the player or permanent each matching creature
      // is attacking; printed alternatives are retained separately.
      const defender = (c: Cursor): CombatDefender | null => c.attempt((c): CombatDefender | null => {
        // "attacking one of your opponents": whichever single opponent it attacks.
        if (c.words("one", "of", "your", "opponents")) return { defender: "player", who: { player: "an-opponent" } };
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
const OUTLAW_TYPES = ["Assassin", "Mercenary", "Pirate", "Rogue", "Warlock"];

export function parseFilter(c: Cursor): ObjectFilter | null {
  return parseFilterExpression(c, false);
}

/** Filter governed by one singular "each" ("each creature and planeswalker
 * your opponents control", "for each Aura and Equipment attached to it").
 * The single determiner covers both singular nouns, so they are one union of
 * candidates and the trailing qualifiers restrict every member; an object
 * that is both is still one object (CR 608.2c). Only bare nouns coordinate
 * this way: a prenominal modifier on either noun leaves its scope open. */
export function parseEachFilter(c: Cursor): ObjectFilter | null {
  return parseFilterExpression(c, false, true);
}

/** "for each A you control and each B card in your graveyard" as one count
 * over two pools that cannot overlap (see pooled-counts.ts). */
export function parsePooledEachFilter(c: Cursor): ObjectFilter | null {
  return c.attempt((c): ObjectFilter | null => {
    const first = parseEachFilter(c);
    return first && (parsePooledFilter(c, first, parseEachFilter, true) ?? first);
  });
}

/** A relative predicate may be adjective-only ("that's white or blue").
 * Ordinary object selections still require a noun. */
function parseFilterExpression(c: Cursor, predicate: boolean, eachUnion = false, listItem = false): ObjectFilter | null {
  return c.attempt((c): ObjectFilter | null => {
    const filterStart = c.pos;
    const repeated = parseRepeatedCardUnionHead(c) ?? parseMixedCardAttributeUnionHead(c);
    const f: ObjectFilter = repeated ?? {};
    let sawNominal = !!repeated;
    let independentAlternative = false;
    let creatureTail: ObjectFilter | null = null;
    let exiledPrefix = false;

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
      // "target 1/1 creature": exactly that power and toughness (CR 208.1).
      if (t?.kind === "number" && !sawNominal && c.pos === filterStart && isTargetWord(c.tokens[filterStart - 1])) {
        const stats = c.attempt((c): PropertyConstraint[] | null => {
          const power = c.number();
          if (power === null || !c.punct("/")) return null;
          const toughness = c.number();
          if (toughness === null || !c.isWord("creature", "creatures")) return null;
          return [{ property: "power", comparison: { op: "eq", value: { amount: "fixed", value: power } } },
            { property: "toughness", comparison: { op: "eq", value: { amount: "fixed", value: toughness } } }];
        });
        if (stats) { f.properties = stats; continue; }
      }
      if (!t || t.kind !== "word" || t.possessive) break;
      const v = t.value;

      // In "a land enchanted player controls", enchanted starts the
      // controller reference, not an adjective on the land. Likewise an
      // attached object's possessive controller must reach the qualifier.
      const following = c.peek(1);
      if (sawNominal && ["enchanted", "equipped"].includes(v) && following?.kind === "word"
        && (following.value === "player" || following.possessive)) break;
      if (sawNominal && v === "other" && following?.kind === "word" && following.value === "than") break;
      // "creatures tapped this way" names what an instruction tapped, not
      // creatures that are currently tapped.
      if (sawNominal && v === "tapped" && following?.kind === "word" && following.value === "this") {
        const way = c.peek(2);
        if (way?.kind === "word" && way.value === "way") break;
      }

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
      // CR 701.54b: Ring-bearer is a designation, not a type.
      if (v === "ring-bearer" || v === "ring-bearers") {
        c.pos++;
        f.ringBearer = true;
        sawNominal = true;
        continue;
      }
      // CR 123.4: "stickered" describes an object with a sticker on it now.
      if (v === "stickered" && following?.kind === "word" && !following.possessive) {
        const nominal = classifyNominal(following.value, following.raw);
        if (!nominal || nominal.cls === "player") return c.fail("stickered object");
        c.pos++;
        f.sticker = "any";
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
      // CR 700.12: an outlaw has the Assassin, Mercenary, Pirate, Rogue
      // and/or Warlock creature type; the bare noun is a permanent (700.12a).
      if (v === "outlaw" || v === "outlaws" || v === "non-outlaw") {
        c.pos++;
        if (v === "non-outlaw") { (f.nonSubtypes ??= []).push(...OUTLAW_TYPES); continue; }
        (f.allOf ??= []).push({ anyOf: OUTLAW_TYPES.map(subtype => ({ subtypes: [subtype] })) });
        const head = c.peek();
        const nominal = v === "outlaw" && head?.kind === "word" && !head.possessive ? classifyNominal(head.value, head.raw) : null;
        if (!nominal || nominal.cls === "player") {
          if (f.cls && f.cls !== "permanent") return c.fail("an outlaw permanent");
          f.cls = "permanent";
          sawNominal = true;
        }
        continue;
      }
      // CR 702.33d: a kicked spell.
      if (v === "kicked" && following?.kind === "word" && !following.possessive && (following.value === "spell" || following.value === "spells")) {
        c.pos++;
        f.kicked = true;
        continue;
      }
      // "exiled cards", "face-up exiled cards you own": cards in exile (CR 406.1).
      // Only cards are exiled in this sense; "exiled creatures" would need
      // another reading, so a non-card noun fails below.
      // A bare "exiled cards", or one after a definite or demonstrative
      // determiner ("those exiled cards"), names the result of an earlier
      // exile, which is another reference.
      const determiner = c.tokens[filterStart - 1];
      const definite = determiner?.kind !== "word"
        || ["the", "those", "these", "that", "this", "its", "their"].includes(determiner.value);
      if (v === "exiled" && !sawNominal && !definite && !f.zone && following?.kind === "word" && !following.possessive) {
        c.pos++;
        f.zone = { zone: "exile" };
        exiledPrefix = true;
        continue;
      }
      // CR 702.62b: a suspended card is in exile, has suspend and has a time
      // counter on it; a defined state of a card, like the condition status.
      if (v === "suspended" && following?.kind === "word" && !following.possessive && (following.value === "card" || following.value === "cards")) {
        c.pos++;
        (f.status ??= []).push("suspended");
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
          // "a noncreature or Dragon spell": either test, then the shared head.
          const union = c.attempt((c): ObjectFilter | null => {
            c.pos++;
            if (c.word("or") === null) return null;
            const alternative = c.peek();
            const nominal = alternative?.kind === "word" && !alternative.possessive ? classifyNominal(alternative.value, alternative.raw) : null;
            const head = c.peek(1);
            const sharedHead = head?.kind === "word" && !head.possessive ? classifyNominal(head.value, head.raw) : null;
            if (!nominal || (nominal.cls !== "type" && nominal.cls !== "subtype") || sharedHead?.cls !== "class") return null;
            c.pos++;
            return { anyOf: [{ nonTypes: [restSingular] }, nominal.cls === "type" ? { types: [nominal.word] } : { subtypes: [nominal.word] }] };
          });
          if (union) { (f.allOf ??= []).push(union); continue; }
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
        // "nonattacking", "nonblocking": not attacking/blocking now (CR 506.4),
        // the same test as "that isn't attacking".
        if (rest === "attacking" || rest === "blocking") {
          c.pos++;
          (f.nonStatus ??= []).push(rest);
          continue;
        }
        // CR 506.4: "nonattacking" / "nonblocking" — not an attacking
        // (blocking) creature; no printed antonym status exists.
        if (rest === "attacking" || rest === "blocking") {
          c.pos++;
          (f.nonStatus ??= []).push(rest);
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
        creatureTail = null;
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
            // A modifier before the first noun ("Artifact creatures and Heroes")
            // may or may not reach the second; only "other" is known to.
            const plural = Object.keys(f).every(key => key === "other") && barePluralPair(t, nominal.word, nt, n2.word, c.tokens[filterStart - 1]);
            const singularUnion = eachUnion && Object.keys(f).every(key => key === "other")
              && [nominal, n2].every(noun => noun.cls === "type" || noun.cls === "subtype")
              && t.value === nominal.word.toLowerCase() && nt.value === n2.word.toLowerCase();
            if (!sharedHead && !plural && !singularUnion) return null;
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
          const branches = alternatives.map((n): ObjectFilter =>
            n.cls === "type" ? { types: [n.word] } : n.cls === "class" ? { cls: n.word as ObjectFilter["cls"] } : { subtypes: [n.word] });
          (f.allOf ??= []).push({ anyOf: branches });
          // "artifact, enchantment, or creature with flying": a keyword or
          // power/toughness test after bare card types that end in "creature"
          // describes the creature alternative only (Broken Wings can destroy
          // any artifact or enchantment). A following shared head noun
          // ("… cards") cancels this.
          creatureTail = alternatives.every(n => n.cls === "type") && alternatives.at(-1)!.word === "creature"
            && alternatives.slice(0, -1).every(n => n.word !== "creature") ? branches.at(-1)! : null;
        }
        continue outer;
      }

      break;
    }

    if (exiledPrefix && f.cls !== "card") return c.fail("exiled cards");
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
        const connector = c.word("and", "or");
        const conjunction = connector === "and";
        // "creatures you control each with power greater than …": a filter
        // already tests each object, so the distributive "each" adds nothing.
        const next = c.peek(1);
        if (!connector && c.isWord("each") && next?.kind === "word" && next.value === "with") c.pos++;
        const result = parseFilterQualifier(c, f);
        if (!result) return null;
        // "... on the battlefield or in the command zone": "or" joins only a second zone.
        if (connector === "or" && !(Object.keys(result).length === 1 && result.zone && f.zone)) return null;
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
      const creatureOnly = keys.length === 1 && (keys[0] === "withAbility" || keys[0] === "withoutAbility"
        || (keys[0] === "properties" && qualifier.properties!.every(property => property.property !== "mana-value")));
      if (creatureTail && creatureOnly) { Object.assign(creatureTail, qualifier); creatureTail = null; continue; }
      creatureTail = null;
      if (keys.length === 1 && qualifier.zone && f.zone) {
        // No object is in two zones at once: "cards you own in exile and in
        // your graveyard" counts the cards in either zone (Crackling Drake).
        (f.allOf ??= []).push({ anyOf: [{ zone: f.zone }, { zone: qualifier.zone }] });
        delete f.zone;
        continue;
      }
      if ((qualifier.otherThan && f.other) || keys.some(key => f[key] !== undefined)) (f.allOf ??= []).push(qualifier);
      else Object.assign(f, qualifier);
    }

    if (listItem) return f;
    if (!predicate) {
      const listed = parseIndependentFilterList(c, f, { item: c => parseFilterExpression(c, false, false, true), scoped: hasFilterScope, hoist: hoistSelectionPool });
      if (listed) return listed;
    }
    // Independently repeated noun phrases retain their own qualifiers. The
    // earlier nominal OR handles shared suffixes ("instant or sorcery card").
    // Do not interpret general AND as intersection: it can enumerate sets.
    if (qualified || independentAlternative || predicate) {
      // "that's an instant card, a sorcery card, or a land card": a comma list
      // of predicate descriptions that ends with its own "or" (an unterminated
      // list stays unsupported).
      const listed = predicate && !independentAlternative ? c.attempt((c): ObjectFilter | null => {
        if (!c.punct(",") || c.word("a", "an") === null) return null;
        const rest = parseFilterExpression(c, true);
        return rest?.anyOf && Object.keys(rest).length === 1 ? rest : null;
      }) : null;
      if (listed) {
        const { other, ...term } = f;
        return { ...(other ? { other } : {}), anyOf: [term, ...listed.anyOf!] };
      }
      const alternative = c.attempt(c => {
        if (independentAlternative) c.punct(",");
        const join = parseLogicalJoin(c);
        if (join !== "or" && join !== "and-or") return null;
        if (join === "and-or" && c.isWord("a", "an", "another")) return null;
        const articled = c.word("a", "an") !== null;
        const filter = parseFilterExpression(c, predicate);
        return filter && { filter, articled };
      });
      if (alternative) return joinFilterBranches(f, alternative.filter, alternative.articled) ?? c.fail("explicit scope for alternative filter qualifiers");
      if (independentAlternative) return c.fail("complete alternative filter");
    }
    return f;
  });
}

const PERSONAL_ZONE_WORDS = new Set(["hand", "graveyard", "library", "hands", "graveyards", "libraries"]);

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
    const zoneHistory = parseZoneChangeHistoryQualifier(c, head, { parseZoneRef, parsePossessivePlayer });
    if (zoneHistory) return zoneHistory;
    const pastControl = parsePastControlQualifier(c, head, { parsePlayerRef, parseQualifier: parseFilterQualifier });
    if (pastControl) return pastControl;
    const costTapper = parseCostTapperQualifier(c, parseObjectRef);
    if (costTapper) return head.relation ? null : costTapper;
    const parity = parseParityQualifier(c);
    if (parity) return head.parity ? null : parity;
    const predicate = c.attempt(c => {
      if (c.word("that's") === null && !c.words("that", "is") && !c.words("that", "are")) return null;
      if (c.word("both") !== null) {
        // "both red and white" states the conjunction of exactly two colors.
        const both = parseFilterExpression(c, true);
        return both && Object.keys(both).length === 1 && both.colors?.length === 2 ? both : null;
      }
      const colorSet = parseColorSetPredicate(c);
      if (colorSet) return colorSet;
      c.word("a", "an");
      return parseObjectRelationPredicate(c) ?? parseFilterExpression(c, true);
    });
    if (predicate) return predicate;
    const negatedPredicate = parseNegatedPredicate(c, c => parseFilterExpression(c, true));
    if (negatedPredicate) return negatedPredicate;
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
    // CR 702.167c: "an exiled card used to craft it" (the zone came from the prenominal "exiled").
    const craftMaterial = c.attempt((c): ObjectFilter | null => {
      if (head.cls !== "card" || head.zone?.zone !== "exile" || head.zone.owner !== undefined || !c.words("used", "to", "craft")) return null;
      const of = parseObjectRef(c);
      return of ? { relation: { relation: "craft-material", of } } : null;
    });
    if (craftMaterial) return craftMaterial;
    const otherThanKinds = parseOtherThanDescriptions(c, parseFilter);
    // Excluding the very type being selected would select nothing.
    if (otherThanKinds) return otherThanKinds.nonTypes?.some(type => head.types?.includes(type)) ? c.fail("excluded kind that is not part of the selection") : otherThanKinds;
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
    const valueOfChoice = parseValueOfChoiceQualifier(c);
    if (valueOfChoice) return { valueOfChoice };
    const thatChoice = parseThatChoiceQualifier(c, parseFilter);
    if (thatChoice) return thatChoice;
    const chosenName = parseChosenNameQualifier(c, parseFilter, "with");
    if (chosenName) return { chosenName };
    // CR 115.9c: "a single creature" restates the one-target requirement
    // that "targets only" already carries.
    const targetsOnly = c.attempt((c): ObjectRef | PlayerRef | null => {
      if (c.word("that") === null || c.word("target", "targets") === null || c.word("only") === null) return null;
      const single = c.attempt(c => c.words("a", "single") ? parseFilter(c) : null);
      if (single) return { ref: "filter", filter: single, count: { amount: "fixed", value: 1 } };
      return c.attempt(parsePlayerRef) ?? parseObjectRef(c);
    });
    if (targetsOnly) return { targetsOnly };
    if ((head.cls === "spell" || head.cls === "ability" || head.allOf?.some(part => part.anyOf?.some(branch => branch.cls === "spell" || branch.cls === "ability")))
        && c.words("with", "a", "single", "target")) return { singleTarget: true };
    const targets = c.attempt(c => {
      if (c.word("that") === null || c.word("target", "targets") === null) return null;
      // "targets one or more creatures": the matching targets number one or
      // more; a following "that many" / "those" counts them (CR 115.1).
      const some = c.attempt((c): ObjectRef | null => {
        if (!c.words("one", "or", "more")) return null;
        const filter = parseFilter(c);
        return filter && { ref: "filter", filter, count: { amount: "chosen", minimum: 1 } };
      });
      return some ?? c.attempt(parsePlayerRef) ?? parseObjectRef(c);
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
    if (c.words("not", "named")) {
      const name = c.peek();
      if (name?.kind !== "card-name") return null;
      c.pos++;
      return { notName: { name: "literal", value: name.value } };
    }
    if (c.word("named") !== null) {
      const name = c.peek();
      if (name?.kind !== "card-name") return null;
      c.pos++;
      return { name: { name: "literal", value: name.value } };
    }
    // Control (CR 108.4) and ownership (108.3) are independent; "but" joins
    // the two requirements on the same object.
    if (c.words("you", "control", "but", "don't", "own")) return { control: "you", own: "not-you" };
    if (c.words("you", "own", "but", "don't", "control")) return { own: "you", control: "not-you" };
    // "you (both) own and control": both requirements, "both" only stresses it.
    if (c.words("you", "own", "and", "control") || c.words("you", "both", "own", "and", "control")) return { own: "you", control: "you" };
    if (c.words("you", "control")) return { control: "you" };
    // "creatures you controlled as you cast this spell": the past tense is
    // control at that moment. The enclosing definition must consume and
    // record the moment (Sentence.bindings asOf), or the line fails.
    if (c.attempt(c => {
      if (!c.words("you", "controlled")) return null;
      const moment = c.tokens.slice(c.pos, c.pos + 5).map(t => t.kind === "word" ? t.value : "").join(" ");
      return moment === "as you cast this spell" || moment === "as you activate this ability" ? true : null;
    })) return { control: "you" };
    if (c.words("you", "don't", "control")) return { control: "not-you" };
    if (c.words("an", "opponent", "controls") || c.words("your", "opponents", "control")) return { control: "opponent" };
    if (c.words("that", "player", "controls") || c.words("that", "opponent", "controls")) return { control: "that-player" };
    if (c.words("you", "own")) return { own: "you" };
    if (c.words("you", "don't", "own")) return { own: "not-you" };
    // CR 123.4: "with a sticker on it" is a stickered object; a named kind
    // requires at least one sticker of that kind (123.1).
    const sticker = c.attempt(c => {
      if (c.word("with") === null) return null;
      const kind = parseStickerKind(c);
      return kind && c.word("on") !== null && c.word("it", "them") !== null ? kind : null;
    });
    if (sticker) return { sticker };
    // "with {X} in its mana cost" (CR 107.3, 202.1).
    const manaCostContains = c.attempt(c => {
      if (c.word("with") === null) return null;
      const token = c.peek();
      if (token?.kind !== "symbol" || token.value !== "X") return null;
      c.pos++;
      return c.words("in", "its", "mana", "cost") || c.words("in", "their", "mana", "costs") ? [token.value] : null;
    });
    if (manaCostContains) return { manaCostContains };
    // CR 102.4: "creatures your team controls".
    if (c.words("your", "team", "controls")) return { controlledBy: { player: "your-team" } };
    const controlledBy = c.attempt(c => {
      const who = parsePlayerRef(c);
      if (!who || c.word("control", "controls") === null) return null;
      return who;
    });
    if (controlledBy) return { controlledBy };
    // CR 108.3: the owner is fixed, whoever controls the object and whatever
    // zone it is in. "An opponent" / "your opponents" is any opponent of yours.
    if (c.words("an", "opponent", "owns") || c.words("your", "opponents", "own")) return { own: "opponent" };
    const ownedBy = c.attempt(c => {
      const who = parsePlayerRef(c);
      if (!who || c.word("own", "owns") === null) return null;
      return who;
    });
    if (ownedBy) return { ownedBy };
    const fromAmong = c.attempt(c => {
      if (!c.words("from", "among")) return null;
      return parseObjectRef(c);
    });
    if (fromAmong) return { fromAmong };
    if (c.words("from", "outside", "the", "game")) return { outsideGame: true };
    let preposition: string | null = null;
    const zone = c.attempt(c => {
      preposition = c.word("in", "from");
      if (preposition === null) return null;
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
        // A following "and"/"or" that adds a third zone is not a two-zone
        // union; one that starts the next verb ("… or graveyard and exile it")
        // ends the zone phrase.
        const after = c.peek(1);
        const more = c.isWord("or", "and") && after?.kind === "word" && PERSONAL_ZONE_WORDS.has(after.value);
        if (!second || second === zone.zone || more || c.isPunct("/")) return null;
        return { zone: second, owner: zone.owner };
      });
      if (alternative) return { anyOf: [{ zone }, { zone: alternative }] };
      // "in graveyards and libraries": ownerless plural zones joined by and/or
      // (members of either zone, whoever owns them; CR 400.1).
      const pooled = c.attempt((c): ZoneRef | null => {
        if (preposition !== "in") return null;
        const previous = c.tokens[c.pos - 1];
        if (zone.owner || zone.position || previous?.kind !== "word" || previous.value !== PLURAL_PERSONAL_ZONES[zone.zone]) return null;
        if (c.word("and", "or") === null) return null;
        const word = c.word(...Object.values(PLURAL_PERSONAL_ZONES));
        const second = Object.keys(PLURAL_PERSONAL_ZONES).find(key => PLURAL_PERSONAL_ZONES[key] === word);
        if (!second || second === zone.zone || c.isWord("and", "or") || c.isPunct("/")) return null;
        return { zone: second as ZoneRef["zone"] };
      });
      return pooled ? { anyOf: [{ zone }, { zone: pooled }] } : { zone };
    }
    const fromPool = c.attempt(c => {
      if (c.word("from") === null) return null;
      if (c.word("it") !== null) return { ref: "it" as const };
      if (c.word("them") !== null) return { ref: "them" as const };
      return null;
    });
    if (fromPool) return { fromAmong: fromPool };
    // "in the pile of their choice" / "from the chosen pile" (CR 700.3b).
    const inPile = c.attempt(c => c.word("in", "from") !== null ? parsePileRef(c, parsePossessivePlayer) : null);
    if (inPile) return { fromAmong: inPile };
    if (c.words("on", "the", "battlefield")) return { zone: { zone: "battlefield" } };
    if (c.words("that", "aren't", "on", "the", "battlefield") || c.words("that", "isn't", "on", "the", "battlefield"))
      return { outsideZone: { zone: "battlefield" } };
    // "with power or toughness 4 or greater": either value satisfies the
    // one stated comparison.
    const eitherStat = c.attempt((c): ObjectFilter | null => {
      if (!c.words("with", "power", "or", "toughness")) return null;
      const comparison = parseComparison(c);
      return comparison && { anyOf: [{ properties: [{ property: "power", comparison }] },
        { properties: [{ property: "toughness", comparison: structuredClone(comparison) }] }] };
    });
    if (eitherStat) return eitherStat;
    // "an ability … without {T} in its activation cost" (CR 107.5).
    if (head.cls === "ability") {
      const tapSymbolInCost = c.attempt(c => {
        const preposition = c.word("with", "without");
        return preposition && c.symbol() === "T" && c.words("in", "its", "activation", "cost") ? preposition === "with" : null;
      });
      if (tapSymbolInCost !== null) return { tapSymbolInCost };
    }
    const baseStat = parseBaseStatQualifier(c, parseComparison);
    if (baseStat) return baseStat;
    // CR 715.1: an Adventurer card has an Adventure (a second set of
    // characteristics); "has an Adventure" tests for that.
    if (c.words("that", "has", "an", "adventure") || c.words("that", "have", "an", "adventure") || c.words("with", "an", "adventure")) return { adventure: true };
    const comparative = parseComparativeAttribute(c, { parseObjectRef });
    if (comparative) return comparative;
    const sameController = parseSameController(c);
    if (sameController) return sameController;
    const tied = parseTiedExtremum(c, head);
    if (tied) return tied;
    const property = parsePropertyConstraint(c);
    // "a creature with the greatest power among creatures that player
    // controls" is one of those creatures, not any creature with that power.
    if (property && poolMembers.has(property)) {
      const value = property.comparison.value as Extract<Amount, { amount: "attribute-extremum" }>;
      return { properties: [property], allOf: [structuredClone(value.among)] };
    }
    if (property) return { properties: [property] };
    // "cards exiled this way that weren't cast" leaves "this way" to the
    // preceding participle; the cast meant is the same one.
    if (c.words("not", "cast", "this", "way") || c.words("that", "weren't", "cast", "this", "way")
      || c.words("that", "wasn't", "cast", "this", "way") || c.words("that", "weren't", "cast")) return { notCastThisWay: true };
    const zoneChange = parseZoneChangeRelation(c, parseZoneRef);
    if (zoneChange) return { relation: zoneChange };
    const eventRelation = parseObjectRelation(c) ?? parseDevouredRelation(c, parseObjectRef) ?? parseCountersPutRelation(c, parseCounterSpec);
    if (eventRelation) return { relation: eventRelation };
    return parseCounterConstraint(c) ?? parseAbilityConstraint(c, head.cls === "spell");
  });
}

/** CR 122.1: presence means at least one counter, absence means zero. Counter
 * kinds are kept separate and these constraints bind to each selected object.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
function parseCounterConstraint(c: Cursor, predicate = false): ObjectFilter | null {
  return c.attempt(c => {
    // "that has a +1/+1 counter on it" / "that doesn't have …" restate
    // "with" / "without" as a relative clause.
    const connector = predicate ? c.word("has", "have", "had") : c.word("with", "without")
      ?? (c.words("that", "has") || c.words("that", "have") ? "with" : null)
      ?? (c.words("that", "doesn't", "have") || c.words("that", "don't", "have") ? "without" : null);
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
      // A bare plural P/T kind ("-1/-1 counters") must not start a number.
      const start = c.pos;
      const bareKind = c.attempt(c => {
        const spec = parseCounterSpec(c);
        return spec && "type" in spec && spec.type.includes("/") && c.isWord("counters") ? true : null;
      });
      c.pos = start;
      comparison = (bareKind ? null : parseComparison(c)) ?? { op: "ge", value: { amount: "fixed", value: 1 } };
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
    if (mode === "with" && c.words("no", "abilities")) return { hasAbilities: false };
    // "with a morph ability" / "with morph abilities" name the keyword ability.
    const indefinite = c.word("a", "an") !== null;
    const first = parseKeywordName(c) ?? parseLandwalkName(c)?.name ?? null;
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
        const name = parseKeywordName(c) ?? parseLandwalkName(c)?.name ?? null;
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

/** Constraints stated as "with the greatest power among <pool>": the object
 * is a member of that pool, unlike "power less than the greatest power". */
const poolMembers = new WeakSet<PropertyConstraint>();

/** "the creature with the greatest power or tied for the greatest power":
 * every creature whose power equals the greatest power among all creatures
 * (CR 107.2; the tie clause admits each tied creature, which the plain
 * value comparison already does). Only the bare creature head names the
 * compared set unambiguously. */
function parseTiedExtremum(c: Cursor, head: ObjectFilter): ObjectFilter | null {
  return c.attempt((c): ObjectFilter | null => {
    if (Object.keys(head).length !== 1 || head.types?.length !== 1 || head.types[0] !== "creature") return null;
    if (!c.words("with", "the")) return null;
    const adjective = c.word("greatest", "least");
    const attribute = adjective && c.word("power", "toughness");
    if (!adjective || (attribute !== "power" && attribute !== "toughness")) return null;
    if (!c.words("or", "tied", "for", "the", adjective, attribute)) return null;
    const value: Amount = { amount: "attribute-extremum", extremum: adjective === "greatest" ? "maximum" : "minimum", attribute, among: { types: ["creature"] } };
    return { properties: [{ property: attribute, comparison: { op: "eq", value } }] };
  });
}

/** "with power 2 or less", "with mana value 3 or greater" */
function parsePropertyConstraint(c: Cursor, past = false): PropertyConstraint | null {
  return c.attempt((c): PropertyConstraint | null => {
    // A bare "has base power 2" is a continuous effect on the subject
    // ("target creature has base power 2 until end of turn"), not a filter.
    const bareHas = !c.isWord("with", "of", "that");
    if (!c.words("with") && !c.words("of") && !c.words("that", "has") && !c.words("that", "have") && c.word("has", "have") === null && !(past && c.word("had") !== null)) return null;
    if (bareHas && c.isWord("base")) return null;
    const extremum = parseAttributeExtremum(c);
    if (extremum) {
      const constraint: PropertyConstraint = { property: extremum.attribute, comparison: { op: "eq", value: extremum } };
      poolMembers.add(constraint);
      return constraint;
    }
    // "with that spell's mana value": equal to that one object's value,
    // the same constraint as "with mana value equal to that spell's mana value".
    const shared = c.tokens[c.pos - 1]?.kind === "word" && (c.tokens[c.pos - 1] as { value: string }).value === "with" ? c.attempt((c): PropertyConstraint | null => {
      const of = parsePossessiveObject(c);
      const property = of && (c.words("mana", "value") ? "mana-value" : c.word("power", "toughness") as "power" | "toughness" | null);
      return of && property ? { property, comparison: { op: "eq", value: { amount: "attribute", of, attribute: property } } } : null;
    }) : null;
    if (shared) return shared;
    let property: PropertyConstraint["property"];
    const base = c.word("base") !== null;
    if (!base && c.words("mana", "value")) property = "mana-value";
    else {
      const w = c.word("power", "toughness");
      if (!w) return null;
      property = base ? (w === "power" ? "base-power" : "base-toughness") : w as "power" | "toughness";
    }
    // "base power and toughness" without the N/N form is not one comparison.
    if (base && c.isWord("and")) return null;
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
  if ((ref.player === "target-player" || ref.player === "target-opponent") && ref.count) return true;
  return ref.player === "filtered" ? isPluralPlayerRef(ref.base)
    : ref.player === "each-player" || ref.player === "each-opponent" || ref.player === "those-players";
}

/** Each candidate supplies the hand/life/control subject, and is the actor
 * of an event outcome ("who didn't discard a card this way"). */
function parsePlayerQualifier(c: Cursor, base?: PlayerRef): PlayerFilter | null {
  return c.attempt(c => {
    const most = parseMostPlayerQualifier(c, { parseFilter }, base);
    if (most) return most;
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
    // "Each opponent attacking that player" (CR 506.2, 508.1b).
    if (c.word("attacking") !== null) {
      const attackingPlayer = parsePlayerBase(c);
      return attackingPlayer && ["you", "that-player", "enchanted-player"].includes(attackingPlayer.player) ? { attackingPlayer } : null;
    }
    const comparative = parseComparativePlayerQualifier(c, { parseAmount, parseCount, parseFilter, parsePlayerRef });
    if (comparative) return comparative;
    const precedingAction = parsePrecedingActionQualifier(c);
    if (precedingAction) return precedingAction;
    const eventResults = parseEventResultPlayerQualifier(c, { parseComparison, parseFilter, parseObjectRef });
    if (eventResults) return eventResults;
    const searched = parseSearchedPlayerQualifier(c, parseZoneRef);
    if (searched) return searched;
    const votedFor = parseVotedForQualifier(c, { parsePlayerRef });
    if (votedFor) return votedFor;
    // CR 119.3: "who lost life this turn" is any life-loss event that turn.
    const lifeChange = c.attempt(c => {
      if (c.word("who") === null) return null;
      const change = c.word("gained", "lost");
      return change && c.words("life", "this", "turn") ? { lifeChange: { change: change as "gained" | "lost", during: "this-turn" as const } } : null;
    });
    if (lifeChange) return lifeChange;
    // "who doesn't control an Elf" is "who controls no Elves".
    const controlsNone = c.attempt(c => {
      if (c.word("who") === null || c.word("doesn't", "don't") === null || c.word("control") === null || c.word("a", "an") === null) return null;
      const filter = parseFilter(c);
      return filter && { controls: { filter, comparison: { op: "eq" as const, value: { amount: "fixed" as const, value: 0 } } } };
    });
    if (controlsNone) return controlsNone;
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

/** Every eligibility clause after a player noun; null when there is none. */
export function parsePlayerQualifiers(c: Cursor, base?: PlayerRef): PlayerFilter | null {
  const filter: PlayerFilter = {};
  for (;;) {
    const qualifier = c.attempt(c => {
      c.word("and");
      return parsePlayerQualifier(c, base);
    });
    if (!qualifier) break;
    const keys = Object.keys(qualifier) as (keyof PlayerFilter)[];
    if (keys.some(key => filter[key] !== undefined)) (filter.allOf ??= []).push(qualifier);
    else Object.assign(filter, qualifier);
  }
  return Object.keys(filter).length ? filter : null;
}

export function parsePlayerRef(c: Cursor): PlayerRef | null {
  return c.attempt(c => {
    const labeled = parseLabeledPlayers(c);
    if (labeled) return labeled;
    const other = parseEachOtherPlayer(c);
    const base = other?.base ?? parsePlayerBase(c);
    if (!base) return null;
    const filter = parsePlayerQualifiers(c, base);
    if (other) return { player: "filtered", base, filter: { ...other.filter, ...filter } };
    return filter ? { player: "filtered", base, filter } : base;
  });
}

/** "target opponent chosen at random": the game picks the target (CR 601.2c). */
function randomTarget(c: Cursor): { random?: true } {
  return c.words("chosen", "at", "random") ? { random: true } : {};
}

function parsePlayerBase(c: Cursor): PlayerRef | null {
  return c.attempt((c): PlayerRef | null => {
    const target = c.peek();
    const chosen = parseChosenPlayer(c);
    if (chosen) return chosen;
    if (c.words("a", "player")) return { player: "a-player" };
    if (c.words("your", "team")) return { player: "your-team" };
    if (c.words("each", "player")) return { player: "each-player" };
    if (c.words("each", "opponent")) return { player: "each-opponent" };
    // "each of your opponents" / "one of your opponents": the same players as
    // "each opponent" / "an opponent" (CR 102.2-3).
    if (c.words("each", "of", "your", "opponents")) return { player: "each-opponent" };
    if (c.words("one", "of", "your", "opponents")) return { player: "an-opponent" };
    if (c.words("target", "player")) return { player: "target-player", ...randomTarget(c), ...targetSelectionProvenance(target) };
    if (c.words("target", "opponent")) return { player: "target-opponent", ...randomTarget(c), ...targetSelectionProvenance(target) };
    // "another target player": one more target word whose player differs from
    // a contrasted one (an earlier target, the trigger's player). The contrast
    // stays unresolved, exactly as PlayerFilter.other. A bare "another player"
    // keeps the readings of its own contexts (see parseTriggerPlayer).
    if (c.isWord("another") && c.peek(1)?.kind === "word" && (c.peek(1) as { value: string }).value === "target") {
      c.pos++;
      const targetWord = c.peek();
      c.pos++;
      const noun = c.word("player", "opponent");
      if (!noun) return null;
      return { player: "filtered", base: { player: noun === "player" ? "target-player" : "target-opponent", ...targetSelectionProvenance(targetWord) }, filter: { other: true } };
    }
    if (c.words("an", "opponent")) return { player: "an-opponent" };
    if (c.words("that", "player")) return parseOrThatObjectsController(c, parsePossessiveObject) ?? { player: "that-player" };
    // "that opponent" is the same contextual reference as "that player"; its
    // antecedent was introduced as an opponent (CR 102.2–3), nothing more.
    if (c.words("that", "opponent")) return { player: "that-player" };
    const definite = parseDefiniteMostPlayer(c, { parseFilter });
    if (definite) return definite;
    if (c.words("those", "players")) return { player: "those-players" };
    if (c.words("the", "player")) {
      // Definite descriptions such as "the player with the most life" are
      // not this bare contextual reference and must retain their qualifier.
      if (c.isWord("who", "whose", "with", "to")) return null;
      return { player: "the-player" };
    }
    // CR 725.1: the player who is the monarch.
    if (c.words("the", "monarch")) return { player: "the-monarch" };
    if (c.word("they") !== null) return { player: "they" };
    if (c.words("defending", "player")) return { player: "defending-player" };
    // CR 506.2: the active player during the combat phase.
    if (c.words("the", "attacking", "player") || c.words("that", "attacking", "player") || c.words("attacking", "player"))
      return { player: "attacking-player" };
    // CR 303.4b,e: an enchanted player is not this Aura's controller.
    // "Enchant opponent" (a separate line) already restricts what the Aura can
    // enchant, so "enchanted opponent" is the same player as "enchanted player".
    if (c.words("enchanted", "player") || c.words("enchanted", "opponent")) return { player: "enchanted-player" };
    if (c.word("you") !== null) return { player: "you" };
    const relational = parseRelationalPlayer(c, parseObjectRef);
    if (relational) return relational;
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

const isTargetWord = (token: Token | undefined) => token?.kind === "word" && !token.possessive && token.value === "target";

export function parseObjectRef(c: Cursor): ObjectRef | null {
  return parseObjectRefInternal(c, false);
}

function parseSelectionFilter(c: Cursor, enabled: boolean): { filter: ObjectFilter; groupConstraints?: GroupConstraint[] } | null {
  return c.attempt(c => {
    const filter = parseFilter(c);
    if (!filter) return null;
    if (!enabled) return { filter };
    const groupConstraints: GroupConstraint[] = [];
    // "controlled by the same opponent": each is controlled by an opponent,
    // and one player controls them all.
    if (!filter.control && !filter.controlledBy && c.words("controlled", "by", "the", "same", "opponent")) {
      filter.control = "opponent";
      groupConstraints.push({ constraint: "same-controller" });
    }
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

/** Postfix qualifiers printed after a phrase that interrupts a description
 * ("a creature of their choice with flying"). They restrict the same
 * candidates as qualifiers printed directly after the noun. A description
 * with alternative branches is left alone: the scope of a late qualifier
 * over its branches is not stated. */
export function parseResumedFilterQualifiers(c: Cursor, filter: ObjectFilter): ObjectFilter {
  if (filter.anyOf || filter.allOf) return filter;
  const result = structuredClone(filter);
  for (;;) {
    const qualifier = parseFilterQualifier(c, result);
    if (!qualifier) return result;
    const keys = Object.keys(qualifier) as (keyof ObjectFilter)[];
    if ((qualifier.otherThan && result.other) || keys.some(key => result[key] !== undefined)) (result.allOf ??= []).push(qualifier);
    else Object.assign(result, qualifier);
  }
}

/** "of an opponent's choice", "of their choice", "of that player's choice"
 * after a target: another player chooses it (CR 601.2c, 603.3d). Your own
 * choice is the default and never printed this way. */
function parseTargetChooser(c: Cursor): PlayerRef | null {
  return c.attempt(c => {
    if (c.word("of") === null) return null;
    const chooser = parsePossessivePlayer(c);
    if (!chooser || c.word("choice") === null) return null;
    return chooser.player === "an-opponent" || chooser.player === "they" || chooser.player === "that-player" ? chooser : null;
  });
}

function parseObjectRefInternal(c: Cursor, groups: boolean): ObjectRef | null {
  return c.attempt((c): ObjectRef | null => {
    const eachOf = parseEachOf(c, c => parseObjectRefInternal(c, groups));
    if (eachOf) return eachOf;
    const leaders = parseVoteLeaders(c, parseFilter);
    if (leaders) return leaders;
    // "Either of them" selects one member of the established pair or group,
    // exactly as "one of them" does.
    const either = c.attempt((c): ObjectRef | null => {
      if (!c.words("either", "of")) return null;
      const pool = parseObjectRef(c);
      return pool && isPluralObjectRef(pool) ? { ref: "from-pool", pool, count: { amount: "fixed", value: 1 } } : null;
    });
    if (either) return either;
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
    // CR 701.47c: the Army chosen by the preceding amass.
    if (c.words("the", "amassed", "army") || c.words("the", "army", "you", "amassed")) return { ref: "amassed-army" };
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
    if (c.attempt(c => c.word("all") !== null && c.possessive("opponents") !== null && c.word("graveyards") !== null ? true : null))
      return { ref: "zone-cards", zone: "graveyard", owner: { player: "each-opponent" } };
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
    // "cards equal to its power from the top of its owner's library": the top
    // N cards of that library (CR 401.1), the amount fixed as it resolves.
    const equalCount = c.attempt((c): ObjectRef | null => {
      if (c.word("cards") === null) return null;
      const count = parseEqualTo(c);
      if (!count || !c.words("from", "the", "top", "of")) return null;
      const of = parseZoneRef(c);
      if (of?.zone !== "library" || of.position) return null;
      return { ref: "library-cards", position: "top", count, of };
    });
    if (equalCount) return equalCount;
    const graveyardTop = parseGraveyardTopCards(c, { parseAmount, parseFilter, parseZoneRef });
    if (graveyardTop) return graveyardTop;
    const pile = parsePileRef(c, parsePossessivePlayer);
    if (pile) return pile;
    if (c.selfref()) return { ref: "self" };
    if (c.word("it", "itself") !== null) return { ref: "it" };
    if (c.word("them", "they") !== null) return { ref: "them" };

    const anyTarget = c.attempt((c): ObjectRef | null => {
      if (c.word("any") === null) return null;
      const token = c.peek();
      if (c.word("target") === null) return null;
      const chosenBy = parseTargetChooser(c);
      // CR 115.4: "any target that isn't a Dragon" excludes matching objects.
      const excluding = !chosenBy ? c.attempt(c => c.words("that", "isn't") && c.word("a", "an") !== null ? parseFilter(c) : null) : null;
      return { ref: "any-target", ...(chosenBy ? { chosenBy } : {}), ...(excluding ? { excluding } : {}), ...targetSelectionProvenance(token) };
    });
    if (anyTarget) return anyTarget;

    const chosenTargets = c.attempt((c): ObjectRef | null => {
      const count: Amount | null = c.words("any", "number", "of")
        ? { amount: "chosen", minimum: 0 }
        : c.words("one", "or", "two")
          ? { amount: "chosen", minimum: 1, maximum: { amount: "fixed", value: 2 } }
          : (groups || isTargetWord(c.peek(3)) || isTargetWord(c.peek(4)) && c.peek(3)?.kind === "word" && (c.peek(3) as { value: string }).value === "other") && c.words("one", "or", "more")
            ? { amount: "chosen", minimum: 1 } : null;
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
      if (noun.value === "emblem") return insideEmblem() ? { ref: "this", noun: "emblem" } : null;
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
    // "Whenever enchanted creature deals damage to a creature, destroy the
    // other creature": one definite participant of the event, contrasted with
    // another one; "other" stays the unresolved lexical contrast.
    const theOther = c.attempt((c): ObjectRef | null => {
      if (!c.words("the", "other")) return null;
      const filter = parseFilter(c);
      const noun = c.tokens[c.pos - 1];
      if (!filter || noun?.kind !== "word" || singularize(noun.value) !== noun.value) return null;
      if (Object.keys(filter).some(key => key !== "types" && key !== "subtypes" && key !== "cls") || filter.other) return null;
      return { ref: "that", filter: { ...filter, other: true } };
    });
    if (theOther) return theOther;

    const targetRef = c.attempt((c): ObjectRef | null => {
      const another = c.word("another") !== null;
      const upTo = c.words("up", "to");
      // "Up to that many target ...": the count is the symbolic anaphoric amount
      // (a repeated payment's count, the previous instruction's number).
      const thatMany = upTo ? c.attempt((c): Amount | null => c.words("that", "many") ? { amount: "that-much" } : null) : null;
      const count: Amount = thatMany ?? c.attempt(parseCount) ?? { amount: "fixed", value: 1 };
      const other = c.word("other") !== null;
      const token = c.peek();
      if (c.word("target") === null) return null;
      const selected = parseSelectionFilter(c, groups);
      if (!selected || !validSelectionCount(count)) return null;
      if (another || other) selected.filter.other = true;
      if (another && (upTo || count.amount !== "fixed" || count.value !== 1)) return null;
      // "up to four target cards from a player's graveyard": one player's
      // graveyard holds every target, as "from a single graveyard" says.
      const zone = selected.filter.zone;
      const owner = zone?.owner;
      if (zone && typeof owner === "object" && owner !== null && "player" in owner && owner.player === "a-player"
        && (upTo || count.amount !== "fixed" || count.value !== 1)) selected.filter.zone = { ...zone, owner: "any-single" };
      const chosenBy = !selected.groupConstraints ? parseTargetChooser(c) : null;
      if (chosenBy) selected.filter = parseResumedFilterQualifiers(c, selected.filter);
      return { ref: "target", ...selected, count, ...(upTo ? { upTo: true } : {}), ...(chosenBy ? { chosenBy } : {}), ...targetSelectionProvenance(token) };
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
      const filter = parseEachFilter(c);
      if (!filter) return null;
      return { ref: "each", filter: parseExceptFor(c, filter, parseFilter) ?? filter };
    });
    if (eachRef) return eachRef;

    const allRef = c.attempt((c): ObjectRef | null => {
      if (c.word("all") === null) return null;
      c.word("the");
      const filter = parseFilter(c);
      if (!filter) return null;
      return { ref: "all", filter: parseAllZoneUnion(c, filter) ?? parseExceptFor(c, filter, parseFilter) ?? filter };
    });
    if (allRef) return allRef;

    const equippedRef = c.attempt((c): ObjectRef | null => {
      const kind = c.word("equipped", "enchanted", "fortified");
      if (!kind) return null;
      const noun = c.anyWord();
      if (!noun) return null;
      // CR 301.6: a Fortification is attached to a land.
      if (kind === "fortified") return noun.value === "land" ? { ref: "fortified", noun: "land" } : null;
      const subtype = canonicalSubtypeName(noun.raw);
      // "enchanted Forest" denotes this Aura's attached object (303.4b),
      // whereas "enchanted Forests" is a filter over multiple objects.
      if (!THIS_NOUNS.has(noun.value) && (!subtype || noun.value !== subtype.toLowerCase())) return null;
      return { ref: kind as "equipped" | "enchanted", noun: noun.value };
    });
    if (equippedRef) return equippedRef;

    const numberOf = parseNumberOfSelection(c, { parseFilter, parseEqualTo });
    if (numberOf) return numberOf;

    // CR 107.1a: "half the creatures they control, rounded down" selects a
    // half-sized subset of that set; the rounding is stated by the step.
    const halfRef = c.attempt((c): ObjectRef | null => {
      if (!c.words("half", "the")) return null;
      const selected = parseSelectionFilter(c, groups);
      if (!selected || selected.groupConstraints) return null;
      const count = deferredHalf({ amount: "count", of: structuredClone(selected.filter) });
      return count && { ref: "filter", ...selected, count };
    });
    if (halfRef) return halfRef;

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

    const definite = parseDefiniteDescription(c, { parseFilter, number: relatedNumber });
    if (definite) return definite;

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
    if (c.attempt(c => c.word("enchanted") !== null && c.possessive("player", "opponent") !== null ? true : null))
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
    if (c.word("that") !== null && c.possessive("player", "opponent") !== null) return { player: "that-player" };
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
/** Participles that name an event result only with an explicit "this way"
 * link: tapped is otherwise a status and returned/drawn have no bare
 * definite form ("the tapped creature" is a current-state description). */
const LINKED_RESULT_EVENTS = {
  tapped: "tap", returned: "return", countered: "counter", milled: "mill", drawn: "draw", chosen: "choose",
} as const;
/** "The milled card(s)" does have a bare definite form: milled is never a
 * status, so it can only name the cards a mill just moved (CR 701.17a). */
const DEFINITE_RESULT_EVENTS = { ...RESULT_EVENTS, milled: "mill" } as const;

/** CR 607.1–2: "exiled with" is an explicit link, not an arbitrary object in
 * exile. Bare definite result references can be linked or within the same
 * resolution, so leave their contextual antecedent symbolic until binding.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
function parseObjectRelation(c: Cursor): ObjectRelation | null {
  return c.attempt(c => {
    const who = parsePlayerRef(c);
    const event = c.word("exiled", "sacrificed", "discarded", "revealed", "destroyed") as keyof typeof RESULT_EVENTS | null;
    if (!event && !who && c.words("created", "with")) {
      // CR 607.2c: linked to the token-making ability printed on the source.
      const source = parseObjectRef(c);
      return source && { relation: "linked-creation", source };
    }
    if (!event) {
      const linked = c.word("tapped", "returned", "countered", "milled", "drawn", "chosen") as keyof typeof LINKED_RESULT_EVENTS | null;
      if (!linked || !c.words("this", "way")) return null;
      return { relation: "event-result", event: LINKED_RESULT_EVENTS[linked], scope: "this-way", ...(who ? { who } : {}) };
    }
    let relation: ObjectRelation;
    if (c.words("this", "way")) relation = { relation: "event-result", event: RESULT_EVENTS[event], scope: "this-way" };
    else if (event === "exiled" && c.word("with") !== null) {
      const source = parseObjectRef(c);
      if (!source) return null;
      relation = { relation: "linked-exile", source };
    } else if (event === "exiled" && c.words("used", "to", "craft")) {
      const of = parseObjectRef(c);
      if (!of) return null;
      relation = { relation: "craft-material", of };
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
    } else if (number && (token.value === "and" || token.value === "or")) {
      // "instant and sorcery cards": a coordinated prenominal list; the
      // number is that of the head noun after it. Without a following
      // nominal the coordination belongs to the outer sentence.
      const next = c.tokens[pos + 1];
      if (next?.kind !== "word" || next.possessive || !classifyNominal(next.value, next.raw)) break;
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
    // "all revealed cards": every card of that event result, i.e. the same
    // set as the plural definite form. "All exiled cards" is not accepted
    // here: it describes cards currently in exile, whatever exiled them.
    const all = !definite && c.attempt(c => c.word("all") !== null && c.isWord("revealed") ? true : null) === true;
    // "those exiled cards" / "that exiled card": the demonstrative names the
    // same event result as the definite article; its number must agree.
    const demonstrative = definite || all ? null : c.attempt(c => {
      const word = c.word("those", "that");
      return word && c.isWord("exiled", "revealed") ? word : null;
    });
    const event = definite || all || demonstrative ? c.word("exiled", "sacrificed", "discarded", "revealed", "destroyed") as keyof typeof DEFINITE_RESULT_EVENTS | null
      ?? (definite ? c.word("milled") as "milled" | null : null) : null;
    const start = c.pos;
    const parsed = parseFilter(c);
    if (!parsed) return null;
    const number = relatedNumber(c, start, c.pos);
    if (!number || (all && number !== "plural")) return null;
    if (demonstrative && (demonstrative === "those") !== (number === "plural")) return null;
    if (event) {
      // A second explicit relation would need its own separately bound scope.
      if (parsed.relation) return null;
      // CR 702.167c: "the exiled cards used to craft it" are the craft materials.
      const craft = event === "exiled" ? c.attempt(c => c.words("used", "to", "craft") ? parseObjectRef(c) : null) : null;
      if (craft) return { ref: "related", number, filter: parsed, relation: { relation: "craft-material", of: craft } };
      return { ref: "related", number, filter: parsed, relation: { relation: "event-result", event: DEFINITE_RESULT_EVENTS[event], scope: "contextual" } };
    }
    if (!parsed.relation) return null;
    const { relation, ...filter } = parsed;
    return { ref: "related", number, filter, relation };
  });
}

// ---------------------------------------------------------------------------
// Durations
// ---------------------------------------------------------------------------

/** "during its controller's next turn" / "during your next combat phase":
 * the rule applies only while that player's next turn or combat phase is
 * taking place. */
export function parseNextPeriodOf(c: Cursor): Duration | null {
  return c.attempt((c): Duration | null => {
    if (c.word("during") === null) return null;
    const who = parsePossessivePlayer(c);
    if (!who || c.word("next") === null) return null;
    if (c.word("turn") !== null) return { duration: "during-next-turn-of", who };
    if (c.word("combat") === null) return null;
    c.word("phase");
    return { duration: "during-next-combat-of", who };
  });
}

export function parseDuration(c: Cursor): Duration | null {
  return c.attempt((c): Duration | null => {
    const nextOf = parseNextPeriodOf(c);
    if (nextOf) return nextOf;
    // CR 611.2a and 511.2 distinguish beginning an end step, ending combat,
    // and cleanup of the next turn; these boundaries must not be collapsed.
    // https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
    if (c.words("until", "the", "end", "of", "your", "next", "turn") || c.words("until", "end", "of", "your", "next", "turn"))
      return { duration: "end-of-your-next-turn" };
    if (c.words("until", "your", "next", "end", "step") || c.words("until", "the", "beginning", "of", "your", "next", "end", "step"))
      return { duration: "your-next-end-step" };
    if (c.words("until", "your", "next", "upkeep") || c.words("until", "the", "beginning", "of", "your", "next", "upkeep")) return { duration: "your-next-upkeep" };
    const untilEvent = c.attempt((c): Duration | null => {
      if (c.word("until") === null) return null;
      if (c.words("you", "exile", "another", "card", "with")) {
        const source = parseObjectRef(c);
        return source && (source.ref === "self" || source.ref === "this")
          ? { duration: "until-next-linked-exile", who: { player: "you" }, source } : null;
      }
      const what = parseObjectRef(c);
      if (!what || (what.ref !== "self" && what.ref !== "this") || !c.words("is", "cast", "from")) return null;
      const from = parseZoneRef(c);
      return from && { duration: "until-cast", what, from };
    });
    if (untilEvent) return untilEvent;
    if (c.words("until", "end", "of", "combat")) return { duration: "end-of-combat" };
    if (c.words("until", "end", "of", "turn")) return { duration: "end-of-turn" };
    if (c.words("until", "your", "next", "turn")) return { duration: "your-next-turn" };
    if (c.words("this", "turn")) return { duration: "this-turn" };
    if (c.words("this", "combat")) return { duration: "this-combat" };
    if (c.words("for", "the", "rest", "of", "the", "game")) return { duration: "rest-of-game" };
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

/** "a blue permanent": one unqualified indefinite object, nothing more. A
 * disjunction ("a Plains or an Island and a Swamp") has no unambiguous grouping. */
function isSingleIndefinite(ref: ObjectRef): boolean {
  return ref.ref === "filter" && ref.count.amount === "fixed" && ref.count.value === 1 && Object.keys(ref).length === 3
    && !JSON.stringify(ref.filter).includes('"anyOf"');
}

export function parseCondition(c: Cursor, options?: { purpose?: "unless"; outcomes?: true }): Condition | null {
  return parseConditionExpression(c, {
    ...options,
    parseAmount, parseComparison, parseCount, parseCounterSpec, parseFilter, parseObjectRef,
    parsePlayerRef, parsePossessivePlayer, parsePossessiveObject, parseZoneRef, parseAtomic: parseAtomicCondition,
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
      // "~'s power is odd": the object's characteristic has that parity, the
      // same test as "with odd power" (zero is even).
      const parity = value.amount === "attribute" && (value.attribute === "power" || value.attribute === "mana-value")
        ? c.word("odd", "even") as "odd" | "even" | null : null;
      if (parity && value.amount === "attribute")
        return { condition: "is-filter", what: value.of, filter: { parity: { property: value.attribute as "power" | "mana-value", parity } } };
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
    // "your library has no cards in it" is "there are no cards in your
    // library", as in the replacement-condition grammar.
    const emptyLibrary = c.attempt((c): Condition | null => {
      const zone = parseZoneRef(c);
      return zone?.zone === "library" && !zone.position && c.words("has", "no", "cards", "in", "it")
        ? { condition: "count", of: { cls: "card", zone }, comparison: { op: "eq", value: { amount: "fixed", value: 0 } } } : null;
    });
    if (emptyLibrary) return emptyLibrary;
    const counted = c.attempt((c): Condition | null => {
      if (!c.words("there", "are") && !c.words("there", "is")) return null;
      const comparison = parseComparison(c);
      if (!comparison) return null;
      const of = parseFilter(c);
      return of ? { condition: "count", of, comparison } : null;
    });
    if (counted) return counted;
    if (c.words("you", "do", "so") || c.words("you", "do")) return { condition: "you-do" };
    const notCast = c.attempt((c): Condition | null => {
      if (!c.words("you", "don't", "cast")) return null;
      const what = parseObjectRef(c);
      if (!what) return null;
      // "this way" restates the scope the node already fixes.
      c.words("this", "way");
      return { condition: "not-cast", who: { player: "you" }, what, scope: "preceding-instruction" };
    });
    if (notCast) return notCast;
    if (c.words("you", "don't")) return { condition: "you-dont" };
    if (c.words("it's", "your", "turn")) return { condition: "your-turn" };
    if (c.words("it's", "not", "your", "turn")) return { condition: "not-your-turn" };
    if (c.words("it", "is", "your", "turn")) return { condition: "your-turn" };

    const control = c.attempt((c): Condition | null => {
      let who: PlayerRef | null = null;
      if (c.word("you") !== null) who = { player: "you" };
      else if (c.words("an", "opponent")) who = { player: "an-opponent" };
      // "As long as any player controls a black permanent": some player does.
      else if (c.words("any", "player")) who = { player: "a-player" };
      else if (c.words("that", "player")) who = { player: "that-player" };
      else if (c.words("defending", "player")) who = { player: "defending-player" };
      else if (c.words("your", "team")) who = { player: "your-team" };
      // "no opponent controls a white creature": no opponent controls one.
      const none = !who && c.words("no", "opponent");
      if (none) who = { player: "an-opponent" };
      if (!who) return null;
      if (c.word("control", "controls") === null) return null;
      if (none) {
        if (!c.isWord("a", "an")) return null;
        const what = parseObjectRef(c);
        return what?.ref === "filter" ? { condition: "not", of: { condition: "control", who, what } } : null;
      }
      const counted = c.attempt((c): Condition | null => {
        const comparison = parseComparison(c);
        if (!comparison || comparison.op === "eq") return null;
        const of = parseFilter(c);
        return of ? { condition: "control-count", who, of, comparison } : null;
      });
      if (counted) return counted;
      // "the creature with the greatest power or tied for the greatest
      // power": the tie clause makes the definite article claim no
      // uniqueness, so controlling any creature with that power suffices.
      const tied = c.attempt((c): ObjectRef | null => {
        if (c.word("the") === null) return null;
        const start = c.pos;
        const filter = parseFilter(c);
        const words = c.tokens.slice(start, c.pos);
        if (!filter || Object.keys(filter).join() !== "types,properties" || filter.properties?.length !== 1
          || !words.some(token => token.kind === "word" && token.value === "tied")) return null;
        return { ref: "filter", filter, count: { amount: "fixed", value: 1 } };
      });
      const what = tied ?? parseObjectRef(c);
      if (!what) return null;
      // "you control a blue permanent and a black permanent": each
      // existence is tested on its own, so one blue-black permanent satisfies both.
      const also = !tied && isSingleIndefinite(what) ? c.attempt(c => {
        if (c.word("and") === null || !c.isWord("a", "an")) return null;
        const second = parseObjectRef(c);
        return second && isSingleIndefinite(second) ? second : null;
      }) : null;
      if (also) return { condition: "all-of", conditions: [{ condition: "control", who, what }, { condition: "control", who: structuredClone(who), what: also }] };
      return { condition: "control", who, what };
    });
    if (control) return control;

    const controlled = c.attempt((c): Condition | null => {
      const who = parsePlayerRef(c);
      if (!who || c.word("controlled") === null) return null;
      // Only an already identified object has an earlier existence to ask
      // about; "controlled a creature" would be a history of some period.
      const what = parseObjectRef(c);
      if (!what) return null;
      // Faerie Fencing: "if you controlled a Faerie as you cast this spell"
      // asks about control at that moment (CR 601.2), not on resolution.
      if (c.words("as", "you", "cast", "this", "spell")) return { condition: "control", who, what, asOf: "casting" };
      if (!(what.ref === "it" || (what.ref === "that" && !what.plural))) return null;
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
      const paid = parsePaidCosts(c);
      if (paid) return { condition: "pays", who, cost: paid };
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
      const chosenName = property ? null : parseChosenNameQualifier(c, parseFilter, "have");
      const filter = property ? { properties: [property] } : chosenName ? { chosenName } : parseCounterConstraint(c, true);
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
      if (!filter) return null;
      // A bare "they" does not say whether it denotes objects or a player.
      // Counter kinds only players can have (CR 122.1) settle it; any other
      // kind on a bare plural pronoun stays unsupported instead of guessing.
      if (what.ref === "them") {
        const constraint = filter.counterConstraints?.[0];
        const playerOnly = constraint && constraint.counter !== "any" && ["rad", "poison", "energy", "experience", "ticket"].includes(constraint.counter.type);
        if (!constraint || !playerOnly) return c.fail("subject of a counter test");
        return { condition: "not", of: { condition: "player-counters", who: { player: "they" }, counter: constraint.counter, comparison: constraint.comparison } };
      }
      return { condition: "not", of: { condition: "is-filter", what, filter } };
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

    const attached = parseAttachmentCondition(c, { parseComparison, parseFilter, parseObjectRef, parseRelation: parseObjectRelationPredicate });
    if (attached) return attached;

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
      // "A creature is attacking you": at least one matching creature attacks that
      // player or permanent (CR 508.1b); the status node carries no defender.
      if (!time && what.ref === "filter" && !what.upTo && what.count.amount === "fixed" && what.count.value === 1 && !what.groupConstraints) {
        const attacking = c.attempt(c => c.isWord("attacking") ? parseObjectRelationPredicate(c) : null);
        if (attacking) return wrap({ condition: "count", of: { ...what.filter, ...attacking }, comparison: { op: "ge", value: { amount: "fixed", value: 1 } } });
      }
      c.word("a", "an");
      const status = c.attempt((c) => {
        const w = c.anyWord();
        if (!w || !STATUS_WORDS.has(w.value)) return null;
        // CR 506.5: attacking/blocking alone is a status of its own.
        return (w.value === "attacking" || w.value === "blocking") && c.word("alone") !== null ? `${w.value}-alone` : w.value;
      });
      // "is attacking or blocking", "is white and untapped": further statuses
      // joined by one connector hold of the same object, either-or ("or") or
      // all-of ("and"). A negation distributes over "or" only ("isn't attacking
      // or blocking" is neither), and a past status is never chained.
      const moreStatuses = (first: Condition): Condition | null => {
        if (time) return first;
        const parts = [first];
        let connector: "and" | "or" | undefined;
        for (;;) {
          const next = c.attempt((c): { connector: "and" | "or"; status: string } | null => {
            const word = c.word("and", "or") as "and" | "or" | null;
            const w = word && c.anyWord();
            return word && w && STATUS_WORDS.has(w.value) && (!connector || connector === word) ? { connector: word, status: w.value } : null;
          });
          if (!next) break;
          connector = next.connector;
          parts.push({ condition: "status", what, status: next.status });
        }
        if (parts.length === 1) return first;
        if (negate && connector === "and") return c.fail("negated status conjunction");
        return { condition: connector === "or" ? "any-of" : "all-of", conditions: parts };
      };
      if (status) {
        const chained = moreStatuses({ condition: "status", what, status, ...(time ? { time } : {}) });
        return chained && wrap(chained);
      }
      // A predicate may be adjective-only: "isn't legendary", "it's red".
      const filter = parseColorSetPredicate(c) ?? parseFilter(c) ?? parseFilterExpression(c, true);
      if (!filter) return null;
      const chained = moreStatuses({ condition: "is-filter", what, filter, ...(time ? { time } : {}) });
      return chained && wrap(chained);
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
      // A counter kind is a name, never a function word: "two of its
      // counters" and "those counters" refer to counters, they do not name one.
      if (["and", "or", "a", "an", "any", "no", "different", "more", "fewer", "of", "its", "their", "his", "her", "your",
        "the", "this", "that", "these", "those", "each", "all", "other", "another", "on", "from", "with", "to", "target"].includes(w.value)) return null;
      words.push(w.value);
    }
    if (words.length === 0 || !c.isWord("counter", "counters")) return c.fail("counter type");
    return { type: words.join(" ") };
  });
}
