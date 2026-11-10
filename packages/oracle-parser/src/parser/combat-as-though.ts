// Combat rules stated as a counterfactual or as a changed way of assigning
// combat damage. CR 609.4: "as though" applies only to the stated action; for
// every other purpose the game is unchanged. CR 510.1a–c: how much combat
// damage a creature assigns and to what.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { CombatDefender, CombatPretense, CombatRule, Duration, KeywordInstance, ObjectRef, PlayerRef } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseKeywordName, parseLandwalkName } from "./keywords.ts";
import { parseFilter, parseObjectRef, parsePlayerRef } from "./refs.ts";

export interface AsThoughRule { rule: CombatRule; duration?: Duration }
export interface AsThoughClause extends AsThoughRule { what: ObjectRef }

interface Helpers {
  defender: (c: Cursor) => CombatDefender | null;
  objects: (c: Cursor) => ObjectRef | null;
  /** The creatures the rule is stated for. */
  subject: ObjectRef;
}

function parsePeriod(c: Cursor): Duration | null {
  if (c.words("this", "turn")) return { duration: "this-turn" };
  if (c.words("this", "combat")) return { duration: "this-combat" };
  return null;
}

/** The creature the rule is about, restated after "as though". */
function parseAffectedPronoun(c: Cursor): "singular" | "plural" | null {
  if (c.word("it") !== null || c.words("that", "creature")) return "singular";
  if (c.word("they") !== null || c.words("those", "creatures")) return "plural";
  return null;
}

/** "landwalk abilities" names every landwalk ability (CR 702.14a). */
function parsePretendedKeyword(c: Cursor): { keyword: KeywordInstance; name: string } | null {
  return c.attempt((c): { keyword: KeywordInstance; name: string } | null => {
    // "mountainwalk" is that one landwalk ability, not every landwalk.
    const walk = parseLandwalkName(c);
    if (walk) return walk;
    const keyword = parseKeywordName(c);
    if (!keyword) return null;
    if (c.word("abilities") !== null && keyword !== "landwalk") return null;
    return { keyword: { keyword }, name: keyword };
  });
}

function hasAbility(what: ObjectRef | undefined, keyword: string): boolean {
  return !!what && "filter" in what && what.filter.withAbility?.includes(keyword) === true;
}

/** "as though it didn't have defender", "as though it had haste", "as though
 * they were untapped". With blocked creatures printed, a plural pronoun may
 * denote them instead of the blocker; it does so only when the pretense
 * removes the very ability that printed filter requires ("creatures with
 * shadow as though they didn't have shadow"). */
function parsePretense(c: Cursor, subject: ObjectRef, attackers?: ObjectRef): CombatPretense | null {
  return c.attempt((c): CombatPretense | null => {
    if (!c.words("as", "though")) return null;
    const number = parseAffectedPronoun(c);
    if (!number) return null;
    if (c.word("were") !== null) {
      // The tapped/untapped pretense concerns the declared creature only.
      return !attackers && c.word("untapped") !== null ? { pretend: "untapped", of: "affected-creature" } : null;
    }
    const lacks = c.words("didn't", "have");
    if (!lacks && c.word("had") === null) return null;
    let pretended: { keyword: KeywordInstance; name: string } | null;
    if (lacks && c.words("those", "abilities")) {
      // Anaphor for the abilities named by the printed creature filter.
      pretended = hasAbility(attackers ?? subject, "landwalk") ? { keyword: { keyword: "landwalk" }, name: "landwalk" } : null;
    } else pretended = parsePretendedKeyword(c);
    if (!pretended) return null;
    const keyword = pretended.keyword;
    if (!attackers) return { pretend: lacks ? "lacks-keyword" : "has-keyword", of: "affected-creature", keyword };
    const aboutAttackers = lacks && hasAbility(attackers, pretended.name);
    if (number === "singular") {
      // A singular pronoun after plural attackers is the blocker.
      return aboutAttackers ? c.fail("unambiguous creature for the pretense")
        : { pretend: lacks ? "lacks-keyword" : "has-keyword", of: "affected-creature", keyword };
    }
    return aboutAttackers ? { pretend: "lacks-keyword", of: "blocked-creature", keyword }
      : c.fail("unambiguous creature for the pretense");
  });
}

/** "<subject> can attack/block/be blocked [this turn] as though …". */
export function parseAsThoughRule(c: Cursor, helpers: Helpers): AsThoughRule | null {
  return c.attempt((c): AsThoughRule | null => {
    if (c.word("can") === null) return null;
    let action: "attack" | "block" | "be-blocked";
    if (c.words("be", "blocked")) action = "be-blocked";
    else {
      const verb = c.word("attack", "block") as "attack" | "block" | null;
      if (!verb) return null;
      action = verb;
    }
    let duration = parsePeriod(c);
    let defenders: CombatDefender[] | undefined;
    let attackers: ObjectRef | undefined;
    if (!duration && !c.isWord("as")) {
      if (action === "attack") {
        // "your opponents": an attack on any of them (CR 508.1b).
        const defender = (c: Cursor): CombatDefender | null => c.words("your", "opponents")
          ? { defender: "player", who: { player: "each-opponent" } } : helpers.defender(c);
        const first = defender(c);
        if (!first) return null;
        defenders = [first];
        while (c.attempt(c => {
          if (c.word("and", "or") === null) return null;
          const next = defender(c);
          if (!next) return null;
          defenders!.push(next);
          return true;
        })) { /* every printed defender is retained */ }
      } else if (action === "block") {
        const objects = helpers.objects(c);
        if (!objects) return null;
        attackers = objects;
      } else return null;
      duration = parsePeriod(c);
    }
    const pretense = parsePretense(c, helpers.subject, attackers);
    if (!pretense) return null;
    // Defender and haste concern attacking; no other pretense is printed for it.
    if (action === "attack" && !(pretense.pretend === "lacks-keyword" && pretense.keyword.keyword === "defender")
      && !(pretense.pretend === "has-keyword" && pretense.keyword.keyword === "haste")) return c.fail("attack pretense about defender or haste");
    if (action !== "attack" && pretense.pretend !== "untapped" && ["defender", "haste"].includes(pretense.keyword.keyword))
      return c.fail("blocking pretense");
    if (action === "be-blocked" && pretense.pretend !== "lacks-keyword") return c.fail("evasion ability the creature is treated as lacking");
    return { rule: { rule: "as-though", action, pretense, ...(defenders ? { defenders } : {}), ...(attackers ? { attackers } : {}) },
      ...(duration ? { duration } : {}) };
  });
}

/** "assigns combat damage equal to its toughness rather than its power"
 * (CR 510.1a). The possessives all denote the assigning creature. */
export function parseAssignmentValueRule(c: Cursor): CombatRule | null {
  return c.attempt((c): CombatRule | null => {
    if (c.word("assign", "assigns") === null || !c.words("combat", "damage", "equal", "to")) return null;
    const possessive = c.word("its", "their");
    if (!possessive) return null;
    let value: "toughness" | "mana-value";
    if (c.word("toughness") !== null) value = "toughness";
    else if (c.words("mana", "value")) value = "mana-value";
    else return null;
    if (!c.words("rather", "than", possessive, "power")) return null;
    return { rule: "damage-assignment-value", value };
  });
}

function parseUnblockedTail(c: Cursor, number: "singular" | "plural"): AsThoughRule | null {
  return c.attempt((c): AsThoughRule | null => {
    if (c.word(number === "singular" ? "its" : "their") === null || !c.words("combat", "damage")) return null;
    const duration = parsePeriod(c);
    if (!c.words("as", "though") || c.word(number === "singular" ? "it" : "they") === null) return null;
    if (c.word("weren't") === null || c.word("blocked") === null) return null;
    return { rule: { rule: "assign-as-unblocked" }, ...(duration ? { duration } : {}) };
  });
}

function refNumber(what: ObjectRef): "singular" | "plural" | null {
  if (["self", "this", "it", "enchanted", "equipped"].includes(what.ref)) return "singular";
  if (what.ref === "that") return what.plural ? "plural" : "singular";
  if (what.ref === "each" || what.ref === "all" || what.ref === "them") return "plural";
  if (what.ref === "target" || what.ref === "filter")
    return what.count.amount === "fixed" && what.count.value === 1 && !what.upTo ? "singular" : "plural";
  return null;
}

/** CR 510.1c: a blocked creature's combat damage assigned as though it
 * weren't blocked (510.1b). "[Player] may have [creature] assign …" leaves
 * the decision to that player; "[creatures] assign …" does not.
 * "For each [creature], you may have that creature …" states the same
 * permission separately for every matching creature. */
export function parseUnblockedAssignmentClause(c: Cursor): AsThoughClause | null {
  return c.attempt((c): AsThoughClause | null => {
    let distributed: ObjectRef | undefined;
    if (c.words("for", "each")) {
      const filter = parseFilter(c);
      if (!filter || !c.punct(",")) return null;
      distributed = { ref: "each", filter };
    }
    // The effect grammar reads a leading causative "you may have" itself
    // and resumes at the creature; the permission is still the player's.
    const word = (offset: number, value: string): boolean => {
      const token = c.tokens[c.pos - offset];
      return token?.kind === "word" && !token.possessive && token.value === value;
    };
    const causativeConsumed = !distributed && word(1, "have") && word(2, "may") && word(3, "you");
    const chooser: PlayerRef | null = causativeConsumed ? { player: "you" } : c.attempt(c => {
      const who = parsePlayerRef(c);
      return who && c.words("may", "have") ? who : null;
    });
    if (distributed) {
      if (!chooser || !c.words("that", "creature") || c.word("assign") === null) return null;
      const tail = parseUnblockedTail(c, "singular");
      return tail && { what: distributed, ...tail, rule: { rule: "assign-as-unblocked", chooser } };
    }
    let what: ObjectRef | null;
    if (chooser?.player === "controller" && c.word("it") !== null) what = chooser.of;
    else what = parseObjectRef(c);
    if (!what) return null;
    const number = refNumber(what);
    if (!number || c.word(chooser ? "assign" : number === "singular" ? "assigns" : "assign") === null) return null;
    const tail = parseUnblockedTail(c, number);
    if (!tail) return null;
    return { what, ...tail, rule: { rule: "assign-as-unblocked", ...(chooser ? { chooser } : {}) } };
  });
}
