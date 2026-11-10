// Trigger event productions, called from triggers.ts after the
// When/Whenever opener. Each production consumes one complete event phrase or
// leaves the cursor untouched; no word is skipped.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Comparison, ObjectFilter, ObjectRef, PlayerRef, Trigger, ZoneRef } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { isPluralPlayerRef, parseComparison, parseFilter, parseObjectRef, parsePlayerRef, parseZoneRef } from "./refs.ts";
import { parseStickerKind } from "./stickers.ts";

function hasTarget(value: unknown): boolean {
  if (!value || typeof value !== "object") return false;
  if ("ref" in value && (value.ref === "target" || value.ref === "any-target")) return true;
  if ("player" in value && (value.player === "target-player" || value.player === "target-opponent")) return true;
  return Object.values(value).some(hasTarget);
}

/** A description of spells and/or abilities, the only objects whose effects
 * instruct players (CR 609.1). */
function isSpellOrAbility(filter: ObjectFilter): boolean {
  if (filter.zone) return false;
  if (filter.cls === "spell" || filter.cls === "ability") return true;
  return !!filter.allOf?.some(clause => !!clause.anyOf?.length
    && clause.anyOf.every(option => option.cls === "spell" || option.cls === "ability"));
}

/** "a spell or ability an opponent controls causes you to discard this
 * card": the player event that follows, restricted to occurrences an effect
 * of a matching spell or ability brought about (CR 609.1). The infinitive is
 * re-read as the ordinary event "<player> <verb> …" so both wordings share
 * one node; only events with a `causedBy`-compatible meaning are accepted. */
export function parseCausedEvent(c: Cursor, parseEvent: (c: Cursor) => Trigger | null): Trigger | null {
  return c.attempt((c): Trigger | null => {
    const opener = c.tokens[c.pos - 1];
    if (!opener || c.word("a", "an") === null) return null;
    const cause = parseFilter(c);
    if (!cause || !isSpellOrAbility(cause) || hasTarget(cause)) return null;
    // "a spell or ability you control counters a spell" (CR 701.6a).
    if (c.word("counters") !== null) {
      const countered = parseEventObject(c);
      return countered && !countered.grouping && isSpellOrAbility(countered.what.filter)
        ? { trigger: "counters", by: cause, what: countered.what } : null;
    }
    // "a spell or ability you control exiles one or more permanents from the
    // battlefield": those permanents are put into exile by that cause.
    if (c.word("exiles") !== null) {
      const object = parseEventObject(c);
      if (!object) return null;
      // The noun phrase carries its "from <zone>" as the filter's zone: the
      // origin of the move, not a restriction on where the objects are now.
      const { zone: from, ...filter } = object.what.filter;
      if (!from) return null;
      return { trigger: "put-into-zone", what: { ...object.what, filter }, from, to: { zone: "exile" }, causedBy: cause,
        ...(object.grouping ? { grouping: object.grouping } : {}) };
    }
    if (c.word("causes") === null) return null;
    const playerStart = c.pos;
    const who = parsePlayerRef(c);
    if (!who || hasTarget(who)) return null;
    const player = c.tokens.slice(playerStart, c.pos);
    if (c.word("to") === null) return null;
    // "to discard cards" is one collective occurrence ("those cards"), which
    // the per-card discard event does not state (CR 603.2c); it stays unparsed.
    const object = c.peek(1);
    if (c.isWord("discard") && !(object?.kind === "selfref" || object?.kind === "word" && !object.possessive
      && ["a", "an", "this", "it", "another", "one"].includes(object.value))) return c.fail("singular discarded card");
    const clause = new Cursor([opener, ...player, ...c.tokens.slice(c.pos)]);
    const event = parseEvent(clause);
    if (!event || clause.pos <= 1 + player.length) return null;
    if (event.trigger !== "discards" && event.trigger !== "gains-life" && event.trigger !== "loses-life" && event.trigger !== "shuffles-library") return null;
    if (JSON.stringify(event.who) !== JSON.stringify(who) || event.occurrence || event.during) return null;
    c.pos += clause.pos - 1 - player.length;
    return { ...event, causedBy: cause };
  });
}

/** "3 or more" directly before "damage": a threshold on the damage dealt in
 * one event. The word "damage" itself is left for the caller. */
export function parseDamageAmount(c: Cursor): Comparison | null {
  return c.attempt((c): Comparison | null => {
    const amount = parseComparison(c);
    return amount && c.isWord("damage") ? amount : null;
  });
}

/** "is dealt excess [noncombat] damage" / "is dealt 3 or more damage", after
 * "is"/"are" and the subject. CR 120.10 defines excess damage. */
export function parseObjectDealtDamage(c: Cursor, what: ObjectRef): Trigger | null {
  return c.attempt((c): Trigger | null => {
    if (c.word("is", "are") === null || c.word("dealt") === null) return null;
    const excess = c.word("excess") !== null;
    const amount = excess ? null : parseDamageAmount(c);
    if (!excess && !amount) return null;
    const kind = c.word("combat", "noncombat");
    if (c.word("damage") === null) return null;
    return { trigger: "is-dealt-damage", what, ...(kind ? { combat: kind === "combat" } : {}),
      ...(excess ? { excess: true as const } : {}), ...(amount ? { amount } : {}) };
  });
}

function isYou(who: PlayerRef): boolean { return who.player === "you"; }

/** One player other than "you", the antecedent of a following "their". */
export function isSingleOtherPlayer(who: PlayerRef): boolean {
  if (who.player === "filtered") return isSingleOtherPlayer(who.base);
  return who.player === "an-opponent" || who.player === "a-player" || who.player === "enchanted-player"
    || who.player === "that-player";
}

/** "the player with the most life or tied for most life": the one player
 * whose life total equals the highest life total among all players (CR
 * 119.1); a tie leaves several such players, so the event names whichever of
 * them is attacked. Without "or tied" the phrase would name no one in a tie. */
export function parseMostLifePlayer(c: Cursor): PlayerRef | null {
  return c.attempt((c): PlayerRef | null => {
    if (!c.words("the", "player", "with", "the", "most", "life", "or", "tied", "for", "most", "life")) return null;
    return { player: "filtered", base: { player: "a-player" },
      filter: { lifeTotal: { op: "eq", value: { amount: "life-total-extremum", extremum: "maximum", among: { player: "each-player" } } } } };
  });
}

/** "this enchantment's" / "~'s": the possessor is the source itself. */
function parseOwnPossessive(c: Cursor): ObjectRef | null {
  return c.attempt((c): ObjectRef | null => {
    if (c.selfrefPossessive()) return { ref: "self" };
    if (c.word("this") === null) return null;
    const noun = c.peek();
    if (noun?.kind !== "word" || !noun.possessive || !["creature", "enchantment", "artifact", "permanent", "land", "aura"].includes(noun.value)) return null;
    c.pos++;
    return { ref: "this", noun: noun.value };
  });
}

/** "cumulative upkeep" / "echo cost" of the source itself. CR 702.24a, 702.30a. */
function parseOwnUpkeepCost(c: Cursor): { what: ObjectRef; keyword: "cumulative-upkeep" | "echo" } | null {
  return c.attempt(c => {
    const what = parseOwnPossessive(c);
    if (!what) return null;
    if (c.words("cumulative", "upkeep")) return { what, keyword: "cumulative-upkeep" as const };
    return c.words("echo", "cost") ? { what, keyword: "echo" as const } : null;
  });
}

/** The player of a trigger event. In a trigger "another player" is a player
 * other than the ability's controller, the same player set as "a player other
 * than you" (CR 109.5: "you" is the controller). */
export function parseTriggerPlayer(c: Cursor): PlayerRef | null {
  return c.attempt((c): PlayerRef | null => {
    if (c.words("another", "player"))
      return { player: "filtered", base: { player: "a-player" }, filter: { otherThan: { player: "you" } } };
    return parsePlayerRef(c);
  });
}

type FilterRef = Extract<ObjectRef, { ref: "filter" }>;

/** A zone without explicit-undefined keys, so equal moves compare equal. */
function parseEventZone(c: Cursor): ZoneRef | null {
  const zone = parseZoneRef(c);
  return zone && Object.fromEntries(Object.entries(zone).filter(([, value]) => value !== undefined)) as ZoneRef;
}

/** "a token", "one or more nonland cards": the event's object description. */
function parseEventObject(c: Cursor): { what: FilterRef; grouping?: "one-or-more" } | null {
  return c.attempt(c => {
    const grouped = c.words("one", "or", "more");
    if (!grouped && c.word("a", "an") === null) return null;
    const filter = parseFilter(c);
    if (!filter || hasTarget(filter)) return null;
    const what: FilterRef = { ref: "filter", filter, count: { amount: "fixed", value: 1 } };
    return grouped ? { what, grouping: "one-or-more" as const } : { what };
  });
}

/** Keyword-action and game events whose subject is the acting player, after
 * that player was parsed. CR 104.3, 309.7, 701.23, 701.24, 701.30, 701.34,
 * 701.43, 705.2, 725.3. */
export function parsePlayerKeywordEvent(c: Cursor, who: PlayerRef): Trigger | null {
  return c.attempt((c): Trigger | null => {
    if (hasTarget(who) || isPluralPlayerRef(who)) return null;
    if (c.word("proliferate", "proliferates") !== null) return { trigger: "proliferates", who };
    // "a player puts a Swamp onto the battlefield" (the one performing the put).
    const put = c.attempt((c): Trigger | null => {
      if (c.word("put", "puts") === null) return null;
      const object = parseEventObject(c);
      return object && c.words("onto", "the", "battlefield") ? { trigger: "puts-onto-battlefield", who, ...object } : null;
    });
    if (put) return put;
    if (c.word("investigate", "investigates") !== null) return { trigger: "investigates", who };
    if (c.word("forage", "forages") !== null) return { trigger: "forages", who };
    if (c.word("collect", "collects") !== null) return c.word("evidence") !== null ? { trigger: "collects-evidence", who } : null;
    if (c.word("tap", "taps") !== null) {
      // "tap … for mana" is the taps-for-mana production's event.
      const object = parseEventObject(c);
      return object && !c.isWord("for") ? { trigger: "taps", who, ...object } : null;
    }
    if (c.word("sacrifice", "sacrifices") !== null) {
      // "sacrifice ~ or another creature": the source or any other match.
      const first = c.attempt(c => {
        if (!c.isWord("this") && c.peek()?.kind !== "selfref") return null;
        const specific = parseObjectRef(c);
        return specific && (specific.ref === "this" || specific.ref === "self") ? specific : null;
      });
      if (!first || c.word("or") === null || !c.isWord("another")) return null;
      const other = parseFilter(c);
      return other && !hasTarget(other) ? { trigger: "any-of", events: [
        { trigger: "sacrifices", who, what: first }, { trigger: "sacrifices", who, what: other }] } : null;
    }
    // CR 702.24a, 702.30a: paying (or declining to pay) the source's own upkeep cost.
    const payment = c.attempt((c): Trigger | null => {
      const declined = c.word("doesn't", "don't") !== null;
      if (c.word("pay", "pays") === null) return null;
      const cost = parseOwnUpkeepCost(c);
      return cost && { trigger: "pays-upkeep-cost", keyword: cost.keyword, what: cost.what, who, paid: !declined };
    });
    if (payment) return payment;
    // CR 119.4. Only the bare "pay life" is the event; a quantity is not.
    if (c.word("pay", "pays") !== null) return c.word("life") !== null ? { trigger: "pays-life", who } : null;
    // CR 702.143c, 701.57b, 701.62b, 701.67c, 701.51c, 701.52a, 702.174c, 719.3a, 701.54a.
    const action = c.attempt((c): Trigger | null => {
      if (c.word("foretell", "foretells") !== null) return c.words("a", "card") ? { trigger: "foretells", who } : null;
      if (c.word("discover", "discovers") !== null) return { trigger: "discovers", who };
      if (c.word("manifest", "manifests") !== null) return c.words("dread") ? { trigger: "manifests-dread", who } : null;
      if (c.word("waterbend", "waterbends") !== null) return { trigger: "waterbends", who };
      if (c.word("open", "opens") !== null) return c.words("an", "attraction") ? { trigger: "opens-attraction", who } : null;
      if (c.word("visit", "visits") !== null) return c.words("an", "attraction") ? { trigger: "visits-attraction", who } : null;
      if (c.word("give", "gives") !== null) return c.words("a", "gift") ? { trigger: "gives-gift", who } : null;
      if (c.word("put", "puts") !== null) {
        const sticker = parseStickerKind(c);
        if (!sticker || c.word("on") === null) return null;
        const on = parseEventObject(c)?.what ?? c.attempt(c => {
          const ref = parseObjectRef(c);
          return ref && (ref.ref === "this" || ref.ref === "self") ? ref : null;
        });
        return on ? { trigger: "puts-sticker", who, sticker, on } : null;
      }
      if (c.word("solve", "solves") !== null) return c.words("a", "case") ? { trigger: "solves-case", who } : null;
      if (c.word("choose", "chooses") !== null) return c.words("a", "creature", "as") && c.word("your", "their") !== null && c.word("ring-bearer") !== null
        ? { trigger: "chooses-ring-bearer", who } : null;
      if (c.word("turn", "turns") !== null) {
        if (c.word("a", "an") === null) return null;
        const filter = parseFilter(c);
        return filter && !hasTarget(filter) && c.words("face", "up")
          ? { trigger: "turns-face-up", who, what: { ref: "filter", filter, count: { amount: "fixed", value: 1 } } } : null;
      }
      return null;
    });
    if (action) return action;
    // CR 508.3b: once per declaration of attackers that attacks this player.
    if (c.words("is", "attacked")) return { trigger: "is-attacked", who };
    if (c.word("complete", "completes") !== null) return c.words("a", "dungeon") ? { trigger: "completes-dungeon", who } : null;
    if (c.word("become", "becomes") !== null) {
      if (c.words("the", "monarch")) return { trigger: "becomes-monarch", who };
      const of = c.words("the", "target", "of") && c.word("a", "an") !== null ? parseFilter(c) : null;
      return of && !hasTarget(of) ? { trigger: "player-becomes-target", who, of } : null;
    }
    const library = c.word("search", "searches", "shuffle", "shuffles");
    if (library !== null) {
      // Only the acting player's own library: "you … your", "an opponent … their".
      // A single controller ("its controller shuffles their library") likewise owns "their".
      if (!(isYou(who) ? c.word("your") !== null : (isSingleOtherPlayer(who) || who.player === "controller") && c.word("their") !== null)) return null;
      if (c.word("library") === null) return null;
      return { trigger: library.startsWith("search") ? "searches-library" : "shuffles-library", who };
    }
    if (c.word("clash", "clashes") !== null)
      return c.words("and", "win") || c.words("and", "wins") ? { trigger: "clashes", who, won: true } : { trigger: "clashes", who };
    // CR 603.10d: these look back in time at the object that changed hands.
    const controlEvent = c.attempt((c): Trigger | null => {
      const control = c.word("lose", "loses", "gain", "gains");
      if (control === null || !c.words("control", "of")) return null;
      const specific = c.attempt((c): ObjectRef | null => {
        if (!c.isWord("this") && c.peek()?.kind !== "selfref") return null;
        const what = parseObjectRef(c);
        return what && (what.ref === "this" || what.ref === "self") ? what : null;
      });
      const object = specific ? { what: specific } : parseEventObject(c);
      if (!object || "grouping" in object) return null;
      if (control.startsWith("lose")) return { trigger: "loses-control", who, what: object.what };
      // "from you" / "from another player": the previous controller.
      const from = c.attempt(c => c.word("from") !== null ? parseTriggerPlayer(c) : null);
      return { trigger: "gains-control", who, what: object.what, ...(from ? { from } : {}) };
    });
    if (controlEvent) return controlEvent;
    const outcome = c.word("win", "wins", "lose", "loses");
    if (outcome !== null) {
      const result = outcome.startsWith("win") ? "win" as const : "lose" as const;
      if (c.words("a", "coin", "flip")) return { trigger: "coin-flip", who, result };
      if (result === "win" && c.words("a", "clash")) return { trigger: "clashes", who, won: true };
      if (result === "lose" && c.words("the", "game")) return { trigger: "loses-game", who };
      return null;
    }
    if (c.word("get", "gets") !== null) {
      // CR 107.14: {E} is one energy counter, so getting {E} is energy
      // counters being put on that player (122.1) — the counter-change event.
      if (!c.words("one", "or", "more") || c.symbol() !== "E") return null;
      return { trigger: "counter-change", direction: "put", counter: { type: "energy" },
        on: { recipient: "player", who }, match: { counters: "one-or-more", recipients: "each" } };
    }
    const makes = c.word("create", "creates", "mill", "mills");
    if (makes !== null) {
      const alsoSacrifice = makes.startsWith("create") && c.words("or", "sacrifice");
      const object = parseEventObject(c);
      if (!object) return null;
      if (makes.startsWith("mill")) {
        if (object.what.filter.cls !== "card") return c.fail("milled card");
        return { trigger: "mills", who, ...object };
      }
      if (object.what.filter.cls !== "token") return c.fail("created token");
      const created: Trigger = { trigger: "creates-token", who, ...object };
      // "create or sacrifice a token" (Mirkwood Bats): two events, one description.
      return alsoSacrifice ? (object.grouping ? null : { trigger: "any-of", events: [
        created, { trigger: "sacrifices", who, what: object.what.filter }] }) : created;
    }
    if (c.word("exert", "exerts") !== null) {
      if (c.word("a", "an") === null) return null;
      const filter = parseFilter(c);
      return filter && !hasTarget(filter)
        ? { trigger: "exerts", who, what: { ref: "filter", filter, count: { amount: "fixed", value: 1 } } } : null;
    }
    return null;
  });
}

/** Events with a fixed game-object subject. CR 701.54d: "the Ring tempts
 * you" happens once the tempted player has completed the 701.54a actions. */
export function parseGameEvent(c: Cursor): Trigger | null {
  return c.attempt((c): Trigger | null => {
    // CR 714.2e: "the final chapter ability of a Saga you control triggers / resolves".
    const chapter = c.attempt((c): Trigger | null => {
      if (!c.words("the", "final", "chapter", "ability", "of") || c.word("a", "an") === null) return null;
      const filter = parseFilter(c);
      if (!filter || !filter.subtypes?.includes("Saga") || hasTarget(filter)) return null;
      const event = c.word("triggers", "resolves");
      return event !== null ? { trigger: "final-chapter-ability", of: { ref: "filter", filter, count: { amount: "fixed", value: 1 } },
        event: event as "triggers" | "resolves" } : null;
    });
    if (chapter) return chapter;
    // "the first noncreature spell of a turn is cast": the Nth matching spell
    // cast in the turn by any player (no `per`: all players' spells count).
    const nth = c.attempt((c): Trigger | null => {
      if (c.word("the") === null) return null;
      const ordinals = ["first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth"];
      const word = c.word(...ordinals);
      if (word === null) return null;
      let end = c.pos;
      for (; end < c.tokens.length; end++) {
        const token = c.tokens[end];
        if (token.kind === "word" && token.value === "of") break;
      }
      const bounded = new Cursor(c.tokens.slice(0, end));
      bounded.pos = c.pos;
      const what = parseFilter(bounded);
      if (!what || what.cls !== "spell" || hasTarget(what) || bounded.pos !== end) return null;
      c.pos = end;
      return c.words("of", "a", "turn", "is", "cast")
        ? { trigger: "cast", who: { player: "a-player" }, what, occurrence: { ordinal: ordinals.indexOf(word) + 1, period: "turn" } } : null;
    });
    if (nth) return nth;
    const upkeep = c.attempt((c): Trigger | null => {
      const cost = parseOwnUpkeepCost(c);
      return cost && c.words("is", "paid") ? { trigger: "pays-upkeep-cost", keyword: cost.keyword, what: cost.what, paid: true } : null;
    });
    if (upkeep) return upkeep;
    if (c.words("the", "ring", "tempts", "you")) return { trigger: "ring-tempts", who: { player: "you" } };
    if (c.words("players", "finish", "voting")) return { trigger: "players-finish-voting" };
    // CR 731.1a. The usual pair "day becomes night or night becomes day" is
    // two alternative events joined by the caller.
    if (c.words("day", "becomes", "night")) return { trigger: "day-night-changes", to: "night" };
    if (c.words("night", "becomes", "day")) return { trigger: "day-night-changes", to: "day" };
    // "combat damage is dealt to you": the player-dealt-damage event in
    // the passive voice; a further recipient is a separate occurrence.
    const dealt = c.attempt((c): Trigger | null => {
      const kind = c.word("combat", "noncombat");
      if (!c.words("damage", "is", "dealt", "to")) return null;
      const who = parsePlayerRef(c);
      if (!who || hasTarget(who) || isPluralPlayerRef(who)) return null;
      const combat = kind === null ? {} : { combat: kind === "combat" };
      const player: Trigger = { trigger: "player-dealt-damage", who, ...combat };
      if (c.word("or") === null) return player;
      if (c.word("a", "an") === null) return null;
      const filter = parseFilter(c);
      return filter && !hasTarget(filter) ? { trigger: "any-of", events: [player,
        { trigger: "is-dealt-damage", what: { ref: "filter", filter, count: { amount: "fixed", value: 1 } }, ...combat }] } : null;
    });
    if (dealt) return dealt;
    return parsePlayerOrPermanentTargeted(c);
  });
}

/** "you or a permanent you control becomes the target of …": either the
 * player or one matching permanent becomes a target (CR 115.1); each such
 * occurrence is its own event, so the two subjects stay separate branches. */
function parsePlayerOrPermanentTargeted(c: Cursor): Trigger | null {
  return c.attempt((c): Trigger | null => {
    if (c.word("you") === null || c.word("or") === null) return null;
    if (c.word("a", "an") === null && !c.isWord("another")) return null;
    const filter = parseFilter(c);
    if (!filter || hasTarget(filter) || !c.words("becomes", "the", "target", "of") || c.word("a", "an") === null) return null;
    const of = parseFilter(c);
    if (!of || hasTarget(of)) return null;
    return { trigger: "any-of", events: [
      { trigger: "player-becomes-target", who: { player: "you" }, of },
      { trigger: "becomes-target", what: { ref: "filter", filter, count: { amount: "fixed", value: 1 } }, of },
    ] };
  });
}

/** "a creature", "this creature", "~": the other permanent of an attachment
 * event; a description stays a per-occurrence filter. */
function parseAttachmentPartner(c: Cursor): ObjectRef | null {
  return c.attempt((c): ObjectRef | null => {
    if (c.word("a", "an") !== null) {
      const filter = parseFilter(c);
      return filter && !hasTarget(filter) ? { ref: "filter", filter, count: { amount: "fixed", value: 1 } } : null;
    }
    if (!c.isWord("this") && c.peek()?.kind !== "selfref") return null;
    const partner = parseObjectRef(c);
    return partner && (partner.ref === "this" || partner.ref === "self") ? partner : null;
  });
}

/** Keyword-action events of a permanent, after its subject was parsed.
 * CR 701.14, 701.44b, 701.50, 702.26, 702.100b, 702.122b, 702.134c,
 * 702.149c, 702.154c, 702.171c. Plural verbs serve collective subjects. */
export function parseObjectKeywordEvent(c: Cursor, what: ObjectRef): Trigger | null {
  return c.attempt((c): Trigger | null => {
    if (c.word("phases", "phase") !== null) {
      const direction = c.word("in", "out");
      return direction === null ? null : { trigger: direction === "in" ? "phases-in" : "phases-out", what };
    }
    // CR 701.3a, 603.10c.
    if (c.words("becomes", "attached", "to") || c.words("become", "attached", "to")) {
      const to = parseAttachmentPartner(c);
      return to && { trigger: "becomes-attached", what, to };
    }
    if (c.words("becomes", "unattached", "from") || c.words("become", "unattached", "from")) {
      const from = parseAttachmentPartner(c);
      return from && { trigger: "becomes-unattached", what, from };
    }
    // "is exiled from the battlefield" / "is returned to its owner's hand":
    // the same moves as "is put into exile from …" / "is put into … hand".
    const passive = c.attempt(c => c.word("is", "are") !== null ? c.word("exiled", "returned") : null);
    if (passive === "exiled") {
      if (c.word("from") === null) return null;
      const from = parseEventZone(c);
      return from && { trigger: "put-into-zone", what, from, to: { zone: "exile" } };
    }
    if (passive === "returned") {
      if (c.word("to") === null) return null;
      const to = parseEventZone(c);
      if (!to || to.zone !== "hand" || !to.owner) return null;
      if (c.word("from") === null) return { trigger: "put-into-zone", what, to };
      const from = parseEventZone(c);
      return from && { trigger: "put-into-zone", what, from, to };
    }
    // "one or more nonland cards are milled": no milling player is named.
    if (c.words("is", "milled") || c.words("are", "milled"))
      return what.ref === "filter" && what.filter.cls === "card" ? { trigger: "mills", what } : null;
    const acts = c.word("crews", "saddles", "enlists", "mentors");
    if (acts !== null) {
      if (c.word("a", "an") === null) return null;
      const filter = parseFilter(c);
      if (!filter || hasTarget(filter)) return null;
      // CR 702.122a, 702.171a: crew is an ability of Vehicles; "saddles a
      // Mount" is the only printed saddle recipient.
      if (acts === "crews" && !filter.subtypes?.includes("Vehicle")) return c.fail("Vehicle");
      return { trigger: acts as "crews" | "saddles" | "enlists" | "mentors", what,
        recipient: { ref: "filter", filter, count: { amount: "fixed", value: 1 } } };
    }
    const single = c.word("explores", "connives", "fights", "evolves", "trains");
    if (single !== null) return { trigger: single as "explores" | "connives" | "fights" | "evolves" | "trains", what };
    const plural = c.word("explore", "connive", "fight", "evolve", "train");
    if (plural !== null) return { trigger: `${plural}s` as "explores" | "connives" | "fights" | "evolves" | "trains", what };
    return null;
  });
}

/** Index of the token starting "[during your turn] other than {your|the|their}
 * first …" at or after the cursor, so the event's filter can be read without
 * swallowing it; null when that tail is absent. */
export function findNotFirstTail(c: Cursor): number | null {
  const words = c.tokens.map(token => token.kind === "word" && !token.possessive ? token.value : null);
  for (let i = c.pos; i < c.tokens.length; i++) {
    if (words[i] === "other" && words[i + 1] === "than" && ["your", "the", "their"].includes(words[i + 2] ?? "") && words[i + 3] === "first")
      return words[i - 3] === "during" && words[i - 2] === "your" && words[i - 1] === "turn" ? i - 3 : i;
  }
  return null;
}

/** "other than your first spell each turn" / "other than the first instant
 * spell that player casts each turn" / "during your turn other than your
 * first spell that turn": every event except the first of its kind. The
 * restated description must be the event's own (CR 603.2). */
export function parseNotFirstTail(c: Cursor, who: PlayerRef, filter: ObjectFilter): Partial<Trigger> | null {
  return c.attempt((c): Partial<Trigger> | null => {
    const during = c.words("during", "your", "turn") ? "your" as const : undefined;
    if (during && !isYou(who)) return null;
    if (!c.words("other", "than")) return null;
    const own = c.word("your", "their", "the");
    if (own === null || c.word("first") === null) return null;
    // "your" counts only the controller's events; "the … that player casts" a single other player's.
    if (own === "your" && !isYou(who)) return null;
    // The restated description ends where the counting phrase begins.
    let end = c.pos;
    for (; end < c.tokens.length; end++) {
      const token = c.tokens[end];
      if (token.kind === "word" && ["that", "they", "each"].includes(token.value)) break;
    }
    const bounded = new Cursor(c.tokens.slice(0, end));
    bounded.pos = c.pos;
    const restated = parseFilter(bounded);
    if (!restated || bounded.pos !== end || JSON.stringify(restated) !== JSON.stringify(filter)) return null;
    c.pos = end;
    let per: "player" | undefined;
    if (own !== "your") {
      if (!isSingleOtherPlayer(who) || !(c.words("that", "player", "casts") || c.words("they", "cast"))) return null;
      per = "player";
    }
    if (!(c.words("each", "turn") || c.words("that", "turn") && during)) return null;
    return { notOccurrence: { ordinal: 1, period: "turn", ...(per ? { per } : {}) }, ...(during ? { during } : {}) };
  });
}
