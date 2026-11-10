// Keyword actions and player designations as effects (CR 701, 725, 726).
// Every production returns a complete typed node or null; the actor, the
// object acted on and each quantity stay in the node.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Condition, Effect, ObjectRef, PlayerRef, RoleName, Sentence } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import type { EffectContext } from "./effects.ts";
import { parseBeholdEffect } from "./behold.ts";
import { parseVoteStep } from "./votes.ts";
import { parseUnlockDoor } from "./rooms.ts";
import { parseCount, parseDuration, parseEqualTo, isPluralPlayerRef, parseObjectRef, parsePlayerRef } from "./refs.ts";
import { parseOpponentUnit } from "./player-counts.ts";

const you: PlayerRef = { player: "you" };
const ROLES: Readonly<Record<string, RoleName>> = {
  cursed: "Cursed", monster: "Monster", royal: "Royal", sorcerer: "Sorcerer",
  virtuous: "Virtuous", wicked: "Wicked", young: "Young Hero",
};

/** Same actor discipline as the other effect parsers: an explicit subject
 * takes the conjugated phrase; an imperative acts for you, or for a plural
 * actor already in force. `explicitOnly` rejects the imperative (used where
 * the printed text never has it, e.g. "become the monarch"). */
function parseActorPhrase(
  c: Cursor, ctx: EffectContext, imperative: string[], conjugated: string[], explicitOnly = false,
): PlayerRef | null {
  return c.attempt(c => {
    const explicit = parsePlayerRef(c);
    // "you" and plural subjects take the base form; singular ones the -s form.
    const explicitForm = !!explicit && c.words(...conjugated);
    const imperativeForm = !explicitForm && c.words(...imperative);
    // "Its owner shuffles it into their library, then investigates": a
    // conjugated verb without a subject continues the previous actor.
    const elided = !explicit && !imperativeForm && !explicitOnly && !!ctx.lastPlayer && ctx.lastPlayer.player !== "you"
      && c.words(...conjugated);
    if (!explicitForm && !imperativeForm && !elided) return null;
    if (!explicit && explicitOnly) return null;
    const retainsPluralActor = ctx.lastPlayer?.player === "they" || ctx.lastPlayer?.player === "group";
    // "[Player] may investigate": the other player whose "may" governs it acts.
    const who = explicit ?? (elided ? ctx.lastPlayer : ctx.impliedActor ?? (retainsPluralActor ? ctx.lastPlayer : you));
    if (!who) return c.fail("actor of keyword action");
    if (who.player === "group") return c.fail("supported grouped-player action");
    ctx.lastPlayer = who;
    return who;
  });
}

/** Parameterless actions: the whole instruction is the phrase after the actor. */
const SIMPLE_ACTIONS: {
  effect: "become-monarch" | "take-initiative" | "learn" | "forage" | "time-travel" | "open-attraction" | "venture" | "planeswalk";
  imperative: string[]; conjugated: string[]; rest: string[]; explicitOnly?: true;
}[] = [
  // CR 725.2: a player becomes the monarch.
  { effect: "become-monarch", imperative: ["become"], conjugated: ["becomes"], rest: ["the", "monarch"], explicitOnly: true },
  // CR 726.2: a player takes the initiative.
  { effect: "take-initiative", imperative: ["take"], conjugated: ["takes"], rest: ["the", "initiative"], explicitOnly: true },
  { effect: "learn", imperative: ["learn"], conjugated: ["learns"], rest: [] },
  { effect: "forage", imperative: ["forage"], conjugated: ["forages"], rest: [] },
  { effect: "time-travel", imperative: ["time", "travel"], conjugated: ["time", "travels"], rest: [] },
  { effect: "open-attraction", imperative: ["open"], conjugated: ["opens"], rest: ["an", "attraction"] },
  // CR 701.31b: only the planar controller planeswalks (901.7).
  { effect: "planeswalk", imperative: ["planeswalk"], conjugated: ["planeswalks"], rest: [] },
  { effect: "venture", imperative: ["venture"], conjugated: ["ventures"], rest: ["into", "the", "dungeon"] },
];

/** "twice" / "N times" / "X times" after a repeatable action. */
function parseTimes(c: Cursor, thatMany = false): Amount | null {
  return c.attempt((c): Amount | null => {
    if (c.word("twice") !== null) return { amount: "fixed", value: 2 };
    // "investigate that many times": the quantity of the triggering event.
    if (thatMany && c.words("that", "many", "times")) return { amount: "that-much" };
    const count = parseCount(c);
    if (!count || c.word("times") === null) return null;
    return count;
  });
}

function parseSimple(c: Cursor, ctx: EffectContext): Effect[] | null {
  for (const spec of SIMPLE_ACTIONS) {
    const result = c.attempt((c): Effect[] | null => {
      const who = parseActorPhrase(c, ctx, spec.imperative, spec.conjugated, spec.explicitOnly);
      if (!who || !c.words(...spec.rest)) return null;
      return [{ effect: spec.effect, who }];
    });
    if (result) return result;
  }
  return null;
}

// Contraption rules are outside the CR: "it assembles a Contraption" and
// "this Contraption assembles a number of Contraptions equal to <amount>".
function parseAssemble(c: Cursor): Effect[] | null {
  const by = parseObjectRef(c);
  if (!by || (by.ref !== "it" && !(by.ref === "this" && by.noun === "contraption")) || c.word("assembles") === null) return null;
  if (c.words("a", "contraption")) return [{ effect: "assemble-contraption", by, count: { amount: "fixed", value: 1 } }];
  if (!c.words("a", "number", "of", "contraptions")) return null;
  const count = parseEqualTo(c);
  return count && [{ effect: "assemble-contraption", by, count }];
}

// CR 311.7: a resolving spell or ability says that chaos ensues.
function parseChaosEnsues(c: Cursor): Effect[] | null {
  return c.words("chaos", "ensues") ? [{ effect: "chaos-ensues" }] : null;
}

// CR 701.54a: "the Ring tempts you" — the tempted player is the object.
function parseRingTempts(c: Cursor, ctx: EffectContext): Effect[] | null {
  if (!c.words("the", "ring", "tempts")) return null;
  const who = parsePlayerRef(c);
  if (!who) return null;
  ctx.lastPlayer = who;
  return [{ effect: "ring-tempts", who }];
}

// Proliferate/populate/investigate by an explicit actor, and the repeated
// forms: "proliferate twice", "investigate X times", "target player investigates".
function parseRepeatableAction(c: Cursor, ctx: EffectContext): Effect[] | null {
  for (const action of ["proliferate", "populate", "investigate"] as const) {
    const parsed = c.attempt((c): Effect[] | null => {
      const explicit = c.isWord(action);
      const who = parseActorPhrase(c, ctx, [action], [action === "proliferate" ? "proliferates" : action === "populate" ? "populates" : "investigates"]);
      if (!who) return null;
      // "proliferate, then proliferate again": the action twice in sequence.
      const again = c.attempt(c => (c.punct(",") && c.word("then") !== null && c.word(action) !== null && c.word("again") !== null ? true : null)) === true;
      // "Investigate an additional time" is one more ordinary instance of the
      // action, as "an additional card" is an ordinary draw (CR 701.16a).
      const additional = !again && action === "investigate" && c.attempt(c => (c.words("an", "additional", "time") ? true : null)) === true;
      let times: Amount | null | undefined = again ? { amount: "fixed", value: 2 } as const : additional ? undefined : parseTimes(c, true);
      // "investigate once for each opponent who …": the action happens once per
      // counted player, which is that many times (CR 701.16a).
      if (times === null) times = c.attempt(c => c.word("once") !== null && c.words("for", "each") ? parseOpponentUnit(c, { parsePlayerRef, isPluralPlayerRef }) : null);
      // "investigate once for each X" is "investigate for each X" (CR 701.16a
      // repeated per matching object); the per-object rider is attached later.
      const once = !times && c.attempt(c => (c.word("once") !== null && c.isWord("for") ? true : null)) === true;
      // Plain imperative forms are the existing parser's; take only new wording.
      if (explicit && !times && !once && !additional) return null;
      return [{ effect: "keyword-action", action, who, ...(times ? { times } : {}) }];
    });
    if (parsed) return parsed;
  }
  return null;
}

// CR 701.59a: the effect form ("you may collect evidence 4"); the paying
// player chooses the exiled cards, so only the threshold is carried.
function parseCollectEvidence(c: Cursor, ctx: EffectContext): Effect[] | null {
  const who = parseActorPhrase(c, ctx, ["collect", "evidence"], ["collects", "evidence"]);
  if (!who) return null;
  const amount = parseCount(c);
  return amount ? [{ effect: "collect-evidence", who, amount }] : null;
}

// CR 701.29a: the opponent whose library is looked at is not named in the text.
function parseFateseal(c: Cursor, ctx: EffectContext): Effect[] | null {
  const who = parseActorPhrase(c, ctx, ["fateseal"], ["fateseals"]);
  if (!who || who.player !== "you") return null;
  const amount = parseCount(c);
  return amount ? [{ effect: "fateseal", who, amount }] : null;
}

function parseIncubate(c: Cursor, ctx: EffectContext): Effect[] | null {
  const who = parseActorPhrase(c, ctx, ["incubate"], ["incubates"]);
  if (!who) return null;
  const amount = parseCount(c);
  if (!amount) return null;
  const times = parseTimes(c);
  return [{ effect: "incubate", who, amount, ...(times ? { times } : {}) }];
}

// CR 701.66a: "Earthbend N" targets a land its performer controls.
function parseEarthbend(c: Cursor, ctx: EffectContext): Effect[] | null {
  const who = parseActorPhrase(c, ctx, ["earthbend"], ["earthbends"]);
  if (!who || who.player !== "you") return null;
  const amount = parseCount(c);
  if (!amount) return null;
  const what: ObjectRef = { ref: "target", filter: { types: ["land"], control: "you" }, count: { amount: "fixed", value: 1 } };
  return [{ effect: "earthbend", who, amount, what }];
}

function parseAirbend(c: Cursor, ctx: EffectContext): Effect[] | null {
  const who = parseActorPhrase(c, ctx, ["airbend"], ["airbends"]);
  if (!who) return null;
  const what = parseObjectRef(c);
  return what ? [{ effect: "airbend", who, what }] : null;
}

// CR 701.15a: only the imperative; the goader is the ability's controller.
function parseGoad(c: Cursor): Effect[] | null {
  // "Tap and goad X" / "Untap and goad X": both instructions act on X.
  const prefix = c.attempt(c => (c.isWord("tap", "untap") ? c.word("tap", "untap") : null));
  if (prefix && !c.words("and", "goad")) return null;
  if (!prefix && c.word("goad") === null) return null;
  const what = parseObjectRef(c);
  if (!what) return null;
  const goad: Effect = { effect: "goad", what };
  return prefix ? [{ effect: prefix as "tap" | "untap", what }, goad] : [goad];
}

// CR 701.15a with a stated duration: "The tokens are goaded for the rest of the game."
function parseGoadedState(c: Cursor): Effect[] | null {
  // "It's goaded" / "They're goaded": the contracted copula after a pronoun.
  const contracted = c.word("it's") !== null ? { ref: "it" as const } : c.word("they're") !== null ? { ref: "them" as const } : null;
  // "The token(s)" after a creating instruction names the same created tokens as "that token" / "those tokens".
  const created = contracted ? null : c.attempt((c): ObjectRef | null => {
    const plural = c.words("the", "tokens");
    if (!plural && !c.words("the", "token")) return null;
    return { ref: "that", filter: { cls: "token" }, ...(plural ? { plural: true as const } : {}) };
  });
  const what = contracted ?? created ?? parseObjectRef(c);
  if (!what || (!contracted && c.word("is", "are") === null) || c.word("goaded") === null) return null;
  const duration = parseDuration(c);
  return duration ? [{ effect: "goad", what, duration }] : null;
}

// CR 701.63a.
function parseEndure(c: Cursor): Effect[] | null {
  const what = parseObjectRef(c);
  if (!what || c.word("endures") === null) return null;
  const amount = parseCount(c);
  return amount ? [{ effect: "endure", what, amount }] : null;
}

function parseEmpower(c: Cursor, ctx: EffectContext): Effect[] | null {
  const who = parseActorPhrase(c, ctx, ["empower"], ["empowers"]);
  if (!who) return null;
  // The action names the Jace planeswalker token; a Jace card's own name is
  // normalised to "~" in its text.
  if (c.word("jace") === null && !c.selfref()) return null;
  const amount = parseCount(c);
  return amount ? [{ effect: "empower-jace", who, amount }] : null;
}

// CR 701.30a-b.
function parseClash(c: Cursor, ctx: EffectContext): Effect[] | null {
  const who = parseActorPhrase(c, ctx, ["clash"], ["clashes"]);
  if (!who || c.word("with") === null) return null;
  const opponent = parsePlayerRef(c);
  if (!opponent || !["an-opponent", "defending-player", "target-opponent", "that-player"].includes(opponent.player)) return null;
  return [{ effect: "clash", who, with: opponent }];
}

// CR 111.10j-r, 303.7: Role tokens are created attached to a creature.
function parseCreateRole(c: Cursor, ctx: EffectContext): Effect[] | null {
  const who = parseActorPhrase(c, ctx, ["create"], ["creates"]);
  if (!who) return null;
  const count = parseCount(c);
  if (!count) return null;
  const first = c.anyWord();
  const role = first && ROLES[first.value];
  if (!role) return null;
  if (role === "Young Hero" && c.word("hero") === null) return null;
  if (c.word("role") === null || c.word("token", "tokens") === null) return null;
  if (!c.words("attached", "to")) return null;
  const attachedTo = parseObjectRef(c);
  if (!attachedTo) return null;
  return [{ effect: "create-role", role, count, attachedTo, ...(who.player !== "you" ? { who } : {}) }];
}

// scry/surveil by an explicit actor other than the imperative "you"
// (CR 701.22a, 701.25a). "you" is the default actor and is not repeated.
function parseScrySurveilBy(c: Cursor, ctx: EffectContext): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    const who = parsePlayerRef(c);
    if (!who) return null;
    const verb = c.word("scry", "scries", "surveil", "surveils");
    if (!verb) return null;
    const amount = parseCount(c);
    if (!amount) return null;
    ctx.lastPlayer = who;
    const effect = verb.startsWith("scr") ? "scry" : "surveil";
    return [{ effect, amount, ...(who.player === "you" ? {} : { who }) }];
  });
}

// CR 701.57a: another player discovers N ("that player discovers X"); "you"
// and the imperative stay with the existing parser.
function parseDiscoverBy(c: Cursor, ctx: EffectContext): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    const who = parsePlayerRef(c);
    if (!who || who.player === "you" || c.word("discovers") === null) return null;
    const amount = parseCount(c);
    if (!amount) return null;
    ctx.lastPlayer = who;
    return [{ effect: "discover", who, amount }];
  });
}

// CR 701.44a, 701.50d: "explores X times", "explores, then it explores again",
// "connives N". Plain "explores" / "connives" stay with the existing parser.
function parseExploreConniveVariants(c: Cursor): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    const what = parseObjectRef(c);
    if (!what) return null;
    const verb = c.word("explores", "connives");
    if (!verb) return null;
    if (verb === "connives") {
      const amount = parseCount(c);
      return amount ? [{ effect: "connive", what, amount }] : null;
    }
    const times = parseTimes(c);
    if (times) return [{ effect: "explore", what, times }];
    // "explores, then it explores again": the same permanent explores twice.
    const again = c.attempt(c => c.punct(",") && c.words("then", "it", "explores", "again") ? true : null);
    return again ? [{ effect: "explore", what, times: { amount: "fixed", value: 2 } }] : null;
  });
}

/** Step production registered in effects.ts STEP_PARSERS. Each alternative
 * rolls the cursor back on failure, so the next one starts at the same token. */
export function parseKeywordActionStep(c: Cursor, ctx: EffectContext): Effect[] | null {
  const alternatives: ((c: Cursor) => Effect[] | null)[] = [
    c => parseSimple(c, ctx), c => parseChaosEnsues(c), parseAssemble, c => parseRingTempts(c, ctx), c => parseRepeatableAction(c, ctx), c => parseIncubate(c, ctx), c => parseCollectEvidence(c, ctx), c => parseFateseal(c, ctx),
    c => parseEarthbend(c, ctx), c => parseAirbend(c, ctx), parseGoad, parseGoadedState, parseEndure, c => parseEmpower(c, ctx),
    parseBeholdEffect, parseUnlockDoor, c => parseClash(c, ctx), c => parseCreateRole(c, ctx), c => parseScrySurveilBy(c, ctx), c => parseDiscoverBy(c, ctx), parseExploreConniveVariants, parseVoteStep,
  ];
  for (const alternative of alternatives) {
    const result = c.attempt(alternative);
    if (result) return result;
  }
  return null;
}

// CR 701.30d: "If you win" refers to the preceding clash.
export function parseClashOutcomeCondition(c: Cursor): Condition | null {
  return c.attempt((c): Condition | null => {
    const who = parsePlayerRef(c);
    if (!who || who.player !== "you" || c.word("win") === null) return null;
    return { condition: "clash-result", who, result: "won" };
  });
}

/** A clash result is only meaningful after a clash in the same instruction
 * sequence. Returns false when a sentence tests it without one. */
export function clashOutcomeHasClash(previous: Sentence[], sentence: Sentence, contains: (value: unknown, kind: Effect["effect"]) => boolean): boolean {
  const tests = sentence.condition?.condition === "clash-result";
  return !tests || contains(previous, "clash");
}

// CR 702.179f: "your speed" as a quantity.
export function parseSpeedAmount(c: Cursor): Amount | null {
  return c.attempt((c): Amount | null => (c.words("your", "speed") ? { amount: "speed", who: you } : null));
}
