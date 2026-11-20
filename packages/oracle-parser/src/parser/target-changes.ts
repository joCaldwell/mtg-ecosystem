// Changing the targets of a spell or ability on the stack. CR 115.7a–c
// distinguish "the target(s)" (all or none), "a target" (one) and "any
// targets" (any number); each differs from choosing new targets (115.7d).
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Effect, ObjectFilter, ObjectRef, PlayerRef, Sentence } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseFilter, parseObjectRef, parsePlayerRef, targetSelectionProvenance } from "./refs.ts";

type ChangeTargets = Extract<Effect, { effect: "change-targets" }>;
type CombatRuleEffect = Extract<Effect, { effect: "combat-rule" }>;

const randomTargetSlot = { method: "random", unit: "target-slot", whenUnavailable: "keep" } as const;

/** Only called in the proven single-target stack trigger scope.
 * CR 115.7e–f and 115.9a; the random choice may leave a legal target in place. */
export function parseRandomReselectionOfSingleTarget(c: Cursor): ChangeTargets | null {
  return c.attempt(c => c.words("reselect", "its", "target", "at", "random") && (c.punct(".") || c.done()) && c.done() ? {
    effect: "change-targets", what: { ref: "it" }, who: { player: "you" }, scope: "each",
    reselection: randomTargetSlot,
  } : null);
}

/** Redirecting an attacker changes its current defender without declaring
 * it again or changing whom it was originally declared against (CR 508.7a–e). */
function parseReselectAttackDefender(c: Cursor): CombatRuleEffect | null {
  return c.attempt(c => {
    if (!c.words("reselect", "which", "player", "or", "permanent")) return null;
    const what = parseObjectRef(c);
    if (what?.ref !== "target" || !what.filter.status?.includes("attacking") || what.filter.types?.length !== 1 ||
      what.filter.types[0] !== "creature" || what.count.amount !== "fixed" || what.count.value !== 1 || what.upTo ||
      !c.words("is", "attacking")) return null;
    return { effect: "combat-rule", what, rule: { rule: "reselect-attack-defender", candidates: "player-or-permanent" } };
  });
}

/** Both the original and copy are randomized after creation (CR 115.7e–f,
 * 707.10). The exclusions apply to both sets of slots, even in the rare case
 * that a spell gains additional targets between announcement and resolution.
 * https://magic.wizards.com/en/news/feature/modern-horizons-2-release-notes-2021-06-04 */
export function parseRandomReselectionOfSpellAndCopy(c: Cursor, previous: Sentence | undefined): Sentence | null {
  return c.attempt(c => {
    if (!previous || previous.condition || previous.otherwise || previous.bindings || previous.steps.length !== 1) return null;
    const [gain] = previous.steps;
    if (gain.effect !== "gain-control" || gain.who.player !== "you" || gain.optional || gain.duration || gain.forEach ||
      gain.what.ref !== "target" || gain.what.count.amount !== "fixed" || gain.what.count.value !== 1 || gain.what.upTo ||
      gain.what.filter.cls !== "spell") return null;
    const only = gain.what.filter.targetsOnly;
    if (!only || !("ref" in only) || only.ref !== "filter" || only.count.amount !== "fixed" || only.count.value !== 1 ||
      only.upTo || only.filter.cls !== "permanent" || !only.filter.orPlayer) return null;
    if (!c.words("copy", "it") || !c.punct(",") ||
      !c.words("then", "reselect", "the", "targets", "at", "random", "for", "the", "spell", "and", "the", "copy") ||
      !c.punct(".") || !c.words("the", "new", "targets", "can't", "be", "you", "or", "a", "permanent", "you", "control") ||
      (!c.punct(".") && !c.done())) return null;
    const spell: ObjectRef = { ref: "that", filter: { cls: "spell" } };
    return { steps: [
      { effect: "copy-stack-object", what: spell, who: { player: "you" }, count: { amount: "fixed", value: 1 } },
      { effect: "change-targets", what: spell, also: [{ ref: "copy-result", number: "singular" }], who: { player: "you" },
        scope: "each", reselection: { ...randomTargetSlot, excluding: [
          { player: "you" }, { ref: "each", filter: { cls: "permanent", controlledBy: { player: "you" } } },
        ] } },
    ] };
  });
}

/** Only spells and abilities have targets to change (CR 115.1). */
function isStackObjectFilter(filter: ObjectFilter): boolean {
  if (filter.cls === "spell" || filter.cls === "ability") return true;
  return !!filter.allOf?.some(part => !!part.anyOf?.length && part.anyOf.every(isStackObjectFilter));
}

/** The chosen player pair is fixed on announcement (CR 115.3, 603.3d).
 * At each declaration the current controller of a creature identifies which
 * member of the pair it is required to attack (CR 508.1d, 611.2c). */
export function parsePairedPlayerAttackRule(c: Cursor, previous: Sentence | undefined): Sentence | null {
  return c.attempt(c => {
    if (!c.isWord("until") || !previous || previous.condition || previous.otherwise || previous.bindings ||
      previous.steps.length !== 1) return null;
    const [choose] = previous.steps;
    if (choose.effect !== "declare-target" || choose.asOf || choose.optional || choose.forEach ||
      !("player" in choose.what) || choose.what.player !== "target-player" || choose.what.upTo || choose.what.random ||
      choose.what.count?.amount !== "fixed" || choose.what.count.value !== 2) return null;
    if (!c.words("until", "your", "next", "turn") || !c.punct(",") ||
      !c.words("each", "creature", "they", "control", "attacks", "the", "other", "chosen", "player", "each", "combat", "if", "able") ||
      (!c.punct(".") && !c.done())) return null;
    const pair = { player: "those-players" } as const;
    return { steps: [{
      effect: "combat-rule",
      what: { ref: "each", filter: { types: ["creature"], controlledBy: pair } },
      rule: { rule: "attack-requirement", frequency: "each-combat",
        defender: { defender: "other-player-in-pair", pair, comparedTo: "attacker-controller" } },
      duration: { duration: "your-next-turn" },
    }] };
  });
}

/** Called only with a proven single-permanent enters trigger. The entering
 * permanent's controller selects the target at announcement (CR 603.3d);
 * both participants and their controller identities remain relative to
 * those permanents rather than to the ability's controller (CR 701.12a–b). */
export function parseEnteredPermanentExchange(c: Cursor): Sentence[] | null {
  return c.attempt(c => {
    if (!c.words("its", "controller", "chooses")) return null;
    const target = c.peek();
    if (!c.words("target", "permanent", "another", "player", "controls", "that", "shares", "a", "card", "type", "with", "it") ||
      !c.punct(".") || !c.words("exchange", "control", "of", "those", "permanents") ||
      (!c.punct(".") && !c.done())) return null;
    const entrant: ObjectRef = { ref: "trigger-event-object", event: "enter" };
    const controller: PlayerRef = { player: "controller", of: entrant };
    const selected: ObjectRef = {
      ref: "target", count: { amount: "fixed", value: 1 }, chosenBy: controller,
      filter: {
        cls: "permanent",
        controlledBy: { player: "filtered", base: { player: "each-player" }, filter: { otherThan: controller } },
        characteristicRelations: [{
          relation: "shared-value", characteristic: "card-type", with: { operand: "object", what: entrant },
        }],
      },
      ...targetSelectionProvenance(target),
    };
    return [
      { steps: [{ effect: "declare-target", what: selected }] },
      { steps: [{ effect: "exchange-control", what: [
        entrant, { ref: "choice-result", number: "singular", filter: { cls: "permanent" }, which: "chosen", scope: "contextual" },
      ] }] },
    ];
  });
}

/** The affected spell or ability must be an explicit reference; the acting
 * player is the controller of this instruction ("you"). A following "The
 * new target must be …" sentence restricts the same change (CR 608.2c) and
 * is consumed here, leaving the sentence's final period to the caller. */
export function parseChangeTargets(c: Cursor): ChangeTargets | CombatRuleEffect | null {
  if (c.isWord("reselect")) return parseReselectAttackDefender(c);
  return c.attempt((c): ChangeTargets | null => {
    if (c.word("change") === null) return null;
    const scope: ChangeTargets["scope"] | null = c.words("the", "target") || c.words("the", "targets") ? "all"
      : c.words("a", "target") ? "one" : c.words("any", "targets") ? "any" : null;
    if (!scope || c.word("of") === null) return null;
    const what = parseObjectRef(c);
    if (!what || !("filter" in what) || !isStackObjectFilter(what.filter)) return c.fail("spell or ability whose targets change");
    const to = c.attempt((c): ObjectRef | PlayerRef | null =>
      c.word("to") !== null ? c.attempt(parsePlayerRef) ?? parseObjectRef(c) : null);
    const newTarget = to ? null : c.attempt((c): ChangeTargets["newTarget"] | null => {
      if (!c.punct(".") || !c.words("the", "new", "target", "must", "be")) return null;
      if (c.words("a", "player")) return { kind: "player" };
      if (c.word("a", "an") === null) return null;
      const filter = parseFilter(c);
      return filter && { kind: "object", filter };
    });
    return { effect: "change-targets", what, who: { player: "you" }, scope,
      ...(to ? { to } : {}), ...(newTarget ? { newTarget } : {}) };
  });
}
