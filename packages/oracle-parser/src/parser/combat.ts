// Detailed combat restrictions, requirements, and blocking capacities.
// CR 508.1c–d and 509.1a–c distinguish these three concepts; notably an
// evasion restriction never requires the defending player to block.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, CombatDefender, CombatRule, Comparison, Effect, ObjectFilter, ObjectRef, StaticEffect } from "../ast.ts";
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

function parseRuleBase(c: Cursor): CombatRule | null {
  return c.attempt(c => {
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
    const must = c.word("must") !== null;
    if (c.word("attack", "attacks") === null) return null;
    const defender = parseDefender(c);
    let frequency: "each-combat" | "each-turn" = "each-combat";
    if (c.words("each", "combat")) frequency = "each-combat";
    else if (c.words("each", "turn")) frequency = "each-turn";
    else if (!must && !c.isWord("if")) return null;
    if (!c.words("if", "able")) return null;
    return { rule: "attack-requirement", frequency, ...(defender ? { defender } : {}) };
  });
}

function parseRule(c: Cursor): CombatRule | null {
  return c.attempt(c => {
    const rule = parseRuleBase(c);
    if (!rule) return null;
    const conditionKind = c.word("if", "unless") as "if" | "unless" | null;
    if (conditionKind) {
      const condition = parseCondition(c);
      if (!condition) return null;
      return { ...rule, condition, conditionKind };
    }
    const duration = c.attempt(c => {
      if (!c.isWord("as", "for")) return null;
      return parseDuration(c);
    });
    if (duration) {
      if (duration.duration !== "as-long-as") return null;
      return { ...rule, condition: duration.condition, conditionKind: "if" };
    }
    return rule;
  });
}

function parseClause(c: Cursor, previousSubject?: ObjectRef): { what: ObjectRef; rule: CombatRule } | null {
  return c.attempt(c => {
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
    return { what, rule: rules.length === 1 ? first : { rule: "all-of", rules } };
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
    if (!clause || clause.what.ref === "target" || clause.what.ref === "any-target") return null;
    if ("filter" in clause.what && targetedFilter(clause.what.filter)) return null;
    c.punct(".");
    if (!c.done() && !c.isWord("otherwise")) return c.fail("end of static combat rule");
    // Preserve the previously supported simple static node byte for byte.
    const legacy = new Cursor(c.tokens.slice(start, c.pos));
    if (parseObjectRef(legacy) && legacy.words("attacks", "each", "combat", "if", "able")) {
      legacy.punct(".");
      if (legacy.done()) return null;
    }
    return { static: "combat-rule", ...clause };
  });
}

/** Continuous rules created during resolution; a following duration belongs
 * to the outer effect grammar. Condition fields on the rule remain dynamic. */
export function parseCombatEffect(c: Cursor, previousSubject?: ObjectRef): Effect[] | null {
  return c.attempt(c => {
    const clause = parseClause(c, previousSubject);
    return clause ? [{ effect: "combat-rule", ...clause }] : null;
  });
}
