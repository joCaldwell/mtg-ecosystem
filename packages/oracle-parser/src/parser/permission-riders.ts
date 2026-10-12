// A sentence after a cast/play permission can qualify every use of that
// permission ("If you cast a spell this way, …"). Such a sentence is part of
// the permission (CR 608.2c reads the whole text together); it is neither a
// resolution-time test nor a separate ability. Folded properties use the same
// fields as the one-sentence wording ("… by paying life … rather than paying
// its mana cost", "… as though they had flash").
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt

import type {
  BattlefieldEntryModifiers, CardPlayInstruction, Effect, ObjectFilter, PermissionRider, Sentence,
} from "../ast.ts";
import type { Token } from "../lexer.ts";
import { CARD_TYPES } from "../vocab.ts";
import { Cursor } from "./cursor.ts";
import { parseCount, parseCounterSpec, parseFilter } from "./refs.ts";

type SentenceParser = (c: Cursor) => Sentence[] | null;
// Late-bound by effects.ts, as with quoted-ability line parsing, so this
// module never imports the effect grammar that itself imports permissions.
let sentenceParser: SentenceParser | null = null;
export function registerRiderSentenceParser(parser: SentenceParser): void {
  sentenceParser = parser;
}

type Lead =
  | { lead: "spell-cast"; appliesTo?: ObjectFilter }
  | { lead: "used" }
  | { lead: "off-sorcery-timing" };
type Change = (permission: CardPlayInstruction) => CardPlayInstruction | null;
type EntryCounter = NonNullable<BattlefieldEntryModifiers["counters"]>[number];

function parseLead(c: Cursor): Lead | null {
  return c.attempt((c): Lead | null => {
    if (!c.words("if", "you")) return null;
    let lead: Lead;
    if (c.word("do") !== null) lead = { lead: "used" };
    else if (c.word("cast") === null) return null;
    else if (c.words("it", "any", "time", "a", "sorcery", "couldn't", "have", "been", "cast")) lead = { lead: "off-sorcery-timing" };
    else {
      if (c.word("a", "an") === null) return null;
      lead = { lead: "spell-cast" };
      if (c.word("spell") === null) {
        // "a creature spell": only the nominal up to its head may be a filter.
        const head = c.tokens.slice(c.pos, c.pos + 6).findIndex(token => token.kind === "word" && !token.possessive && token.value === "spell");
        if (head <= 0) return null;
        const nominal = new Cursor(c.tokens.slice(c.pos, c.pos + head + 1));
        const appliesTo = parseFilter(nominal);
        if (!appliesTo || !nominal.done() || appliesTo.cls !== "spell") return null;
        c.pos += head + 1;
        lead.appliesTo = appliesTo;
      }
      if (!c.words("this", "way")) return null;
    }
    return c.punct(",") ? lead : null;
  });
}

function parseEntryCounters(c: Cursor): EntryCounter[] | null {
  return c.attempt((c) => {
    if (c.word("with") === null) return null;
    const count = parseCount(c);
    if (!count) return null;
    const additional = c.word("additional") !== null;
    const counter = parseCounterSpec(c);
    if (!counter || c.word("counter", "counters") === null || !c.words("on", "it")) return null;
    return [{ count, counter, ...(additional ? { additional: true as const } : {}) }];
  });
}

/** The entering permanent is named by a pronoun, by the type the permission
 * (or the rider's own filter) guarantees, or as the source card itself. Any
 * other noun could silently narrow or widen the rider, so it is rejected. */
function parseEntryRider(c: Cursor, lead: Lead): Change | null {
  return c.attempt((c): Change | null => {
    let subject: { kind: "it" } | { kind: "that"; type: string } | { kind: "this" };
    if (c.word("it") !== null) subject = { kind: "it" };
    else {
      const determiner = c.word("that", "this");
      const noun = c.anyWord();
      if (!determiner || !noun || !CARD_TYPES.has(noun.value)) return null;
      subject = determiner === "that" ? { kind: "that", type: noun.value } : { kind: "this" };
    }
    if (c.word("enters", "enter") === null) return null;
    const tapped = c.word("tapped") !== null;
    const counters = tapped ? null : parseEntryCounters(c);
    if (!tapped && !counters) return null;
    const appliesTo = lead.lead === "spell-cast" ? lead.appliesTo : undefined;
    const rider: PermissionRider = { rider: "entry", ...(appliesTo ? { appliesTo } : {}),
      ...(tapped ? { tapped: true as const } : {}), ...(counters ? { counters } : {}) };
    return (permission) => {
      const what = permission.what;
      if (subject.kind === "this" && what.ref !== "self" && what.ref !== "this") return null;
      if (subject.kind === "that") {
        const types = appliesTo?.types ?? ("filter" in what ? what.filter.types : undefined);
        if (!types?.includes(subject.type)) return null;
      }
      return { ...permission, riders: [...(permission.riders ?? []), rider] };
    };
  });
}

/** Properties that the one-sentence permission wording already expresses. */
function parseFoldedProperty(c: Cursor): Change | null {
  return c.attempt((c): Change | null => {
    const explicitActor = c.word("you") !== null;
    if (c.words("pay", "life", "equal", "to")) {
      // "its" / "that spell's" / "the spell's": the spell cast this way.
      if (c.word("its") === null && !(c.word("that", "the") !== null && c.possessive("spell") !== null)) return null;
      if (!c.words("mana", "value", "rather", "than") || c.word("pay", "paying") === null || !c.words("its", "mana", "cost")) return null;
      return permission => permission.alternativeCost ? null : { ...permission, alternativeCost: { cost: "pay", costs: [
        { cost: "pay-life", amount: { amount: "attribute", of: { ref: "it" }, attribute: "mana-value" } }] } };
    }
    if (c.words("cast", "it", "without", "paying", "its", "mana", "cost"))
      return permission => permission.alternativeCost ? null : { ...permission, alternativeCost: { cost: "without-mana-cost" } };
    if (!explicitActor || c.word("may") === null) return null;
    if (c.words("spend", "mana", "as", "though", "it", "were", "mana", "of", "any")) {
      const kind = c.word("color", "type");
      if (!kind || !c.words("to", "cast", "it")) return null;
      const manaPayment = kind === "color" ? "any-color" as const : "any-type" as const;
      return permission => permission.manaPayment ? null : { ...permission, manaPayment };
    }
    if (c.words("cast", "it", "as", "though", "it", "had", "flash"))
      return permission => permission.timing ? null : { ...permission, timing: "as-though-flash" };
    return null;
  });
}

/** CR 307.5a. "The controller of the permanent it becomes" is the controller
 * of `it` when the delayed ability resolves; `it` is scoped by the rider. */
function parseOffTimingRider(c: Cursor): Change | null {
  return c.attempt((c): Change | null => {
    if (!sentenceParser || !c.words("the", "controller", "of", "the", "permanent", "it", "becomes")) return null;
    const word = (value: string): Token => ({ kind: "word", value, raw: value, possessive: false });
    const body = new Cursor([word("its"), word("controller"), ...c.tokens.slice(c.pos)]);
    const effects = sentenceParser(body);
    if (!effects || effects.length !== 1 || !body.done()) return null;
    c.pos = c.tokens.length;
    const rider: PermissionRider = { rider: "cast-off-sorcery-timing", effects };
    return permission => permission.action === "cast" && (permission.what.ref === "self" || permission.what.ref === "this")
      ? { ...permission, riders: [...(permission.riders ?? []), rider] } : null;
  });
}

/** Parses one complete rider sentence and returns the permissions with it
 * applied. "If you cast a spell this way" concerns the casting permissions of
 * a coordinated group (lands are not cast); "If you do" needs one permission. */
export function parsePermissionRider(c: Cursor, permissions: CardPlayInstruction[], options: { allowIfYouDo: boolean }): CardPlayInstruction[] | null {
  return c.attempt((c): CardPlayInstruction[] | null => {
    const lead = parseLead(c);
    if (!lead) return null;
    // A static permission followed by "If you do, …" stays rejected: an
    // existing regression test pins that wording as unsupported there.
    if (lead.lead === "used" && !options.allowIfYouDo) return c.fail("represented static permission rider");
    const change = lead.lead === "off-sorcery-timing" ? parseOffTimingRider(c)
      : parseEntryRider(c, lead) ?? (lead.lead === "spell-cast" && !lead.appliesTo ? parseFoldedProperty(c) : null);
    if (!change) return c.fail("represented permission rider");
    if (!c.punct(".") && !c.done()) return null;
    let indexes = permissions.map((_, index) => index);
    if (lead.lead === "spell-cast") {
      const casting = indexes.filter(index => permissions[index].action === "cast");
      if (casting.length) indexes = casting;
    } else if (permissions.length !== 1) return c.fail("one permission for a rider");
    const result = [...permissions];
    for (const index of indexes) {
      const next = change(permissions[index]);
      if (!next) return c.fail("rider compatible with its permission");
      result[index] = next;
    }
    return result;
  });
}

/** Resolving form: the preceding step must consist of lasting permissions. */
export function parseGrantRider(c: Cursor, step: Effect): boolean {
  const grants: Extract<Effect, { effect: "grant-play-permission" }>[] = [];
  if (step.effect === "grant-play-permission") grants.push(step);
  else if (step.effect === "simultaneous") {
    for (const child of step.steps) {
      if (child.effect === "grant-play-permission") grants.push(child);
      else if (child.effect !== "grant-look-permission") return false;
    }
  }
  if (!grants.length) return false;
  const permissions = parsePermissionRider(c, grants.map(grant => grant.permission), { allowIfYouDo: true });
  if (!permissions) return false;
  grants.forEach((grant, index) => { grant.permission = permissions[index]; });
  return true;
}
