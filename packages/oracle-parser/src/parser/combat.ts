// Detailed combat restrictions, requirements, and blocking capacities.
// CR 508.1c–d and 509.1a–c distinguish these three concepts; notably an
// evasion restriction never requires the defending player to block.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, CantAction, CombatDefender, CombatRule, Comparison, DamageTarget, Duration, Effect, ObjectFilter, ObjectRef, StaticEffect } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseComparison, parseCondition, parseCount, parseDuration, parseObjectRef, parsePlayerRef } from "./refs.ts";

function parseDefender(c: Cursor): CombatDefender | null {
  return c.attempt(c => {
    if (c.words("a", "player", "other", "than")) {
      const who = parsePlayerRef(c);
      return who && { defender: "player-other-than", who };
    }
    const who = parsePlayerRef(c);
    if (who) return { defender: "player", who };
    const what = parseObjectRef(c);
    return what && { defender: "object", what };
  });
}

function parseBlockCount(c: Cursor): Comparison | null {
  return c.attempt(c => {
    // An indefinite singular means a class of eligible blockers, not exactly
    // one blocker; only explicit numeric comparisons constrain their number.
    if (c.isWord("a", "an")) return null;
    let comparison: Comparison | null;
    if (c.words("more", "than")) {
      const value = parseCount(c);
      comparison = value && { op: "gt", value };
    } else if (c.words("fewer", "than")) {
      const value = parseCount(c);
      comparison = value && { op: "lt", value };
    } else comparison = parseComparison(c);
    if (!comparison || c.word("creature", "creatures") === null) return null;
    return comparison;
  });
}

function parseCombatObjects(c: Cursor): ObjectRef | null {
  return c.attempt(c => {
    const first = parseGenericCombatRef(c);
    if (!first) return null;
    const alternatives = [first];
    while (c.attempt(c => {
      const connector = c.word("or") !== null || (c.word("and") !== null && c.punct("/") && c.word("or") !== null);
      if (!connector) return null;
      const next = parseGenericCombatRef(c);
      if (!next || first.ref !== "each" || next.ref !== "each") return null;
      alternatives.push(next);
      return true;
    })) { /* independently specified blocker categories form an eligible set */ }
    if (alternatives.length === 1) return first;
    const filters: ObjectFilter[] = [];
    for (const alternative of alternatives) {
      if (alternative.ref !== "each") return null;
      filters.push(alternative.filter);
    }
    return { ref: "each", filter: { anyOf: filters } };
  });
}

/** Generic subjects and eligible blocker/attacker classes apply to each
 * matching creature (CR 508.1c / 509.1b). This is deliberately not used for
 * attack-requirement defender choices or numeric blocking capacities. */
function parseGenericCombatRef(c: Cursor): ObjectRef | null {
  const generic = c.isWord("a", "an");
  const what = parseObjectRef(c);
  return generic && what?.ref === "filter" && what.count?.amount === "fixed" && what.count.value === 1
    ? { ref: "each", filter: what.filter } : what;
}

function parseBlockers(c: Cursor, mode: "exclude" | "only"): CombatRule | null {
  return c.attempt(c => {
    const comparison = parseBlockCount(c);
    if (comparison) return { rule: "blocker-count", mode, comparison };
    const blockers = parseCombatObjects(c);
    return blockers && { rule: "blocker-filter", mode, blockers };
  });
}

function parseCapacity(c: Cursor): CombatRule | null {
  return c.attempt(c => {
    if (c.words("any", "number", "of", "creatures")) {
      c.words("each", "combat");
      return { rule: "block-capacity", capacity: { maximum: "unlimited" } };
    }
    const additional = c.attempt((c): Amount | null => {
      // "an additional creature", "an additional seven creatures", and
      // "one additional creature" all add to the existing blocking capacity.
      c.word("a", "an");
      if (c.word("additional") !== null) return parseCount(c) ?? { amount: "fixed", value: 1 };
      const count = parseCount(c);
      return count && c.word("additional") !== null ? count : null;
    });
    if (additional) {
      if (c.word("creature", "creatures") === null) return null;
      c.words("each", "combat");
      return { rule: "block-capacity", capacity: { additional } };
    }
    if (c.words("up", "to") || c.word("only") !== null) {
      const maximum = parseCount(c);
      if (!maximum || c.word("creature", "creatures") === null) return null;
      c.words("each", "combat");
      return { rule: "block-capacity", capacity: { maximum } };
    }
    return null;
  });
}

/** A requirement's own period ("blocks this turn if able") precedes "if
 * able". It is the lifetime of the continuous effect that creates the
 * requirement (CR 611.2a), so resolving effects carry it as their duration. */
interface ParsedRule { rule: CombatRule; duration?: Duration }

function parseRequirementPeriod(c: Cursor): Duration | null {
  if (c.words("this", "turn")) return { duration: "this-turn" };
  if (c.words("this", "combat")) return { duration: "this-combat" };
  return null;
}

/** CR 508.1d / 509.1c: requirements are obeyed as far as restrictions allow;
 * they are neither restrictions nor one-shot instructions. A requirement for
 * a turn applies at each declare attackers/blockers step of that turn. */
function parseRequirement(c: Cursor): ParsedRule | null {
  return c.attempt((c): ParsedRule | null => {
    const must = c.word("must") !== null;
    if (must && c.words("be", "blocked")) {
      let blockers: ObjectRef | undefined;
      let count: Comparison | undefined;
      if (c.word("by") !== null) {
        const comparison = parseBlockCount(c);
        if (comparison) count = comparison;
        else {
          const objects = parseCombatObjects(c);
          if (!objects) return null;
          blockers = objects;
        }
      }
      // Redundant with CR 509.1c: every such requirement applies each combat.
      c.words("each", "combat");
      const duration = parseRequirementPeriod(c);
      if (!c.words("if", "able")) return null;
      return { rule: { rule: "blocked-requirement", ...(blockers ? { blockers } : {}), ...(count ? { count } : {}) },
        ...(duration ? { duration } : {}) };
    }
    const verb = c.word("attack", "attacks", "block", "blocks");
    if (!verb) return null;
    if (verb.startsWith("block")) {
      const attackers = c.attempt(parseObjectRef);
      c.words("each", "combat");
      const duration = parseRequirementPeriod(c);
      if (!c.words("if", "able")) return null;
      return { rule: { rule: "block-requirement", ...(attackers ? { attackers } : {}) }, ...(duration ? { duration } : {}) };
    }
    const orBlocks = c.attempt(c => c.word("or") !== null && c.word("block", "blocks") !== null ? true : null) === true;
    const defender = orBlocks ? null : parseDefender(c);
    let frequency: "each-combat" | "each-turn" = "each-combat";
    let explicit = true;
    if (c.words("each", "combat")) frequency = "each-combat";
    else if (c.words("each", "turn")) frequency = "each-turn";
    else explicit = false;
    const duration = parseRequirementPeriod(c);
    if (!explicit && !duration && !must && !c.isWord("if")) return null;
    if (!c.words("if", "able")) return null;
    const attack: CombatRule = { rule: "attack-requirement", frequency, ...(defender ? { defender } : {}) };
    return { rule: orBlocks ? { rule: "all-of", rules: [attack, { rule: "block-requirement" }] } : attack,
      ...(duration ? { duration } : {}) };
  });
}

/** CR 506.5: a creature attacks/blocks alone if it is the only creature
 * declared as an attacker/blocker in that declaration. */
function parseAloneRestriction(c: Cursor): CombatRule | null {
  return c.attempt((c): CombatRule | null => {
    if (c.word("can't") !== null) {
      if (c.words("attack", "or", "block", "alone")) return { rule: "all-of", rules: [
        { rule: "alone-restriction", action: "attack", mode: "exclude" },
        { rule: "alone-restriction", action: "block", mode: "exclude" },
      ] };
      const action = c.word("attack", "block") as "attack" | "block" | null;
      return action && c.word("alone") !== null ? { rule: "alone-restriction", action, mode: "exclude" } : null;
    }
    if (!c.words("can", "only")) return null;
    const action = c.word("attack", "block") as "attack" | "block" | null;
    return action && c.word("alone") !== null ? { rule: "alone-restriction", action, mode: "only" } : null;
  });
}

function parseRuleBase(c: Cursor): CombatRule | null {
  return c.attempt(c => {
    const alone = parseAloneRestriction(c);
    if (alone) return alone;
    if (c.word("can't") !== null) {
      if (c.words("block", "or", "be", "blocked", "by")) {
        const other = parseCombatObjects(c);
        return other && { rule: "all-of", rules: [
          { rule: "blocking-filter", mode: "exclude", attackers: other },
          { rule: "blocker-filter", mode: "exclude", blockers: other },
        ] };
      }
      if (c.words("be", "blocked")) {
        const mode = c.word("except") !== null ? "only" : "exclude";
        if (c.word("by") === null) return null;
        return parseBlockers(c, mode);
      }
      if (c.word("block") !== null) {
        const attackers = parseCombatObjects(c);
        return attackers && { rule: "blocking-filter", mode: "exclude", attackers };
      }
      if (c.word("attack") !== null) {
        const first = parseDefender(c);
        if (!first) return null;
        const defenders = [first];
        while (c.attempt(c => {
          if (c.word("or") === null) return null;
          const next = parseDefender(c);
          if (!next) return null;
          defenders.push(next);
          return true;
        })) { /* each forbidden defender is retained independently */ }
        if (c.words("or", "block")) {
          const attackers = parseCombatObjects(c);
          return attackers && { rule: "all-of", rules: [
            { rule: "attack-restriction", defenders }, { rule: "blocking-filter", mode: "exclude", attackers },
          ] };
        }
        return { rule: "attack-restriction", defenders };
      }
      return null;
    }
    if (c.word("can") !== null) {
      if (c.words("be", "blocked", "only", "by")) return parseBlockers(c, "only");
      if (c.word("block") === null) return null;
      const capacity = parseCapacity(c);
      if (capacity) return capacity;
      if (c.word("only") === null) return null;
      const attackers = parseCombatObjects(c);
      return attackers && { rule: "blocking-filter", mode: "only", attackers };
    }
    return null;
  });
}

function parseRule(c: Cursor): ParsedRule | null {
  return c.attempt((c): ParsedRule | null => {
    const base = parseRuleBase(c);
    const parsed = base ? { rule: base } : parseRequirement(c);
    if (!parsed) return null;
    const rule = parsed.rule;
    const conditionKind = c.word("if", "unless") as "if" | "unless" | null;
    if (conditionKind) {
      const condition = parseCondition(c);
      if (!condition) return null;
      return { ...parsed, rule: { ...rule, condition, conditionKind } };
    }
    const duration = c.attempt(c => {
      if (!c.isWord("as", "for")) return null;
      return parseDuration(c);
    });
    if (duration) {
      if (duration.duration !== "as-long-as") return null;
      return { ...parsed, rule: { ...rule, condition: duration.condition, conditionKind: "if" } };
    }
    return parsed;
  });
}

interface ParsedClause { what: ObjectRef; rule: CombatRule; duration?: Duration }

/** "All creatures able to block X do so" requires each of those creatures to
 * block X if able (CR 509.1c). It is the same requirement as "[creature]
 * blocks X if able", stated for every potential blocker at once. */
function parseLure(c: Cursor): ParsedClause | null {
  return c.attempt((c): ParsedClause | null => {
    const what = parseObjectRef(c);
    if (!what || (what.ref !== "all" && what.ref !== "each") || !c.words("able", "to", "block")) return null;
    const attackers = parseObjectRef(c);
    if (!attackers) return null;
    const duration = parseRequirementPeriod(c);
    if (!c.words("do", "so")) return null;
    return { what, rule: { rule: "block-requirement", attackers }, ...(duration ? { duration } : {}) };
  });
}

function parseClause(c: Cursor, previousSubject?: ObjectRef): ParsedClause | null {
  return c.attempt((c): ParsedClause | null => {
    const lure = parseLure(c);
    if (lure) return lure;
    const what = parseGenericCombatRef(c) ?? previousSubject;
    if (!what) return null;
    const first = parseRule(c);
    if (!first) return null;
    const rules = [first];
    while (c.attempt(c => {
      if (c.word("and") === null) return null;
      const next = parseRule(c);
      if (!next) return null;
      rules.push(next);
      return true;
    })) { /* coordinated clauses with the same subject */ }
    if (rules.length === 1) return { what, ...first };
    // One printed period cannot be assigned to coordinated rules by guesswork.
    if (rules.some(rule => rule.duration)) return c.fail("explicit period for each coordinated combat rule");
    return { what, rule: { rule: "all-of", rules: rules.map(rule => rule.rule) } };
  });
}

function targetedFilter(filter: ObjectFilter): boolean {
  const hasTarget = (value: unknown): boolean => {
    if (!value || typeof value !== "object") return false;
    if (Array.isArray(value)) return value.some(hasTarget);
    const node = value as Record<string, unknown>;
    return node.ref === "target" || node.ref === "any-target"
      || node.player === "target-player" || node.player === "target-opponent"
      || Object.values(node).some(hasTarget);
  };
  return hasTarget(filter);
}

/** Parse a complete recurring static clause. Detailed temporary clauses are
 * left to the resolving-effect parser, whose caller owns their duration. */
export function parseCombatStatic(c: Cursor): StaticEffect | null {
  return c.attempt(c => {
    if (c.words("as", "long", "as")) {
      const condition = parseCondition(c);
      if (!condition || !c.punct(",")) return null;
      const effect = parseCombatStatic(c);
      return effect && { static: "conditional", condition, effect };
    }
    const start = c.pos;
    const clause = parseClause(c);
    // A stated period belongs to a resolving effect, never a static ability.
    if (!clause || clause.duration || clause.what.ref === "target" || clause.what.ref === "any-target") return null;
    if ("filter" in clause.what && targetedFilter(clause.what.filter)) return null;
    c.punct(".");
    if (!c.done() && !c.isWord("otherwise")) return c.fail("end of static combat rule");
    // Preserve the previously supported simple static node byte for byte.
    const legacy = new Cursor(c.tokens.slice(start, c.pos));
    if (parseObjectRef(legacy) && legacy.words("attacks", "each", "combat", "if", "able")) {
      legacy.punct(".");
      if (legacy.done()) return null;
    }
    return { static: "combat-rule", what: clause.what, rule: clause.rule };
  });
}

/** One-shot changes of combat status (CR 506.4, 509.1h). Neither is a
 * continuous effect, so neither takes a duration. */
function parseCombatAction(c: Cursor): Effect | null {
  return c.attempt((c): Effect | null => {
    if (c.word("remove") !== null) {
      const what = parseObjectRef(c);
      return what && c.words("from", "combat") ? { effect: "remove-from-combat", what } : null;
    }
    const what = parseObjectRef(c);
    if (!what || c.word("become", "becomes") === null || c.word("blocked") === null) return null;
    // "becomes blocked by X" names a blocker and is a different instruction.
    return c.isWord("by") ? null : { effect: "become-blocked", what };
  });
}

/** "the player or planeswalker it's attacking": the recipient is whatever
 * that creature attacks, never a target (CR 508.1b). The printed kinds are
 * kept, so an attacker of an unlisted kind of permanent has no recipient. */
export function parseAttackedRecipient(c: Cursor): Extract<DamageTarget, { to: "attacked" }> | null {
  return c.attempt(c => {
    if (c.word("the") === null) return null;
    const kinds: ("player" | "planeswalker" | "battle")[] = [];
    for (;;) {
      const kind = c.word("player", "planeswalker", "battle") as "player" | "planeswalker" | "battle" | null;
      if (!kind || kinds.includes(kind)) return null;
      kinds.push(kind);
      if (c.punct(",")) { c.word("or"); continue; }
      if (c.word("or") === null) break;
    }
    let by: ObjectRef | null;
    if (c.word("it's") !== null) by = { ref: "it" };
    else {
      by = parseObjectRef(c);
      if (!by || c.word("is") === null) return null;
    }
    return c.word("attacking") !== null ? { to: "attacked", by, kinds } : null;
  });
}

/** Continuous rules created during resolution; a following duration belongs
 * to the outer effect grammar. Condition fields on the rule remain dynamic. */
export function parseCombatEffect(c: Cursor, previousSubject?: ObjectRef): Effect[] | null {
  return c.attempt(c => {
    const clause = parseClause(c, previousSubject);
    if (clause) return [{ effect: "combat-rule", what: clause.what, rule: clause.rule, ...(clause.duration ? { duration: clause.duration } : {}) }];
    const oneShot = parseCombatAction(c);
    if (oneShot) return [oneShot];
    // "... and can't be blocked this turn": the same simple prohibition as
    // with a printed subject, applied to the coordinated clause's subject.
    if (!previousSubject || c.word("can't") === null) return null;
    let action: CantAction;
    if (c.words("attack", "or", "block")) action = "attack-or-block";
    else if (c.word("attack") !== null) action = "attack";
    else if (c.word("block") !== null) action = "block";
    else if (c.words("be", "blocked")) action = "be-blocked";
    else return null;
    return [{ effect: "cant", what: previousSubject, action }];
  });
}
