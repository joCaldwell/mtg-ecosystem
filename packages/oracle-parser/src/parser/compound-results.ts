import type { Effect, ObjectRef, PlayerRef, Sentence } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import { parseBattlefieldEntryDetails } from "./entry.ts";
import { isPluralPlayerRef, parseObjectRef } from "./refs.ts";
import { settleScopedContinuation } from "./for-each.ts";

// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt

/** The mill repeats on the same player only if both cards from that
 * iteration share a color (CR 105.2, 608.2c, 701.17a). */
export function foldMillRepetition(c: Cursor, previous: Sentence): boolean {
  const [mill] = previous.steps;
  if (previous.condition || previous.otherwise || previous.bindings || previous.steps.length !== 1 ||
    mill.effect !== "mill" || mill.optional || mill.forEach || mill.repeatWhile ||
    mill.who.player === "they" || mill.who.player === "your-team" || isPluralPlayerRef(mill.who) ||
    mill.amount.amount !== "fixed" || mill.amount.value !== 2) return false;
  const start = c.pos;
  const repeat = c.attempt(c => {
    if (!c.words("if", "two", "cards", "that", "share", "a", "color", "were", "milled", "this", "way")
      || !c.punct(",") || !c.words("repeat", "this", "process") || !c.punct(".") || !c.done()) return null;
    return true;
  });
  if (repeat) {
    mill.repeatWhile = { from: "last-iteration", condition: "two-cards-share-color" };
    c.fold(mill, start);
  }
  return Boolean(repeat);
}

type SentenceParser = (c: Cursor, ctx: { modalOptions: never[] }) => Sentence[] | null;

/** CR 603.7a–c, 614.1a, 712.21c, 730.3c: capture the card successors
 * of each replacement separately, including all melded or merged components.
 * https://magic.wizards.com/en/news/feature/kaldheim-release-notes-2021-01-22 */
export function parseExileReturnReplacement(c: Cursor, parse: SentenceParser): Sentence | null {
  return c.attempt(c => {
    if (!c.isWord("if")) return null;
    const period = c.tokens.findIndex((token, index) => index >= c.pos && token.kind === "punct" && token.value === ".");
    if (period < 0) return null;
    const tail = new Cursor(c.tokens.slice(period + 1));
    if (!tail.words("return", "it", "to", "the", "battlefield", "under", "its") ||
      tail.possessive("owner") === null || !tail.words("control", "at", "the", "beginning", "of", "the", "next", "end", "step") ||
      !tail.punct(".") || !tail.done()) return null;
    const head = new Cursor(c.tokens.slice(c.pos, period + 1));
    const parsed = parse(head, { modalOptions: [] });
    const sentence = parsed?.[0];
    const effect = sentence?.steps[0];
    if (!sentence || parsed?.length !== 1 || !head.done() || sentence.steps.length !== 1 || sentence.condition ||
      effect?.effect !== "create-replacement" || effect.duration?.duration !== "this-turn") return null;
    const { replacement } = effect;
    if (replacement.optional || replacement.event.event !== "move-zone" || replacement.event.from === "anywhere" ||
      replacement.event.from?.zone !== "battlefield" || replacement.event.to?.zone !== "graveyard" ||
      replacement.replace.replacement !== "effects" || replacement.replace.effects.length !== 1) return null;
    const [body] = replacement.replace.effects;
    const [exile] = body.steps;
    if (body.condition || body.steps.length !== 1 || exile.effect !== "exile" || exile.optional ||
      exile.what.ref !== "replacement-event-object" || exile.what.role !== "affected") return null;
    const delayed: Effect = { effect: "create-delayed-trigger", trigger: { trigger: "phase", phase: "end-step", whose: "each-player" },
      occurrence: "next", nextOnly: true, targets: "enclosing-ability", effects: [{ steps: [{
        effect: "move-zone",
        what: { ref: "related", filter: { cls: "card" }, relation: { relation: "replacement-exile-results" }, number: "plural" },
        to: { zone: "battlefield" }, controller: "owner",
      }] }] };
    body.steps.push(delayed);
    c.pos = c.tokens.length;
    return sentence;
  });
}

/** CR 400.7j, 609.3: the two independent results move together whenever
 * present. A failed exile or failed search does not suppress the other.
 * https://magic.wizards.com/en/news/feature/modern-horizons-2-release-notes-2021-06-04 */
export function parseExileAndSearchPlacement(c: Cursor, previous: Sentence,
  parseShuffle: (c: Cursor, ctx: { modalOptions: never[] }) => Effect[] | null): Sentence | null {
  return c.attempt(c => {
    if (!c.words("put", "both", "cards", "onto", "the", "battlefield") ||
        previous.condition || previous.otherwise || previous.bindings || previous.steps.length !== 2) return null;
    const [exile, search] = previous.steps;
    if (exile.effect !== "exile" || exile.what.ref !== "target" || !exile.what.upTo ||
      exile.what.count.amount !== "fixed" || exile.what.count.value !== 1 || exile.optional || exile.forEach ||
      search.effect !== "search" || search.optional || search.forEach || search.who.player !== "you" ||
      search.for.ref !== "filter" || search.for.filter.cls !== "card" || search.for.count.amount !== "fixed" ||
      search.for.count.value !== 1 || search.zones.selection !== "all" || search.zones.of.length !== 1 ||
      search.zones.of[0].zone !== "library" || search.zones.of[0].owner !== "your") return null;
    const details = parseBattlefieldEntryDetails(c);
    if (!details || details.from || !c.punct(",") || c.word("then") === null) return null;
    const shuffle = parseShuffle(c, { modalOptions: [] });
    if (!shuffle || shuffle.length !== 1 || shuffle[0].effect !== "shuffle" || shuffle[0].who.player !== "you" ||
      !c.punct(".") || !c.done()) return null;
    return { steps: [{ effect: "move-zone", what: { ref: "related", filter: { cls: "card" }, number: "plural",
        relation: { relation: "exile-and-search-results", from: "preceding-sentence" } },
      to: { zone: "battlefield" }, ...details }, ...shuffle] };
  });
}

/** CR 110.4, 205.2b, 608.2d–e: choose from each opponent's own permanents,
 * allowing reuse across types. Once the choices are complete that
 * opponent sacrifices the complement of their own choices (CR 701.21a). */
export function parsePermanentRepresentatives(c: Cursor): Sentence | null {
  const listed = c.attempt(c => {
    const start = c.pos;
    if (!c.words("each", "player")) return null;
    const who: PlayerRef = c.mark({ player: "each-player" }, start);
    if (c.word("chooses") === null) return null;
    const listStart = c.pos;
    const types: string[] = [];
    const readType = () => c.word("a", "an") !== null
      ? c.word("artifact", "battle", "creature", "enchantment", "planeswalker") : null;
    let last = false;
    while (!last) {
      const type = readType();
      if (!type || types.includes(type)) return null;
      types.push(type);
      const comma = c.punct(",");
      last = c.word("and") !== null;
      if (!comma && !last) return null;
    }
    const finalType = readType();
    if (!finalType || types.includes(finalType)) return null;
    types.push(finalType);
    const perPermanentType = c.mark({ types, skipMissing: true, sameObjectForMultipleTypes: true } as const, listStart);
    if (!c.words("from", "among", "the")) return null;
    const poolStart = c.pos;
    if (!c.words("nonland", "permanents")) return null;
    const controlStart = c.pos;
    if (!c.words("they", "control")) return null;
    const controlledBy = c.mark({ player: "they" } as const, controlStart, controlStart + 1);
    const pool: ObjectRef = c.mark({
      ref: "each", filter: c.mark({ cls: "permanent", nonTypes: ["land"], controlledBy } as const, poolStart),
    }, poolStart);
    const choose: Effect = c.mark({ effect: "choose-object", who, what: pool, perPermanentType }, start);
    const restStart = c.pos;
    if (!c.punct(",") || !c.words("then", "sacrifices")) return null;
    const whatStart = c.pos;
    if (!c.words("the", "rest")) return null;
    const rest: ObjectRef = c.mark({ ref: "rest", of: c.alias(pool), exclude: "own-preceding-choices" }, whatStart);
    const sacrifice: Effect = c.mark({ effect: "sacrifice", who: c.alias(who), what: rest }, restStart);
    if (!c.punct(".") || !c.done()) return null;
    return { steps: [choose, sacrifice] };
  });
  if (listed) return listed;
  return c.attempt(c => {
    if (!c.words("each", "opponent", "chooses", "a", "permanent", "they", "control", "of", "each", "permanent", "type")
      || !c.words("and", "sacrifices", "the", "rest") || !c.punct(".") || !c.done()) return null;
    const who: PlayerRef = { player: "each-opponent" };
    const pool: ObjectRef = { ref: "each", filter: { cls: "permanent", controlledBy: { player: "they" } } };
    return { steps: [
      { effect: "choose-object", who, what: pool, perPermanentType: { skipMissing: true, sameObjectForMultipleTypes: true } },
      { effect: "sacrifice", who, what: { ref: "rest", of: pool, exclude: "own-preceding-choices" } },
    ] };
  });
}

/** CR 608.2d–e, 701.14: select distinct opposing creatures for the
 * tokens made by the immediately preceding loop, then fight simultaneously.
 * https://magic.wizards.com/en/news/feature/commander-2015-edition-release-notes-2015-11-10 */
export function parseDistinctFights(c: Cursor, previous: Sentence): Sentence | null {
  return c.attempt(c => {
    const [loop] = previous.steps;
    if (previous.condition || previous.otherwise || previous.bindings || previous.steps.length !== 1 ||
      loop.effect !== "for-each" || !("ref" in loop.each) || loop.each.ref !== "each" ||
      loop.each.filter.types?.length !== 1 || loop.each.filter.types[0] !== "creature" ||
      loop.each.filter.control !== "opponent" || loop.effects.length !== 1) return null;
    const [body] = loop.effects;
    const [create] = body.steps;
    if (body.condition || body.otherwise || body.bindings || body.steps.length !== 1 ||
      create.effect !== "create-token" || create.optional || create.forEach || create.who ||
      create.count.amount !== "fixed" || create.count.value !== 1 || !("types" in create.token)
      || !create.token.types.includes("creature")) return null;
    if (!c.words("each", "of", "those", "tokens", "fights", "a", "different", "one", "of", "those", "creatures")
      || !c.punct(".") || !c.done()) return null;
    return { steps: [{ effect: "fight", a: { ref: "that", plural: true, filter: { cls: "token" } },
      b: { ref: "that", plural: true, filter: { types: ["creature"] } },
      pairing: { kind: "distinct-pairs", who: { player: "you" }, inputs: "preceding-token-loop", unmatched: "omit" },
    }] };
  });
}

/** CR 122.1, 608.2d: the counter kind is taken from counters on the
 * currently bound permanent. The optional addition is to that permanent. */
export function parsePerObjectCounterKind(c: Cursor): Effect | null {
  return c.attempt(c => {
    if (c.word("for") === null || !c.isWord("each")) return null;
    const each = parseObjectRef(c);
    if (each?.ref !== "each" || each.filter.cls !== "permanent" || !c.punct(",")
      || !c.words("choose", "a", "counter", "on", "it") || !c.punct(".")
      || !c.words("you", "may", "put", "an", "additional", "counter", "of", "that", "kind", "on", "that", "permanent")
      || !c.isPunct(".") || c.peek(1)) return null;
    const loop: Extract<Effect, { effect: "for-each" }> = { effect: "for-each", each, processing: "instruction-wise", effects: [{ steps: [{
      effect: "put-counters-of", counters: { of: { ref: "it" }, select: "one-existing-counter-kind", placement: "optional", phaseOrder: "choose-all-then-place" },
      on: { ref: "it" },
    }] }] };
    settleScopedContinuation(loop);
    return loop;
  });
}

/** CR 400.3, 607.2a: each linked exiled card is moved by its own owner to
 * that owner's library. The unit of repetition is the card. */
export function parseLinkedOwnersBottomPlacement(c: Cursor): Effect | null {
  return c.attempt(c => {
    if (!c.words("the", "owner", "of") || !c.isWord("each")) return null;
    const each = parseObjectRef(c);
    if (each?.ref !== "each" || each.filter.cls !== "card" || each.filter.relation?.relation !== "linked-exile"
      || !c.words("puts", "that", "card", "on", "the", "bottom", "of", "their", "library") || !c.isPunct(".")) return null;
    const loop: Extract<Effect, { effect: "for-each" }> = { effect: "for-each", each, processing: "instruction-wise", effects: [{ steps: [{
      effect: "move-zone", who: { player: "owner", of: { ref: "it" } }, what: { ref: "it" },
      to: { zone: "library", owner: "their", position: "bottom" },
    }] }] };
    if (c.peek(1)) return null;
    settleScopedContinuation(loop);
    return loop;
  });
}

/** CR 608.2h, 701.23h: keep the LKI controller of each exiled creature,
 * combine searches by that player, then each moves their found cards and
 * shuffles their own library once.
 * https://magic.wizards.com/en/news/feature/modern-horizons-release-notes-2019-05-31 */
export function parseGroupedSearchPlacement(c: Cursor, previous: Sentence, source: Sentence | undefined): Sentence | null {
  return c.attempt(c => {
    if (!source || source.condition || source.otherwise || source.bindings || source.steps.length !== 1 ||
      source.steps[0].effect !== "exile" || source.steps[0].optional || source.steps[0].forEach) return null;
    const [loop] = previous.steps;
    if (previous.condition || previous.otherwise || previous.bindings || previous.steps.length !== 1 ||
      loop.effect !== "for-each" || !("ref" in loop.each) || loop.each.ref !== "each" ||
      loop.each.filter.types?.length !== 1 || loop.each.filter.types[0] !== "creature" ||
      loop.each.filter.relation?.relation !== "event-result" || loop.each.filter.relation.event !== "exile" ||
      loop.each.filter.relation.scope !== "this-way" || loop.effects.length !== 1) return null;
    const [body] = loop.effects;
    const [search] = body.steps;
    if (body.condition || body.otherwise || body.bindings || body.steps.length !== 1 ||
      search.effect !== "search" || search.optional || search.forEach || search.who.player !== "controller" ||
      search.who.of.ref !== "it" || search.zones.selection !== "all" || search.zones.of.length !== 1 ||
      search.zones.of[0].zone !== "library" || search.zones.of[0].owner !== "their") return null;
    if (!c.words("those", "players", "put", "those", "cards", "onto", "the", "battlefield", "tapped") ||
      !c.punct(",") || !c.words("then", "shuffle") || !c.punct(".") || !c.done()) return null;
    const who: PlayerRef = { player: "those-players" };
    loop.groupedSearch = "last-controllers-of-exiled-creatures";
    c.hull(loop);
    settleScopedContinuation(loop);
    return { steps: [{
      effect: "move-zone", what: { ref: "related", number: "plural", filter: { cls: "card" },
        relation: { relation: "grouped-search-results", from: "preceding-search-loop", by: "searching-player" } },
      to: { zone: "battlefield" }, who, tapped: true,
    }, { effect: "shuffle", who, library: { zone: "library", owner: who }, coalesceEarlierLibrarySearches: true }] };
  });
}

/** CR 101.4, 400.3, 608.2e–h, 701.20: each player retains their own
 * revealed pool. Each placement is a separate simultaneous action.
 * https://gatherer.wizards.com/Pages/Card/Details.aspx?multiverseid=190187&printed=false */
export function parseGroupedRevealPlacements(c: Cursor, previous: Sentence): Sentence | null {
  return c.attempt(c => {
    if (!c.words("each", "player", "puts", "all", "artifact") || !c.punct(",") || !c.word("creature") ||
      !c.punct(",") || !c.words("and", "land", "cards", "revealed", "this", "way", "onto", "the", "battlefield") ||
      !c.punct(",") || !c.words("then", "does", "the", "same", "for", "enchantment", "cards") ||
      !c.punct(",") || !c.words("then", "puts", "all", "cards", "revealed", "this", "way", "that", "weren't", "put",
        "onto", "the", "battlefield", "on", "the", "bottom", "of", "their", "library") ||
      !c.punct(".") || !c.done()) return null;
    const [shuffle, reveal] = previous.steps;
    if (previous.condition || previous.otherwise || previous.bindings || previous.steps.length !== 2 ||
      shuffle.effect !== "shuffle-into-library" || shuffle.who.player !== "each-player" || shuffle.optional ||
      shuffle.library.zone !== "library" || shuffle.library.owner !== "their" ||
      shuffle.what.length !== 1 || shuffle.what[0].ref !== "all" ||
      shuffle.what[0].filter.cls !== "permanent" || shuffle.what[0].filter.ownedBy?.player !== "they" ||
      Object.keys(shuffle.what[0].filter).length !== 2 ||
      reveal.effect !== "reveal" || reveal.who.player !== "each-player" || reveal.optional ||
      reveal.what.ref !== "filter" || reveal.what.count.amount !== "that-much" || reveal.what.filter.cls !== "card" ||
      reveal.what.filter.zone?.zone !== "library" || reveal.what.filter.zone.owner !== "their" ||
      reveal.what.filter.zone.position !== "top" || Object.keys(reveal.what.filter).length !== 2) return null;
    const who: PlayerRef = { player: "each-player" };
    const relation = { relation: "grouped-reveal-results", by: "revealing-player" } as const;
    const remaining = { ...relation, afterPlacement: "exclude-successful-battlefield-moves" } as const;
    reveal.what.count = { amount: "permanents-moved-by-shuffle", scope: "for-each-shuffling-player" };
    return { steps: [
      { effect: "move-zone", who, what: { ref: "all", filter: {
        cls: "card", anyOf: ["artifact", "creature", "land"].map(type => ({ types: [type] })), relation,
      } }, to: { zone: "battlefield" } },
      { effect: "move-zone", who, what: { ref: "all", filter: { cls: "card", types: ["enchantment"], relation: remaining } },
        to: { zone: "battlefield" } },
      { effect: "move-zone", who, what: { ref: "all", filter: { cls: "card", relation: remaining } },
        to: { zone: "library", owner: "their", position: "bottom" } },
    ] };
  });
}
