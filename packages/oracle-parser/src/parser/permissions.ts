// Casting permissions and instructions are different rules objects. A static
// permission applies while the ability functions; a resolving instruction
// without a duration casts now (CR 608.2g); a stated duration creates a
// permission used later under the normal timing rules (CR 611.2a).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt

import type {
  CardPlayInstruction, Condition, Cost, Duration, Effect, ObjectRef,
  PlayerRef, StaticEffect, ZoneRef,
} from "../ast.ts";
import type { Token } from "../lexer.ts";
import { CARD_TYPES, canonicalSubtypeName, singularize } from "../vocab.ts";
import { Cursor } from "./cursor.ts";
import { parseLandPlayEffect } from "./casting-rules.ts";
import { parsePermissionRider } from "./permission-riders.ts";
import { parseCostList } from "./costs.ts";
import { parseKeywordLine } from "./keywords.ts";
import {
  parseAmount, parseCondition, parseDuration, parseObjectRef, parsePlayerRef, parseZoneRef,
} from "./refs.ts";

export interface PlayInstructionOptions {
  /** parseSteps may already have consumed the enclosing "you may". */
  optionalConsumed?: boolean;
  /** Must be supplied before choosing immediate action vs duration grant. */
  leadingDuration?: Duration;
}

interface ParsedPlay {
  instruction: CardPlayInstruction;
  optional: boolean;
  duration?: Duration;
  durationScope?: "each-permitted-card";
}

function joinConditions(a: Condition | undefined, b: Condition): Condition {
  return a ? { condition: "all-of", conditions: [a, b] } : b;
}

function sentenceBoundary(c: Cursor): boolean {
  const next = c.peek(1);
  if (c.isWord("and") && next?.kind === "punct" && next.value === "/") return false;
  return c.done() || c.isPunct(".") || c.isPunct(",") || c.isWord("and", "then", "if", "unless");
}

function parseManaCostPhrase(c: Cursor): boolean {
  return c.attempt((c) => {
    if (c.word("its", "their") === null || !c.words("mana")) return null;
    return c.word("cost", "costs") !== null ? true : null;
  }) === true;
}

/** Casting uses the source zone, whether Oracle puts "from" after a self
 * reference or inside a target/filter. Do not retain a spell-in-graveyard
 * predicate in place of the zone from which a card will be cast. */
function separateSource(what: ObjectRef): { what: ObjectRef; from?: ZoneRef; fromAmong?: ObjectRef } {
  if (!("filter" in what) || (!what.filter.zone && !what.filter.fromAmong)) return { what };
  const { zone: from, fromAmong, ...filter } = what.filter;
  return { what: { ...what, filter }, ...(from ? { from } : {}), ...(fromAmong ? { fromAmong } : {}) };
}

/** Plural eligible classes enumerate alternatives: Melek's "instant and
 * sorcery spells" permits either kind. Adjacent "artifact creature" still
 * requires both types, and "green and blue" still requires both colors.
 * This localized grammar does not alter general object-filter conjunctions.
 * Card wording: https://api.scryfall.com/cards/named?exact=Melek%2C%20Izzet%20Paragon */
export function parseEligibleCardRef(c: Cursor, options: { distributive?: boolean } = {}): ObjectRef | null {
  return c.attempt((c): ObjectRef | null => {
    const start = c.pos;
    let tokens = c.tokens.slice(start);
    // An explicit "for each" also enumerates nominal alternatives with a
    // singular head: Bedlam Reveler's "each instant and sorcery card" counts
    // either type once. Ordinary singular selections keep their normal grammar.
    const heads = options.distributive ? ["spell", "card", "spells", "cards"] : ["spells", "cards"];
    const headEnd = tokens.findIndex(token => token.kind === "word" && heads.includes(token.value));
    if (headEnd > 0) {
      const nominal = (token: Token | undefined): boolean => token?.kind === "word" && !token.possessive &&
        (CARD_TYPES.has(singularize(token.value)) || canonicalSubtypeName(token.raw) !== undefined);
      tokens = tokens.map((token, index): Token => {
        if (index >= headEnd || token.kind !== "word" || token.value !== "and") return token;
        const before = tokens[index - 1]?.kind === "punct" ? tokens[index - 2] : tokens[index - 1];
        if (!nominal(before) || !nominal(tokens[index + 1])) return token;
        return { ...token, value: "or", raw: "or" };
      });
    }
    const local = new Cursor(tokens);
    const first = parseObjectRef(local);
    if (!first) return null;
    c.pos = start + local.pos;
    // A multiplier owns any repeated "and each" count groups. Do not fold
    // those groups into the normal plural casting-eligibility union.
    if (first.ref !== "each" || options.distributive) return first;
    const branches = [first.filter];
    while (c.attempt((c) => {
      if (c.word("and") === null) return null;
      const next = parseObjectRef(c);
      // Repeated plural spell/card nouns form an eligibility union. A new
      // action, singular selection, or unrelated conjunction stays outside.
      if (next?.ref !== "each" || !["spell", "card"].includes(next.filter.cls ?? "") ||
          !["spell", "card"].includes(first.filter.cls ?? "")) return null;
      branches.push(next.filter);
      return true;
    })) { /* preserve all complete eligible alternatives */ }
    if (branches.length === 1) return first;
    const from = branches[branches.length - 1].zone;
    const fromAmong = branches[branches.length - 1].fromAmong;
    if (branches.slice(0, -1).some(filter => filter.zone && JSON.stringify(filter.zone) !== JSON.stringify(from))) return null;
    if (branches.slice(0, -1).some(filter => filter.fromAmong && JSON.stringify(filter.fromAmong) !== JSON.stringify(fromAmong))) return null;
    const filters = branches.map(({ zone: _zone, fromAmong: _among, ...filter }) => filter);
    return { ref: "each", filter: { anyOf: filters, ...(from ? { zone: from } : {}), ...(fromAmong ? { fromAmong } : {}) } };
  });
}

const COST_VERBS: Record<string, string> = {
  paying: "pay", sacrificing: "sacrifice", discarding: "discard", exiling: "exile",
  tapping: "tap", removing: "remove", revealing: "reveal", returning: "return",
};

/** "by paying … and sacrificing …" has the same typed costs as an
 * activation cost. Only action-position gerunds are normalized, in an
 * isolated token span whose entire contents must parse. */
function parseGerundCosts(tokens: Token[]): Cost[] | null {
  const normalized = tokens.map((token, index): Token => {
    const previous = tokens[index - 1];
    const actionPosition = index === 0 ||
      (previous?.kind === "word" && previous.value === "and") ||
      (previous?.kind === "punct" && previous.value === ",");
    if (actionPosition && token.kind === "word" && !token.possessive && COST_VERBS[token.value]) {
      return { ...token, value: COST_VERBS[token.value], raw: COST_VERBS[token.value] };
    }
    return token;
  });
  const costsCursor = new Cursor(normalized);
  // Access Maze: payment is variable life, not an alternative mana value.
  const life = costsCursor.attempt((c): Cost[] | null => {
    if (!c.words("pay", "life", "equal", "to")) return null;
    const amount = parseAmount(c);
    return amount && c.done() ? [{ cost: "pay-life", amount }] : null;
  });
  if (life) return life;
  const costs = parseCostList(costsCursor);
  return costs && costsCursor.done() ? costs : null;
}

function parseByCosts(c: Cursor): Pick<CardPlayInstruction, "additionalCosts" | "alternativeCost"> | null {
  return c.attempt((c) => {
    if (c.word("by") === null) return null;
    const start = c.pos;
    for (let end = start + 1; end < c.tokens.length; end++) {
      const marker = c.tokens[end];
      if (marker.kind !== "word" || !["rather", "in"].includes(marker.value)) continue;
      const tail = new Cursor(c.tokens.slice(end));
      let kind: "alternative" | "additional";
      if (tail.words("rather", "than")) {
        tail.word("paying", "pay");
        if (!parseManaCostPhrase(tail)) continue;
        kind = "alternative";
      } else if (tail.words("in", "addition", "to")) {
        tail.word("paying");
        if (tail.word("its", "their") === null || !tail.words("other", "costs")) continue;
        kind = "additional";
      } else continue;
      const costs = parseGerundCosts(c.tokens.slice(start, end));
      if (!costs) continue;
      c.pos = end + tail.pos;
      return kind === "alternative" ? { alternativeCost: { cost: "pay" as const, costs } } : { additionalCosts: costs };
    }
    return c.fail("complete alternative or additional casting cost");
  });
}

function parseExtraManaForTiming(c: Cursor): Cost[] | null {
  return c.attempt((c) => {
    if (!c.words("if", "you", "pay")) return null;
    const start = c.pos;
    while (c.peek()?.kind === "symbol") c.pos++;
    const costs = parseGerundCosts(c.tokens.slice(start, c.pos));
    if (!costs || costs.some(cost => cost.cost !== "mana") ||
        !c.words("more", "to", "cast", "it")) return null;
    return costs;
  });
}

/** The rider is tied to this casting permission, not a new mana-producing
 * action. CR 609.4b: it changes acceptable payment, not the mana spent. */
function parseManaPayment(c: Cursor, who: PlayerRef): CardPlayInstruction["manaPayment"] | null {
  return c.attempt((c) => {
    c.punct(",");
    if (c.word("and") === null) return null;
    let mode: "any-color" | "any-type";
    if (c.words("mana", "of", "any", "type", "can", "be", "spent")) mode = "any-type";
    else {
      const actor = parsePlayerRef(c);
      if (!actor || JSON.stringify(actor) !== JSON.stringify(who) ||
          !c.words("may", "spend", "mana", "as", "though", "it", "were", "mana", "of", "any")) return null;
      const word = c.word("color", "type");
      if (!word) return null;
      mode = word === "color" ? "any-color" : "any-type";
    }
    if (!c.words("to", "cast")) return null;
    if (c.word("it", "them") === null && !c.words("that", "spell") && !c.words("those", "spells")) return null;
    return mode;
  });
}

function parsePlay(c: Cursor, options: PlayInstructionOptions): ParsedPlay | null {
  return c.attempt((c): ParsedPlay | null => {
    const leadingDuration = options.leadingDuration ?? c.attempt((c) => {
      const duration = parseDuration(c);
      return duration && c.punct(",") ? duration : null;
    });
    let optional = options.optionalConsumed === true;
    let who: PlayerRef = { player: "you" };
    const explicitPlayer = c.attempt((c) => {
      const player = parsePlayerRef(c);
      if (!player || c.word("may") === null) return null;
      return player;
    });
    if (explicitPlayer) { who = explicitPlayer; optional = true; }
    const action = c.word("cast", "play");
    if (!action) return null;
    const referenceHead = c.peek();
    const ref = parseEligibleCardRef(c);
    if (!ref) return null;
    const { what, from, fromAmong } = separateSource(ref);
    const instruction: CardPlayInstruction = { who, action: action as "cast" | "play", what,
      ...(from ? { from } : {}), ...(fromAmong ? { fromAmong } : {}) };
    let duration = leadingDuration ?? undefined;
    for (;;) {
      if (!instruction.from && !instruction.fromAmong && c.word("from") !== null) {
        if (c.word("among") !== null) {
          const pool = parseObjectRef(c);
          if (!pool) return null;
          instruction.fromAmong = pool;
        } else {
          const source = parseZoneRef(c);
          if (!source) return null;
          instruction.from = source;
        }
        continue;
      }
      if (instruction.from) {
        const sourceAlternative = c.attempt((c) => {
          if (c.word("or") === null) return null;
          c.word("from");
          return parseZoneRef(c);
        });
        if (sourceAlternative) {
          instruction.from = { anyOf: [...("anyOf" in instruction.from ? instruction.from.anyOf : [instruction.from]), sourceAlternative] };
          continue;
        }
      }
      if (!instruction.alternativeCost && c.words("without", "paying")) {
        if (!parseManaCostPhrase(c)) return null;
        instruction.alternativeCost = { cost: "without-mana-cost" };
        continue;
      }
      if (c.isWord("by")) {
        const payment = parseByCosts(c);
        if (!payment || (payment.additionalCosts && instruction.additionalCosts) ||
            (payment.alternativeCost && instruction.alternativeCost)) return null;
        Object.assign(instruction, payment);
        continue;
      }
      if (!instruction.timing) {
        const timing = c.attempt((c) => {
          c.word("and");
          if (!c.words("as", "though") || c.word("it", "they") === null || !c.words("had", "flash")) return null;
          return "as-though-flash" as const;
        });
        if (timing) { instruction.timing = timing; continue; }
      }
      if (instruction.timing && !instruction.additionalCosts) {
        const costs = parseExtraManaForTiming(c);
        if (costs) { instruction.additionalCosts = costs; continue; }
      }
      if (!duration) {
        const trailingDuration = parseDuration(c);
        if (trailingDuration) { duration = trailingDuration; continue; }
      }
      if (c.word("if") !== null) {
        const condition = parseCondition(c);
        if (!condition) return null;
        instruction.condition = joinConditions(instruction.condition, condition);
        continue;
      }
      if (!instruction.manaPayment) {
        const mana = parseManaPayment(c, who);
        if (mana) { instruction.manaPayment = mana; continue; }
      }
      break;
    }
    if (!sentenceBoundary(c)) return c.fail("complete cast/play instruction");
    // A generic counted permission chooses the card when used, not while
    // this effect resolves, and can be used only the stated number of times.
    // Chainer's official rulings explicitly distinguish these independent
    // permissions from the activation's once-per-turn limit.
    // https://magic.wizards.com/en/news/feature/commander-2019-edition-release-notes-2019-08-09
    if (duration && instruction.what.ref === "filter") {
      instruction.useLimit = { maximum: instruction.what.count, period: "permission" };
    }
    let durationScope: ParsedPlay["durationScope"];
    if (duration?.duration === "as-long-as" && duration.condition.condition === "remains" && duration.condition.zone.zone === "exile") {
      const number = what.ref === "related" ? what.number : what.ref === "them" ? "plural"
        : what.ref === "that" ? (referenceHead?.kind === "word" && referenceHead.value === "those" ? "plural" : "singular")
        : ["self", "this", "it"].includes(what.ref) ? "singular" : undefined;
      const bound = duration.condition.what;
      const matches = number && (JSON.stringify(bound) === JSON.stringify(what) ||
        (bound.ref === "it" && number === "singular") || (bound.ref === "them" && number === "plural"));
      // Brainstealer Dragon / Valakut Exploration: each card's permission
      // ends when that card leaves exile, independently of the other cards.
      // Do not let a plural pronoun become an all-cards-still-exiled guard.
      if (matches) durationScope = "each-permitted-card";
      else if (bound.ref === "it" || bound.ref === "them") return c.fail("unambiguous permitted-card lifetime");
    }
    return { instruction, optional, ...(duration ? { duration } : {}), ...(durationScope ? { durationScope } : {}) };
  });
}

function parseCoordinatedPlays(c: Cursor, options: PlayInstructionOptions): ParsedPlay[] | null {
  return c.attempt((c) => {
    const leadingDuration = options.leadingDuration ?? c.attempt((c) => {
      const duration = parseDuration(c);
      return duration && c.punct(",") ? duration : null;
    }) ?? undefined;
    const sharedOptions = { ...options, leadingDuration };
    const first = parsePlay(c, sharedOptions);
    if (!first) return null;
    const results = [first];
    while (c.attempt((c) => {
      if (c.word("and") === null || !c.isWord("cast", "play")) return null;
      const next = parsePlay(c, { optionalConsumed: first.optional, leadingDuration });
      if (!next) return null;
      results.push(next);
      return true;
    })) { /* each explicit cast/play verb keeps its own typed action */ }
    const last = results[results.length - 1];
    for (const result of results.slice(0, -1)) {
      // Future Sight: the shared final source qualifies both playing lands
      // and casting spells. A mana/timing alteration concerns spells only.
      if (!result.instruction.from && last.instruction.from) result.instruction.from = last.instruction.from;
      if (!result.instruction.fromAmong && last.instruction.fromAmong) result.instruction.fromAmong = last.instruction.fromAmong;
      if (!result.duration && last.duration) result.duration = last.duration;
      if (result.duration && result.instruction.what.ref === "filter") {
        result.instruction.useLimit = { maximum: result.instruction.what.count, period: "permission" };
      }
      if (last.instruction.condition) result.instruction.condition = joinConditions(result.instruction.condition, last.instruction.condition);
      if (result.instruction.action === "cast") {
        if (!result.instruction.alternativeCost && last.instruction.alternativeCost) result.instruction.alternativeCost = last.instruction.alternativeCost;
        if (!result.instruction.additionalCosts && last.instruction.additionalCosts) result.instruction.additionalCosts = last.instruction.additionalCosts;
        if (!result.instruction.timing && last.instruction.timing) result.instruction.timing = last.instruction.timing;
        if (!result.instruction.manaPayment && last.instruction.manaPayment) result.instruction.manaPayment = last.instruction.manaPayment;
      }
    }
    return results;
  });
}

/** Use before generic optional/duration decoration, or provide those consumed
 * values explicitly. A duration grant is mandatory creation of a permission;
 * "may" describes the later choice and must not become optional:true here. */
export function parsePlayInstruction(c: Cursor, options: PlayInstructionOptions = {}): Effect[] | null {
  return c.attempt((c) => {
    const landPlays = parseLandPlayEffect(c, options);
    if (landPlays) return landPlays;
    const coordinated = c.attempt((c): Effect[] | null => {
      const results = parseCoordinatedPlays(c, options);
      if (!results || results.length < 2 || results.some(result => !result.optional || !result.duration)) return null;
      return results.map(result => ({ effect: "grant-play-permission", permission: result.instruction, duration: result.duration!,
        ...(result.durationScope ? { durationScope: result.durationScope } : {}) }));
    });
    if (coordinated) return coordinated;
    const result = parsePlay(c, options);
    if (!result) return null;
    if (result.duration) {
      if (!result.optional) return c.fail("may for a cast/play permission");
      return [{ effect: "grant-play-permission", permission: result.instruction, duration: result.duration,
        ...(result.durationScope ? { durationScope: result.durationScope } : {}) }];
    }
    return [{ effect: "play-card", instruction: result.instruction, ...(result.optional ? { optional: true as const } : {}) }];
  });
}

/** Chiss-Goria: the later sentence modifies the spell cast under the granted
 * permission, before its costs are determined, not the earlier resolving
 * ability. CR 608.2c, 601.2f, 702.41, 400.7a. Quoted captures, different
 * actors, plural ambiguous references and further sentences are unsupported.
 * The caller must establish one immediately preceding duration permission.
 * https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt */
export function parsePermissionSpellAbilities(c: Cursor, permission: CardPlayInstruction): CardPlayInstruction | null {
  return c.attempt(c => {
    if (permission.action !== "cast" || permission.who.player !== "you" || permission.spellAbilities) return null;
    const what = permission.what;
    const singular = what.ref === "self" || what.ref === "this" || what.ref === "it"
      || (what.ref === "that" && !what.plural)
      || (what.ref === "related" && what.number === "singular")
      || ((what.ref === "filter" || what.ref === "target" || what.ref === "from-pool")
        && what.count.amount === "fixed" && what.count.value === 1);
    if (!singular || !c.words("if", "you", "do") || !c.punct(",") || !c.words("it", "has")) return null;
    // parseKeywordLine requires the complete remaining input. Do not assign
    // unknown following instructions to either resolution or later casting.
    const spellAbilities = parseKeywordLine(c);
    return spellAbilities && { ...permission, spellAbilities };
  });
}

/** Whole-line static recognition is intentionally conservative. A target or
 * demonstrative is generally a resolution instruction (Memory Plunder,
 * Dazzling Sphinx), whereas self-casting and unrestricted plural spell/land
 * sets describe a static permission (Gravecrawler, Omniscience, Yeva). */
export function parsePlayPermissionStatic(c: Cursor): StaticEffect | null {
  return parseStaticPermission(c, true);
}

/** Classification guard only: an unrepresented rider must not cause a static
 * permission to fall through to an immediate spell instruction. This does
 * not consume tokens and must only run in top-level line classification. */
export function looksLikeStaticPlayPermission(c: Cursor): boolean {
  const start = c.pos;
  const result = parseStaticPermission(c, false);
  c.pos = start;
  return result !== null;
}

function parseStaticPermission(c: Cursor, complete: boolean): StaticEffect | null {
  return c.attempt((c): StaticEffect | null => {
    let condition: Condition | undefined;
    let useLimit: CardPlayInstruction["useLimit"];
    if (c.words("once", "during", "each", "of", "your", "turns")) {
      if (!c.punct(",")) return null;
      useLimit = { maximum: 1, period: "turn", during: "your" };
    } else if (c.words("once", "each", "turn")) {
      if (!c.punct(",")) return null;
      useLimit = { maximum: 1, period: "turn", during: "any" };
    } else if (c.words("during", "your", "turn") || c.words("during", "each", "of", "your", "turns")) {
      if (!c.punct(",")) return null;
      condition = { condition: "your-turn" };
    }
    if (c.word("if") !== null) {
      const value = parseCondition(c);
      if (!value || !c.punct(",")) return null;
      condition = joinConditions(condition, value);
    }
    const results = parseCoordinatedPlays(c, {});
    if (!results || results.some(result => !result.optional)) return null;
    // A once-per-turn bound on a coordinated choice must be shared, not
    // incorrectly multiplied into one use for each action (Hauken's Insight).
    if (results.length > 1 && useLimit) return null;
    const effects: Extract<StaticEffect, { static: "play-permission" }>[] = [];
    for (const result of results) {
      const permission = result.instruction;
      const ownCard = permission.what.ref === "self" || permission.what.ref === "this";
      const wholeSet = permission.what.ref === "each";
      const linkedSet = permission.what.ref === "related" && permission.what.relation.relation === "linked-exile";
      if (!ownCard && !wholeSet && !linkedSet && !useLimit) return null;
      if (!permission.from && !permission.fromAmong && !linkedSet && !permission.alternativeCost && !permission.timing) return null;
      let guard = condition;
      if (result.duration) {
        if (result.duration.duration !== "as-long-as") return null;
        guard = joinConditions(guard, result.duration.condition);
      }
      if (guard) permission.condition = joinConditions(permission.condition, guard);
      if (useLimit) permission.useLimit = useLimit;
      effects.push({ static: "play-permission", permission });
    }
    c.punct(".");
    for (;;) {
      const permissions = parsePermissionRider(c, effects.map(effect => effect.permission), { allowIfYouDo: false });
      if (!permissions) break;
      effects.forEach((effect, index) => { effect.permission = permissions[index]; });
    }
    if (complete && !c.done()) return c.fail("complete static cast/play permission");
    return effects.length === 1 ? effects[0] : { static: "group", effects };
  });
}
