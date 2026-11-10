// Replacement effects watch prospective events; the watched event does not
// happen when replaced. They never use trigger or ordinary-condition nodes.
// CR 614.1, 614.5–6, 614.11, 614.16, 616.1:
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type {
  Amount, Comparison, CounterSpec, Duration, Effect, ObjectFilter, ObjectRef,
  PlayerRef, ReplacementEvent, ReplacementInstruction, Sentence, StaticEffect,
} from "../ast.ts";
import type { Token } from "../lexer.ts";
import { COLORS } from "../vocab.ts";
import { Cursor } from "./cursor.ts";
import type { EffectContext } from "./effects.ts";
import { parseActionEvent, parseActionReplacement } from "./replacement-actions.ts";
import { parseAmount, parseCondition, parseCount, parseCounterSpec, parseDuration,
  parseFilter, parseObjectRef, parsePlayerRef, parseZoneRef } from "./refs.ts";

export type ReplacementBodyParser = (c: Cursor, ctx: EffectContext) => Sentence[] | null;
export interface ReplacementOptions { leadingDuration?: Duration }
export interface StaticReplacementOptions {
  /** The static replacement completes the cast permission printed before it. */
  afterCastPermission?: boolean;
}
const one: Amount = { amount: "fixed", value: 1 };
const you: PlayerRef = { player: "you" };
const eventQuantity: Amount = { amount: "replacement-event-quantity" };
const eventObject = (role: "affected" | "source" | "recipient"): ObjectRef => ({ ref: "replacement-event-object", role });
const eventPlayer = (role: "actor" | "recipient"): PlayerRef => ({ player: "replacement-event-player", role });

function same(a: unknown, b: unknown): boolean { return JSON.stringify(a) === JSON.stringify(b); }
function word(token: Token | undefined, value: string): boolean {
  return token?.kind === "word" && !token.possessive && token.value === value;
}

/** Quantity restrictions apply to the prospective event, not to the number
 * of objects a quantified noun happens to denote. */
function parseQuantity(c: Cursor): Comparison | null {
  return c.attempt(c => {
    const exactly = c.word("exactly") !== null;
    const value = parseCount(c);
    if (!value) return null;
    const op = c.words("or", "more") ? "ge" : "eq";
    if (exactly && op !== "eq") return null;
    return { op, value };
  });
}

function parseEligibleObject(c: Cursor): ObjectRef | null {
  return c.attempt(c => {
    const first = parseObjectRef(c);
    if (!first) return null;
    // Card and token are disjoint object classes (Rest in Peace). The shared
    // nominal parser deliberately does not guess conjunction of classes.
    if (first.ref === "filter" && first.filter.cls === "card" && c.words("or", "token")) {
      return { ...first, filter: { anyOf: [{ cls: "card" }, { cls: "token" }] } };
    }
    // "A spell cast this way": the cast instruction or permission printed
    // before this sentence (CR 608.2c), never every spell being cast.
    if (first.ref === "filter" && first.filter.cls === "spell" && !first.filter.relation && c.words("cast", "this", "way")) {
      return { ...first, filter: { ...first.filter, relation: { relation: "event-result", event: "cast", scope: "this-way" } } };
    }
    return first;
  });
}

function parseRecipients(c: Cursor): (ObjectRef | PlayerRef)[] | null {
  return c.attempt(c => {
    const refs: (ObjectRef | PlayerRef)[] = [];
    for (;;) {
      // "… or on yourself" repeats the preposition before the last recipient.
      if (refs.length) c.word("on");
      let ref: ObjectRef | PlayerRef | null = c.word("yourself") !== null ? you : parsePlayerRef(c) ?? parseEligibleObject(c);
      // A bare player after an explicit `or` is the same quantified match.
      if (!ref && refs.length && c.word("player") !== null) ref = { player: "a-player" };
      if (!ref) return null;
      refs.push(ref);
      if (c.word("or") === null) break;
    }
    return refs;
  });
}

function parseDamageSource(c: Cursor): ObjectRef | null {
  return c.attempt(c => {
    const source = c.attempt((c): ObjectRef | null => {
      const other = c.word("another") !== null;
      // "Any source" matches each source separately, as "a source" does.
      if (!other && c.word("a", "an", "any") === null) return null;
      const filter: ObjectFilter = {};
      if (other) filter.otherThan = { ref: "self" };
      while (c.peek()?.kind === "word") {
        const token = c.peek();
        if (token?.kind !== "word" || !COLORS.has(token.value)) break;
        (filter.colors ??= []).push(token.value as NonNullable<ObjectFilter["colors"]>[number]);
        c.next();
      }
      if (c.word("source") === null) return null;
      if (c.words("you", "control")) filter.control = "you";
      else if (c.words("an", "opponent", "controls")) filter.control = "opponent";
      // Source is a semantic role, not a permanent/spell type restriction.
      return { ref: "filter", filter, count: one };
    });
    return source ?? parseEligibleObject(c);
  });
}

function parseTokenFilter(c: Cursor): ObjectFilter | null {
  return c.attempt(c => {
    const start = c.pos;
    // The token noun is mandatory; a type/subtype alone is not creation.
    const filter = parseFilter(c);
    if (!filter || !c.tokens.slice(start, c.pos).some(token => word(token, "token") || word(token, "tokens"))) return null;
    return filter;
  });
}

function parseEvent(c: Cursor): ReplacementEvent | null {
  return c.attempt(c => {
    const mana = c.attempt((c): ReplacementEvent | null => {
      const by = c.attempt(c => { const who = parsePlayerRef(c); return who && c.word("tap", "taps") !== null ? who : null; });
      const what = parseEligibleObject(c);
      if (!what || (!by && !c.words("is", "tapped")) || c.word("for") === null) return null;
      const quantity = c.isWord("mana") ? null : parseQuantity(c);
      if ((!quantity && !c.isWord("mana")) || c.word("mana") === null) return null;
      return { event: "tap-for-mana", what, ...(by ? { by } : {}), ...(quantity ? { quantity } : {}) };
    });
    if (mana) return mana;
    const playerEvent = c.attempt((c): ReplacementEvent | null => {
      const who = parsePlayerRef(c);
      if (!who || c.word("would") === null) return null;
      const action = parseActionEvent(c, who);
      if (action) return action;
      const life = c.word("gain", "lose");
      if (life && c.word("life") !== null) return { event: life === "gain" ? "gain-life" : "lose-life", who };
      if (life === "lose" && c.words("the", "game")) return { event: "lose-game", who };
      if (life === "lose" && c.words("unspent", "mana")) return { event: "lose-unspent-mana", who };
      if (c.word("begin") !== null) {
        if (c.words("an", "extra", "turn")) return { event: "begin-time", who, what: { kind: "turn" }, extra: true };
        // The possessive names the same player whose turn or step it is.
        if (who.player !== "you" || c.word("your") === null) return null;
        if (c.word("turn") !== null) return { event: "begin-time", who, what: { kind: "turn" } };
        const step = c.word("untap", "upkeep", "draw");
        return step && c.word("step") !== null ? { event: "begin-time", who, what: { kind: "step", step: step as "untap" | "upkeep" | "draw" } } : null;
      }
      if (c.word("draw") !== null) {
        const quantity = parseQuantity(c);
        if (!quantity || c.word("card", "cards") === null) return null;
        const event: Extract<ReplacementEvent, { event: "draw" }> = { event: "draw", who, quantity };
        if (c.word("except") !== null) {
          if (!c.words("the", "first", "one")) return null;
          const subject = c.word("you", "they");
          if (!subject || (subject === "you") !== (who.player === "you") || !c.words("draw", "in", "each", "of")) return null;
          if (!c.word(subject === "you" ? "your" : "their") || !c.words("draw", "steps")) return null;
          event.except = "first-in-each-draw-step";
        }
        return event;
      }
      if (c.word("create") !== null) {
        const match = c.isWord("a", "an") ? "each-token" : "creation-event";
        const quantity = parseQuantity(c);
        const tokens = quantity && parseTokenFilter(c);
        return tokens && quantity && { event: "create-token", under: who, tokens, cause: "any", quantity, match };
      }
      if (c.word("put") !== null) {
        const quantity = parseQuantity(c);
        const counter = c.isWord("counter", "counters") ? "any" : parseCounterSpec(c);
        if (!quantity || !counter || c.word("counter", "counters") === null || c.word("on") === null) return null;
        const on = parseRecipients(c);
        return on && { event: "put-counters", by: who, on, counter, cause: "any", quantity };
      }
      if (c.word("get") !== null) {
        const quantity = parseQuantity(c);
        // CR 107.14: each {E} is one energy counter given to the player.
        if (quantity && c.attempt(c => c.symbol() === "E" ? true : null)) return { event: "put-counters", on: [who], counter: { type: "energy" }, cause: "any", quantity };
        const counter = c.isWord("counter", "counters") ? "any" : parseCounterSpec(c);
        if (!quantity || !counter || c.word("counter", "counters") === null) return null;
        return { event: "put-counters", on: [who], counter, cause: "any", quantity };
      }
      return null;
    });
    if (playerEvent) return playerEvent;

    const discard = c.attempt((c): ReplacementEvent | null => {
      const cause = parseObjectRef(c);
      if (!cause || c.word("causes") === null) return null;
      const who = parsePlayerRef(c);
      if (!who || !c.words("to", "discard")) return null;
      const what = parseEligibleObject(c);
      return what && { event: "discard", who, what, cause };
    });
    if (discard) return discard;

    const generated = c.attempt((c): ReplacementEvent | null => {
      let effect = false;
      if (c.words("an", "effect", "would")) effect = true;
      const verb = effect ? c.word("create", "put") : null;
      const match = c.isWord("a", "an") ? "each-token" : "creation-event";
      const quantity = parseQuantity(c);
      if (!quantity || (effect && !verb)) return null;
      if (verb === "create" || (!effect && !c.isWord("counter", "counters") && c.peek()?.kind !== "punct")) {
        const tokenEvent = c.attempt((c): ReplacementEvent | null => {
          const tokens = parseTokenFilter(c);
          if (!tokens || (!effect && !c.words("would", "be", "created"))) return null;
          const under = c.words("under", "your", "control") ? you : undefined;
          return { event: "create-token", tokens, cause: effect ? "effect" : "any", quantity, match, ...(under ? { under } : {}) };
        });
        if (tokenEvent) return tokenEvent;
      }
      if (effect && verb !== "put") return null;
      const counter = c.isWord("counter", "counters") ? "any" : parseCounterSpec(c);
      if (!counter || c.word("counter", "counters") === null) return null;
      if (!effect && !c.words("would", "be", "put")) return null;
      if (c.word("on") === null) return null;
      const on = parseRecipients(c);
      return on && { event: "put-counters", on, counter, cause: effect ? "effect" : "any", quantity };
    });
    if (generated) return generated;

    const damage = c.attempt((c): ReplacementEvent | null => {
      const passive = c.isWord("damage", "combat", "noncombat");
      const source = passive ? undefined : parseDamageSource(c);
      if (!passive && (!source || !c.words("would", "deal"))) return null;
      const quantity = passive ? undefined : parseQuantity(c) ?? undefined;
      const kind = (c.word("combat", "noncombat") ?? "all") as "all" | "combat" | "noncombat";
      if (c.word("damage") === null) return null;
      if (passive && !c.words("would", "be", "dealt")) return null;
      const event: Extract<ReplacementEvent, { event: "damage" }> = { event: "damage", kind,
        ...(source ? { source } : {}), ...(quantity ? { quantity } : {}) };
      if (c.word("to") !== null) {
        const to = parseRecipients(c);
        if (!to) return null;
        event.to = to;
      }
      return event;
    });
    if (damage) return damage;

    const what = parseEligibleObject(c);
    if (!what || c.word("would") === null) return null;
    if (c.word("die") !== null) return { event: "move-zone", what, from: { zone: "battlefield" }, to: { zone: "graveyard" } };
    if (c.words("leave", "the", "battlefield")) return { event: "move-zone", what, from: { zone: "battlefield" } };
    if (c.words("be", "destroyed")) return { event: "destroy", what };
    // CR 608.3, 614.12: entering is being put onto the battlefield, by a
    // resolving permanent spell or otherwise.
    if (c.word("enter") !== null) { c.words("the", "battlefield"); return { event: "move-zone", what, to: { zone: "battlefield" } }; }
    if (!c.words("be", "put", "into") && !c.words("be", "put", "onto")) return null;
    const to = parseZoneRef(c);
    if (!to) return null;
    const event: Extract<ReplacementEvent, { event: "move-zone" }> = { event: "move-zone", what, to };
    if (c.word("from") !== null) {
      const from = c.word("anywhere") !== null ? "anywhere" : parseZoneRef(c);
      if (!from) return null;
      event.from = from;
    }
    return event;
  });
}

interface ParsedHeader { event: ReplacementEvent; duration?: Duration; condition?: ReplacementInstruction["condition"] }
function parseHeader(tokens: Token[]): ParsedHeader | null {
  const c = new Cursor(tokens);
  const parsedEvent = parseEvent(c);
  if (!parsedEvent) return null;
  const duration = parseDuration(c) ?? undefined;
  // "would deal damage this turn to …": the recipients follow the duration.
  if (duration && parsedEvent.event === "damage" && !parsedEvent.to && c.word("to") !== null) {
    const to = parseRecipients(c);
    if (!to) return null;
    parsedEvent.to = to;
  }
  const event = bindWatchedExclusions(parsedEvent);
  if (!event) return null;
  let condition: ReplacementInstruction["condition"];
  // CR 400.7d: the entering permanent's own casting history.
  if (event.event === "move-zone" && event.to?.zone === "battlefield" && !event.from && c.words("and", "it", "wasn't", "cast"))
    condition = { condition: "not", of: { condition: "was-cast", what: eventObject("affected") } };
  else if (c.word("while") !== null) {
    // "Your library has no cards in it" is "there are no cards in your library".
    const empty = c.attempt(c => {
      const zone = parseZoneRef(c);
      return zone?.zone === "library" && !zone.position && c.words("has", "no", "cards", "in", "it") ? zone : null;
    });
    const parsed: ReplacementInstruction["condition"] | null = empty
      ? { condition: "count", of: { cls: "card", zone: empty }, comparison: { op: "eq", value: { amount: "fixed", value: 0 } } }
      : parseCondition(c);
    if (!parsed) return null;
    condition = parsed;
  }
  else if (c.words("during", "your", "turn")) condition = { condition: "your-turn" };
  if (!c.done()) return null;
  return { event, ...(duration ? { duration } : {}), ...(condition ? { condition } : {}) };
}

function bindWatchedExclusions(event: ReplacementEvent): ReplacementEvent | null {
  const bind = (ref: ObjectRef, otherThan: ObjectRef): ObjectRef => {
    if (!("filter" in ref) || !ref.filter.other) return ref;
    const { other: _, ...filter } = ref.filter;
    return { ...ref, filter: { ...filter, otherThan } };
  };
  if (event.event === "move-zone") return { ...event, what: bind(event.what, { ref: "self" }) };
  if (event.event === "put-counters") return { ...event, on: event.on.map(ref => "ref" in ref ? bind(ref, { ref: "self" }) : ref) };
  if (event.event === "damage" && event.source && "filter" in event.source && event.source.filter.other) {
    let otherThan: ObjectRef = { ref: "self" };
    const recipient = event.to?.length === 1 ? event.to[0] : undefined;
    if (recipient && "ref" in recipient) {
      const nominal = event.source.filter.types;
      if ((recipient.ref === "equipped" || recipient.ref === "enchanted")
        && nominal?.length === 1 && nominal[0] === recipient.noun) otherThan = eventObject("recipient");
      else if ("filter" in recipient && nominal?.some(type => recipient.filter.types?.includes(type))) return null;
    }
    return { ...event, source: bind(event.source, otherThan) };
  }
  return event;
}

type Change = Extract<ReplacementInstruction["replace"], { replacement: "modify-quantity" }>["change"];
function parseChangedQuantity(c: Cursor): Change | null {
  return c.attempt(c => {
    const times = c.attempt((c): Change | null => {
      const factor = parseCount(c);
      if (factor?.amount !== "fixed" || factor.value < 1 || c.word("times") === null
        || (!c.words("that", "many") && !c.words("that", "much"))) return null;
      return { operation: "multiply", factor: factor.value };
    });
    if (times) return times;
    if (c.word("half") !== null) {
      if (!c.words("that", "many") && !c.words("that", "much") && !c.words("that", "damage")) return null;
      // The explicit rounding phrase is parsed after the repeated unit and
      // recipient. A half change is rejected without that phrase below.
      return { operation: "half", round: "down" };
    }
    const multiplier = c.word("twice", "double", "triple");
    if (multiplier) {
      if (!c.words("that", "much") && !c.words("that", "many") && !c.words("that", "damage")) return null;
      return { operation: "multiply", factor: multiplier === "triple" ? 3 : 2 };
    }
    if (c.words("that", "much") || c.words("that", "many")) {
      if (c.word("plus") === null) return { operation: "set", amount: eventQuantity };
      const amount = parseAmount(c);
      return amount && { operation: "add", amount };
    }
    const amount = c.word("no") !== null ? { amount: "fixed", value: 0 } as const : parseAmount(c);
    return amount && { operation: "set", amount };
  });
}

function isEventPlayer(ref: PlayerRef, event: ReplacementEvent): boolean {
  const who = "who" in event ? event.who : event.event === "create-token" ? event.under : undefined;
  return Boolean(who && (same(ref, who) || ref.player === "that-player" || ref.player === "the-player" || ref.player === "they"));
}

function isEventObject(ref: ObjectRef, event: ReplacementEvent): boolean {
  if (ref.ref === "it") return true;
  const watched = event.event === "move-zone" || event.event === "destroy" || event.event === "discard" ? event.what : event.event === "damage" ? event.source : undefined;
  if (ref.ref === "that") {
    // Whatever would deal the watched damage is its source (CR 609.7).
    if (event.event === "damage" && watched && ref.filter.cls === "source" && Object.keys(ref.filter).length === 1) return true;
    return Boolean(watched && matchesDefiniteFilter(ref.filter, watched));
  }
  return same(ref, watched);
}

function matchesDefiniteFilter(definite: ObjectFilter, watched: ObjectRef): boolean {
  if (Object.keys(definite).length !== 1) return false;
  const filter: ObjectFilter | undefined = "filter" in watched ? watched.filter : watched.ref === "this" || watched.ref === "enchanted" || watched.ref === "equipped"
    ? ["card", "permanent", "spell", "token"].includes(watched.noun) ? { cls: watched.noun as ObjectFilter["cls"] } : { types: [watched.noun] }
    : undefined;
  if (!filter) return false;
  if (definite.cls) {
    if (filter.cls === definite.cls) return true;
    if (definite.cls === "card") return "nonToken" in filter && filter.nonToken === true;
    if (definite.cls === "permanent") {
      const permanentTypes = ["creature", "artifact", "enchantment", "land", "planeswalker", "battle"];
      if (filter.types?.some(type => permanentTypes.includes(type))) return true;
      // "An artifact or creature you control": only permanents are controlled
      // objects with these types or subtypes (CR 110.1, 110.2).
      const alternatives = filter.allOf?.length === 1 ? filter.allOf[0].anyOf : undefined;
      return Boolean(filter.control && alternatives?.every(alternative => Object.keys(alternative).every(key => key === "types" || key === "subtypes")
        && (alternative.types ?? []).every(type => permanentTypes.includes(type))));
    }
    return false;
  }
  return Boolean(definite.types?.length && definite.types.every(type => filter.types?.includes(type)));
}

function parseQuantityReplacement(c: Cursor, event: ReplacementEvent): ReplacementInstruction["replace"] | null {
  return c.attempt(c => {
    if (event.event === "gain-life" || event.event === "lose-life") {
      const who = parsePlayerRef(c);
      if (!who || !isEventPlayer(who, event) || c.word(event.event === "gain-life" ? "gain" : "lose", event.event === "gain-life" ? "gains" : "loses") === null) return null;
      // "that much life plus N" puts the unit before the addend.
      const plus = c.attempt((c): Change | null => {
        if (!c.words("that", "much", "life", "plus")) return null;
        const amount = parseAmount(c);
        return amount && { operation: "add", amount };
      });
      const change = plus ?? parseChangedQuantity(c);
      if (!change || change.operation === "half" || (!plus && c.word("life") === null) || !c.done()) return null;
      return { replacement: "modify-quantity", change };
    }
    if (event.event === "draw") {
      const who = parsePlayerRef(c);
      if (who && !isEventPlayer(who, event)) return null;
      if (!who && event.who.player !== "you") return null;
      if (c.word("draw", "draws") === null) return null;
      const change = parseChangedQuantity(c);
      if (!change || change.operation === "half" || c.word("card", "cards") === null || !c.done()) return null;
      return { replacement: "modify-quantity", change };
    }
    if (event.event === "damage") {
      const source = parseObjectRef(c);
      if (!source || !isEventObject(source, event) || c.word("deal", "deals") === null) return null;
      let change = parseChangedQuantity(c);
      if (!change) return null;
      // "double/triple/half that damage" already consumed the damage noun.
      const previous = c.tokens[c.pos - 1];
      if (!word(previous, "damage") && c.word("damage") === null) return null;
      if (change.operation === "half") {
        // CR 107.1a: halving states its rounding ("…, rounded down, to …").
        if (!c.punct(",") || c.word("rounded") === null) return null;
        const round = c.word("up", "down");
        if (round !== "up" && round !== "down") return null;
        change = { operation: "half", round };
        c.punct(",");
      }
      const adjust = c.word("plus", "minus");
      if (adjust) {
        if (change.operation !== "set" || change.amount.amount !== "replacement-event-quantity") return null;
        const amount = parseAmount(c);
        // "minus A plus B" would subtract A only; no such amount is printed.
        if (!amount || (adjust === "minus" && (amount.amount === "plus" || amount.amount === "minus"))) return null;
        change = { operation: adjust === "plus" ? "add" : "subtract", amount };
      }
      if (c.word("to") !== null) {
        // An explicitly repeated recipient must match the watched recipient;
        // redirection is a different operation and is not a quantity change.
        if (!parseRepeatedRecipients(c, event.to)) return null;
      }
      if (!c.done()) return null;
      return { replacement: "modify-quantity", change };
    }
    if (event.event === "put-counters") {
      const actor = parsePlayerRef(c);
      let active = false;
      if (actor) {
        const get = c.word("get", "gets") !== null;
        if (get) {
          if (event.by || event.on.length !== 1 || !same(event.on[0], actor)) return null;
        } else if (!event.by || !(same(actor, event.by) || actor.player === "they") || c.word("put", "puts") === null) return null;
        active = true;
      } else if (c.words("it", "puts")) { if (event.cause !== "effect") return null; active = true; }
      else if (c.word("put") !== null) { if (event.by?.player !== "you") return null; active = true; }
      let change = parseChangedQuantity(c);
      if (!change || !parseRepeatedCounter(c, event.counter)) return null;
      if (c.word("minus") !== null) {
        if (change.operation !== "set" || change.amount.amount !== "replacement-event-quantity") return null;
        const amount = parseAmount(c);
        if (!amount) return null;
        change = { operation: "subtract", amount };
      }
      if (!active && !c.words("are", "put")) return null;
      if (c.word("on") !== null) { if (!parseRepeatedRecipients(c, event.on)) return null; }
      else if (!(event.on.length === 1 && "player" in event.on[0] && actor && same(actor, event.on[0]))) return null;
      if (change.operation === "half") {
        if (!c.punct(",") || c.word("rounded") === null) return null;
        const round = c.word("up", "down");
        if (round !== "up" && round !== "down") return null;
        change = { operation: "half", round };
      }
      if (!c.done()) return null;
      return { replacement: "modify-quantity", change };
    }
    if (event.event === "create-token") {
      let active = false;
      const actor = parsePlayerRef(c);
      if (actor) { if (!isEventPlayer(actor, event) || c.word("create", "creates") === null) return null; active = true; }
      else if (c.words("it", "creates")) { if (event.cause !== "effect") return null; active = true; }
      else if (c.word("create") !== null) { if (event.under?.player !== "you") return null; active = true; }
      const change = parseChangedQuantity(c);
      // Scaling all original tokens preserves each token specification. A
      // flat added number would not identify WHICH tokens to add.
      if (!change || (change.operation !== "multiply" && change.operation !== "half")) return null;
      if (!c.words("of", "those", "tokens") && !c.words("of", "each", "of", "those", "kinds", "of", "tokens")) return null;
      if (!active && !c.words("are", "created")) return null;
      if (change.operation === "half") {
        if (!c.punct(",") || c.word("rounded") === null || c.word(change.round) === null) return null;
      }
      if (!c.done()) return null;
      return { replacement: "modify-quantity", change };
    }
    return null;
  });
}

function parseRepeatedCounter(c: Cursor, counter: CounterSpec | "any"): boolean {
  if (counter !== "any" && counter.type === "energy" && c.attempt(c => c.symbol() === "E" ? true : null)) return true;
  if (c.words("of", "those", "counters") || c.words("of", "each", "of", "those", "kinds", "of", "counters")) return true;
  const repeated = parseCounterSpec(c);
  return repeated !== null && same(repeated, counter) && c.word("counter", "counters") !== null;
}

function parseRepeatedRecipients(c: Cursor, watched: (ObjectRef | PlayerRef)[] | undefined): boolean {
  if (!watched?.length) return false;
  // These definite phrases identify the single affected recipient of the
  // prospective event, even if its eligible set contains different classes.
  if (c.words("that", "permanent", "or", "player") || c.words("that", "player", "or", "permanent"))
    return watched.some(ref => "player" in ref) && watched.some(ref => "ref" in ref);
  const player = parsePlayerRef(c);
  if (player) return watched.every(ref => "player" in ref) && (player.player === "that-player" || player.player === "they" || watched.some(ref => same(ref, player)));
  const object = parseObjectRef(c);
  return Boolean(object && watched.every(ref => "ref" in ref) && (object.ref === "it"
    || (object.ref === "that" && watched.every(ref => "ref" in ref && matchesDefiniteFilter(object.filter, ref)))
    || watched.some(ref => same(ref, object))));
}

function parseTokenReplacement(tokens: Token[], event: ReplacementEvent, ctx: EffectContext, parseBody: ReplacementBodyParser): ReplacementInstruction["replace"] | null {
  if (event.event !== "create-token") return null;
  const c = new Cursor(tokens);
  const actor = parsePlayerRef(c);
  let active = c.word("create", "creates") !== null;
  if (active && !(actor ? isEventPlayer(actor, event) : event.under?.player === "you")) return null;
  if (actor && !active) return null;
  const augment = c.words("those", "tokens", "plus");
  let payload = tokens.slice(c.pos);
  if (!active) {
    if (!word(payload[payload.length - 2], "are") || !word(payload[payload.length - 1], "created")) return null;
    payload = payload.slice(0, -2);
  }
  if (augment && (word(payload[0], "a") || word(payload[0], "an")) && word(payload[1], "additional"))
    payload = payload.filter((_, index) => index !== 1);
  const create: Token = { kind: "word", value: "create", raw: "create", possessive: false };
  const body = new Cursor([create, ...payload]);
  const parsed = parseBody(body, { modalOptions: ctx.modalOptions });
  if (!body.done() || parsed?.length !== 1 || parsed[0].condition || parsed[0].steps.length !== 1) return null;
  const step = parsed[0].steps[0];
  if (step.effect !== "create-token" || step.optional || step.duration || step.forEach || "copyOf" in step.token) return null;
  // The total event quantity is used only for the additional/replacement
  // token count; it does not multiply an unspecified original token kind.
  const count = step.count.amount === "that-much" ? eventQuantity : step.count;
  if (count.amount !== "fixed" && count.amount !== "replacement-event-quantity") return null;
  return augment ? { replacement: "augment-tokens", count, token: step.token }
    : { replacement: "replace-token-spec", count, token: step.token };
}

const ANAPHORIC_REFS = new Set(["it", "them", "that", "rest", "related", "copy-result", "choice-result", "trigger-event-object", "from-pool"]);
const ANAPHORIC_PLAYERS = new Set(["that-player", "they", "the-player", "chosen-player", "that-player-or-controller"]);
const ANAPHORIC_AMOUNTS = new Set(["that-much", "that-number", "event-quantity", "dice-result", "vote-count"]);
const ANAPHORIC_WORDS = new Set(["their", "its-owner", "that-player"]);
/** Any reference whose antecedent lies outside the instruction itself,
 * wherever it occurs (including inside granted or delayed abilities). */
function containsAnaphor(value: unknown): boolean {
  if (typeof value === "string") return ANAPHORIC_WORDS.has(value);
  if (Array.isArray(value)) return value.some(containsAnaphor);
  if (!value || typeof value !== "object") return false;
  const node = value as Record<string, unknown>;
  if (typeof node.ref === "string" && ANAPHORIC_REFS.has(node.ref)) return true;
  if (typeof node.player === "string" && ANAPHORIC_PLAYERS.has(node.player)) return true;
  if (typeof node.amount === "string" && ANAPHORIC_AMOUNTS.has(node.amount)) return true;
  if (node.recipient === "they" || node.other === true) return true;
  return Object.values(node).some(containsAnaphor);
}

/** Only known direct instructions are rebound here. Traversing arbitrary
 * JSON would incorrectly capture pronouns inside quoted or delayed abilities. */
function bindBody(body: Sentence[], event: ReplacementEvent): Sentence[] | null {
  const bindObject = (ref: ObjectRef): ObjectRef | null => {
    if ((event.event === "move-zone" || event.event === "destroy" || event.event === "discard") && isEventObject(ref, event)) return eventObject("affected");
    if (event.event === "damage" && event.to?.length === 1 && "ref" in event.to[0]) {
      const recipient = event.to[0];
      if (same(ref, recipient) || (ref.ref === "it" && !event.source)
        || (ref.ref === "that" && matchesDefiniteFilter(ref.filter, recipient))) return eventObject("recipient");
    }
    if (ref.ref === "it" || ref.ref === "them" || ref.ref === "that") return null;
    return ref;
  };
  const bindPlayer = (ref: PlayerRef): PlayerRef | null => {
    if (ref.player === "group") {
      const members: PlayerRef[] = [];
      for (const member of ref.members) {
        const bound = bindPlayer(member);
        if (!bound) return null;
        members.push(bound);
      }
      return { ...ref, members };
    }
    // "each other player": the contrasted player may be the event's own.
    if (ref.player === "filtered" && ref.filter.other) return null;
    if (ref.player === "controller" || ref.player === "owner") {
      const of = bindObject(ref.of);
      return of && { ...ref, of };
    }
    if (ref.player === "that-player" || ref.player === "the-player" || ref.player === "they") {
      if (isEventPlayer(ref, event)) return eventPlayer("actor");
      if (event.event === "damage" && event.to?.length === 1 && "player" in event.to[0]) return eventPlayer("recipient");
      return null;
    }
    return ref;
  };
  let quantityStillUnambiguous = event.event !== "move-zone" && event.event !== "lose-game" && event.event !== "discard";
  const bindAmount = (amount: Amount): Amount | null => {
    if (amount.amount === "that-much") return quantityStillUnambiguous ? eventQuantity : null;
    if (amount.amount === "that-number") return null;
    // The general amount grammar historically supplies a default rounding
    // direction. Replacement bodies accept halves only through the explicit
    // quantity grammar above, which requires the printed rounding clause.
    if (amount.amount === "half") return null;
    if (amount.amount === "twice") {
      const of = bindAmount(amount.of);
      return of && { ...amount, of };
    }
    if (amount.amount === "plus" || amount.amount === "minus") {
      const a = bindAmount(amount.a), b = bindAmount(amount.b);
      return a && b && { ...amount, a, b };
    }
    if (amount.amount === "attribute" || amount.amount === "counter-count") {
      const ref = amount.amount === "attribute" ? amount.of : amount.on;
      const bound = "ref" in ref ? bindObject(ref) : bindPlayer(ref);
      if (!bound) return null;
      if (amount.amount === "attribute") return "ref" in bound ? { ...amount, of: bound } : null;
      return { ...amount, on: bound };
    }
    return amount;
  };
  const output: Sentence[] = [];
  for (const sentence of body) {
    if (sentence.condition || sentence.otherwise || sentence.bindings) return null;
    const steps: Effect[] = [];
    for (const step of sentence.steps) {
      // A printed duration limits a continuous effect of the replacement.
      if (step.optional || step.forEach || (step.duration && step.effect !== "gain-abilities" && step.effect !== "pump")) return null;
      // An instruction that refers to nothing of the watched event is
      // complete as printed and needs no binding.
      // Tokens created in place of tokens stay with the dedicated production.
      const free = !containsAnaphor(step) && !(event.event === "create-token" && JSON.stringify(step).includes('"effect":"create-token"'));
      switch (step.effect) {
        case "regenerate": case "gain-abilities": case "pump": {
          if (event.event !== "destroy") { if (!free) return null; steps.push(step); quantityStillUnambiguous = false; break; }
          if (!("ref" in step.what)) return null;
          const what = bindObject(step.what);
          if (!what) return null;
          steps.push({ ...step, what }); break;
        }
        case "exile": case "return-to-hand": case "move-zone": case "reveal": {
          if (event.event !== "move-zone" && event.event !== "destroy" && event.event !== "discard") { if (!free) return null; steps.push(step); quantityStillUnambiguous = false; break; }
          const what = bindObject(step.what);
          if (!what) return null;
          steps.push({ ...step, what }); break;
        }
        // "Reveal ~ and shuffle it into its owner's library instead" (CR 614.1a, 701.24c).
        case "shuffle-into-library": {
          if (event.event !== "move-zone") { if (!free) return null; steps.push(step); quantityStillUnambiguous = false; break; }
          const what: ObjectRef[] = [];
          for (const ref of step.what) { const bound = bindObject(ref); if (!bound) return null; what.push(bound); }
          steps.push({ ...step, what }); break;
        }
        case "create-token":
          // Replacing a token's characteristics must retain the originating
          // creation modifiers. The dedicated token production owns it.
          if (event.event === "create-token") return null;
          if ("copyOf" in step.token) return null;
          { const count = bindAmount(step.count); if (!count) return null;
            const who = step.who && bindPlayer(step.who); if (step.who && !who) return null;
            steps.push({ ...step, count, ...(who && { who }) }); quantityStillUnambiguous = false; } break;
        case "draw": case "gain-life": case "lose-life": case "mill":
          { const amount = bindAmount(step.amount); if (!amount) return null;
            const who = bindPlayer(step.who); if (!who) return null;
            steps.push({ ...step, who, amount }); quantityStillUnambiguous = false; } break;
        case "put-counters":
          if (event.event !== "move-zone" && event.event !== "draw" && event.event !== "damage") { if (!free) return null; steps.push(step); quantityStillUnambiguous = false; break; }
          { const on = bindObject(step.on); if (!on) return null;
            const count = bindAmount(step.count); if (!count) return null;
            const who = step.who && bindPlayer(step.who); if (step.who && !who) return null;
            steps.push({ ...step, on, count, ...(who && { who }) }); quantityStillUnambiguous = false; } break;
        default:
          if (!free) return null;
          steps.push(step); quantityStillUnambiguous = false;
      }
    }
    output.push({ steps });
  }
  return output;
}

function parseCore(c: Cursor, ctx: EffectContext, parseBody: ReplacementBodyParser, options: ReplacementOptions): { replacement: ReplacementInstruction; duration?: Duration } | null {
  return c.attempt(c => {
    let duration = options.leadingDuration;
    if (!duration) {
      const prefix = c.attempt(c => { const value = parseDuration(c); return value && c.punct(",") ? value : null; });
      if (prefix) duration = prefix;
    }
    const floor = parseLifeFloor(c);
    if (floor) return { replacement: floor, ...(duration ? { duration } : {}) };
    // "Each time X would …" states the recurrence that "if" leaves implicit.
    const occurrence = c.words("the", "next", "time") ? "next" : c.word("if") !== null || c.words("each", "time") ? "each" : null;
    if (!occurrence) return null;
    const headerStart = c.pos;
    for (let comma = headerStart; comma < c.tokens.length; comma++) {
      const token = c.tokens[comma];
      if (token.kind === "punct" && token.value === ".") break;
      if (token.kind !== "punct" || token.value !== ",") continue;
      const header = parseHeader(c.tokens.slice(headerStart, comma));
      if (!header) continue;
      // A comma inside a list ("an Army, Goblin, or Orc you control") also
      // ends a shorter event; that reading has no body.
      const parsed = c.attempt(c => { c.pos = comma + 1; return parseAfterHeader(c, ctx, parseBody, header, occurrence, duration); });
      if (parsed) return parsed;
    }
    return null;
  });
}

function parseAfterHeader(c: Cursor, ctx: EffectContext, parseBody: ReplacementBodyParser, header: ParsedHeader,
  occurrence: ReplacementInstruction["occurrence"], leading: Duration | undefined): { replacement: ReplacementInstruction; duration?: Duration } | null {
  {
    let duration = leading;
    if (duration && header.duration) return null;
    duration ??= header.duration;
    const bodyStart = c.pos;
    let end = bodyStart;
    let quoted = false;
    for (; end < c.tokens.length; end++) {
      const token = c.tokens[end];
      if (token.kind === "punct" && token.value === '"') quoted = !quoted;
      if (!quoted && token.kind === "punct" && token.value === ".") break;
    }
    const raw = c.tokens.slice(bodyStart, end);
    const body = new Cursor(raw);
    const optional = body.attempt(c => {
      const who = parsePlayerRef(c);
      return who && c.word("may") !== null ? { who, length: c.pos } : null;
    });
    if (optional) body.pos = 0;
    let tokens = optional ? raw.slice(optional.length) : raw;
    if (header.event.event === "tap-for-mana" || header.event.event === "lose-unspent-mana") {
      const replace = optional ? null : header.event.event === "tap-for-mana" ? parseManaReplacement(tokens, header.event) : parseKeptMana(tokens);
      const next = c.tokens[end + 1];
      if (!replace || (next && !(next.kind === "word" && next.value === "activate"))) return null;
      c.pos = end;
      c.punct(".");
      return { replacement: { event: header.event, occurrence, replace, ...(header.condition ? { condition: header.condition } : {}) }, ...(duration ? { duration } : {}) };
    }
    // CR 614.8, 701.19b: "instead" is implicit in regenerating.
    if (header.event.event === "destroy" && !optional && tokens.length === 2 && word(tokens[0], "regenerate") && word(tokens[1], "it")) {
      const next = c.tokens[end + 1];
      if (next && !(next.kind === "word" && next.value === "activate")) return null;
      c.pos = end;
      c.punct(".");
      return { replacement: { event: header.event, occurrence, replace: { replacement: "effects", effects: [{ steps: [{ effect: "regenerate", what: eventObject("affected") }] }] },
        ...(header.condition ? { condition: header.condition } : {}) }, ...(duration ? { duration } : {}) };
    }
    const positions = tokens.flatMap((token, index) => word(token, "instead") ? [index] : []);
    if (positions.length !== 1) return null;
    const position = positions[0];
    // "… instead of putting it anywhere else" restates that every destination
    // of a leaves-the-battlefield event is replaced; the event says so already.
    const elsewhere = tokens.slice(position + 1);
    if (elsewhere.length === 5 && header.event.event === "move-zone" && !header.event.to
      && header.event.from !== "anywhere" && header.event.from?.zone === "battlefield"
      && word(elsewhere[0], "of") && word(elsewhere[1], "putting") && (word(elsewhere[2], "it") || word(elsewhere[2], "them"))
      && word(elsewhere[3], "anywhere") && word(elsewhere[4], "else")) tokens = tokens.slice(0, position);
    // "… instead of putting it into your graveyard" names the discarded
    // card's own destination (CR 701.9a); "anywhere else" covers every one.
    else if (header.event.event === "discard" && header.event.who.player === "you" && word(elsewhere[0], "of") && word(elsewhere[1], "putting") && word(elsewhere[2], "it")
      && ((elsewhere.length === 6 && word(elsewhere[3], "into") && word(elsewhere[4], "your") && word(elsewhere[5], "graveyard"))
        || (elsewhere.length === 5 && word(elsewhere[3], "anywhere") && word(elsewhere[4], "else")))) tokens = tokens.slice(0, position);
    else
    if (position === 0) tokens = tokens.slice(1);
    else if (position === tokens.length - 1) tokens = tokens.slice(0, -1);
    else if (position === tokens.length - 4 && tokens[position + 1]?.kind === "punct"
      && (tokens[position + 1] as Extract<Token, { kind: "punct" }>).value === ","
      && word(tokens[position + 2], "rounded") && (word(tokens[position + 3], "up") || word(tokens[position + 3], "down")))
      tokens = tokens.filter((_, index) => index !== position);
    else return null;
    if (!tokens.length) return null;
    // Optionality belongs to choosing the replacement, not to later actions
    // inside it; preserve an explicit non-you actor by reusing their tokens.
    if (optional && optional.who.player !== "you") tokens = [...raw.slice(0, optional.length - 1), ...tokens];
    const direct = new Cursor(tokens);
    let replace = parseSkip(tokens, header.event, Boolean(optional), ctx, parseBody)
      ?? parseQuantityReplacement(direct, header.event) ?? parseActionReplacement(new Cursor(tokens), header.event)
      ?? parseTokenReplacement(tokens, header.event, ctx, parseBody);
    if (!replace) {
      const nested = new Cursor(tokens);
      const sentences = parseBody(nested, { modalOptions: ctx.modalOptions });
      const effects = sentences && nested.done() ? bindBody(sentences, header.event) : null;
      if (!effects) return null;
      replace = { replacement: "effects", effects };
    }
    let instructionChoice = optional;
    const outcome = parseEntryOutcome(c.tokens, end + 1, header.event, replace, optional?.who);
    if (outcome) { replace = outcome.replace; end = outcome.end; instructionChoice = null; }
    const after = replace.replacement === "skip" && optional ? parseAfterSkip(c.tokens, end + 1, ctx, parseBody) : null;
    if (after) { replace = { replacement: "skip", after: after.sentences }; end = after.end; }
    const reflexive = replace.replacement === "effects" ? parseReflexiveAfter(c.tokens, end + 1, ctx, parseBody) : null;
    if (reflexive && replace.replacement === "effects") { replace = { ...replace, effects: [...replace.effects, ...reflexive.sentences] }; end = reflexive.end; }
    // A following linked conditional or reflexive trigger can belong inside
    // the replacement body; reject until that scope is represented explicitly.
    // An instruction whose object is this source itself ("Exile ~.") is a
    // separate instruction, not a continuation of the replacement body.
    const next = c.tokens[end + 1], object = c.tokens[end + 2];
    const aboutSource = next?.kind === "word" && !next.possessive && (object?.kind === "selfref" && !object.possessive || word(object, "this"));
    if (next && !(next.kind === "word" && next.value === "activate") && !aboutSource) return null;
    c.pos = end;
    c.punct(".");
    return { replacement: { event: header.event, occurrence, replace,
      ...(header.condition ? { condition: header.condition } : {}),
      ...(instructionChoice ? { optional: { who: isEventPlayer(instructionChoice.who, header.event)
        && ["that-player", "they"].includes(instructionChoice.who.player) ? eventPlayer("actor") : instructionChoice.who } } : {}) }, ...(duration ? { duration } : {}) };
  }
}

/** CR 614.10: "that player skips that turn", "you may skip that step",
 * "skips that draw". Skipping a draw and doing something else is replacing
 * the draw with that something else ("instead that player skips that draw
 * and you draw a card"). */
function parseSkip(tokens: Token[], event: ReplacementEvent, optional: boolean, ctx: EffectContext, parseBody: ReplacementBodyParser): ReplacementInstruction["replace"] | null {
  if (event.event !== "begin-time" && event.event !== "draw") return null;
  const c = new Cursor(tokens);
  const who = parsePlayerRef(c);
  if (who ? !isEventPlayer(who, event) || c.word(optional && who.player === "you" ? "skip" : "skips") === null
    : !optional || event.who.player !== "you" || c.word("skip") === null) return null;
  if (c.word("that") === null) return null;
  const noun = event.event === "draw" ? "draw" : event.what.kind === "step" ? "step" : "turn";
  if (c.word(noun) === null) return null;
  if (c.done()) return { replacement: "skip" };
  if (event.event !== "draw" || optional || c.word("and") === null) return null;
  const rest = new Cursor(tokens.slice(c.pos));
  const sentences = parseBody(rest, { modalOptions: ctx.modalOptions });
  const effects = sentences && rest.done() ? bindBody(sentences, event) : null;
  return effects && { replacement: "effects", effects };
}

/** "If you do, …" after an optional skip: the next sentence, up to its period. */
function parseAfterSkip(tokens: Token[], start: number, ctx: EffectContext, parseBody: ReplacementBodyParser): { sentences: Sentence[]; end: number } | null {
  const c = new Cursor(tokens.slice(start));
  if (!c.words("if", "you", "do") || !c.punct(",")) return null;
  let end = c.pos;
  while (end < c.tokens.length && !(c.tokens[end].kind === "punct" && (c.tokens[end] as Extract<Token, { kind: "punct" }>).value === ".")) end++;
  const body = new Cursor(c.tokens.slice(c.pos, end));
  const sentences = parseBody(body, { modalOptions: ctx.modalOptions });
  if (!sentences?.length || !body.done() || containsAnaphor(sentences)) return null;
  return { sentences, end: start + end };
}

/** "… exile it instead. When you do, …": CR 603.12, a reflexive ability
 * created by the replacement's own instruction, performed by its controller.
 * It ends the ability and carries no reference into the replaced event. */
function parseReflexiveAfter(tokens: Token[], start: number, ctx: EffectContext, parseBody: ReplacementBodyParser): { sentences: Sentence[]; end: number } | null {
  const c = new Cursor(tokens.slice(start));
  if (!c.isWord("when") || !(word(tokens[start + 1], "you") && word(tokens[start + 2], "do"))) return null;
  const sentences = parseBody(c, { modalOptions: ctx.modalOptions });
  const step = sentences?.length === 1 && sentences[0].steps.length === 1 && !sentences[0].condition ? sentences[0].steps[0] : undefined;
  if (!c.done() || step?.effect !== "create-reflexive-trigger" || step.event.event !== "preceding-action"
    || !step.event.performed || step.event.who.player !== "you" || containsAnaphor(step.effects)) return null;
  const last = tokens[tokens.length - 1];
  return { sentences: sentences!, end: last.kind === "punct" && last.value === "." ? tokens.length - 1 : tokens.length };
}

/** "Damage that would reduce your life total to less than N reduces it to
 * N instead." (CR 120.3a, 614.1a): a replacement of the life total that
 * results from damage, not of the damage and not of other life loss. */
function parseLifeFloor(c: Cursor): ReplacementInstruction | null {
  return c.attempt(c => {
    if (!c.words("damage", "that", "would", "reduce", "your", "life", "total", "to", "less", "than")) return null;
    const below = parseAmount(c);
    if (!below || !c.words("reduces", "it", "to")) return null;
    const amount = parseAmount(c);
    if (!amount || c.word("instead") === null) return null;
    if (!c.punct(".") && !c.done()) return null;
    return { event: { event: "damage-life-reduction", who: you, below }, occurrence: "each", replace: { replacement: "reduce-life-to", amount } };
  });
}

const MANA_COLOR_SYMBOLS: Record<string, string> = { colorless: "C", white: "W", blue: "U", black: "B", red: "R", green: "G" };
/** "That mana becomes colorless instead": the mana the player would lose. */
function parseKeptMana(tokens: Token[]): ReplacementInstruction["replace"] | null {
  const c = new Cursor(tokens);
  if (!c.words("that", "mana", "becomes")) return null;
  const color = c.word(...Object.keys(MANA_COLOR_SYMBOLS));
  return color && c.word("instead") !== null && c.done() ? { replacement: "keep-mana", as: { symbol: MANA_COLOR_SYMBOLS[color] } } : null;
}

/** CR 106.12b. "It" is the tapped permanent. "Twice as much of that mana"
 * scales the event and keeps its types; "instead of any other type" keeps
 * the amount; "… and amount" replaces the whole production, so the new
 * amount must be printed. */
function parseManaReplacement(tokens: Token[], event: Extract<ReplacementEvent, { event: "tap-for-mana" }>): ReplacementInstruction["replace"] | null {
  const c = new Cursor(tokens);
  if (c.word("it") === null || c.word("produces") === null) return null;
  const factor = c.word("twice") !== null ? 2 : c.attempt(c => {
    const count = parseCount(c);
    return count?.amount === "fixed" && count.value > 1 && c.word("times") !== null ? count.value : null;
  });
  if (factor) {
    return c.words("as", "much", "of", "that", "mana") && c.word("instead") !== null && c.done()
      ? { replacement: "modify-quantity", change: { operation: "multiply", factor } } : null;
  }
  let type: { symbol: string } | { choice: "color"; who: PlayerRef };
  let amount: 1 | undefined;
  const symbol = c.symbol();
  if (symbol !== null) {
    if (!["W", "U", "B", "R", "G", "C"].includes(symbol)) return null;
    type = { symbol }; amount = 1;
  } else if (c.words("colorless", "mana")) type = { symbol: "C" };
  else {
    if (c.word("one") !== null) amount = 1;
    if (!c.words("mana", "of", "a", "color", "of", "your", "choice")) return null;
    type = { choice: "color", who: you };
  }
  if (!c.words("instead", "of")) return null;
  if (c.words("any", "other", "type")) {
    if (c.words("and", "amount")) {
      if (!amount || !c.done()) return null;
      if ("symbol" in type) return { replacement: "mana", mana: { mana: "fixed", symbols: [type.symbol] } };
      // The player adding the mana chooses its color; "your" must be that player.
      const yours = event.by?.player === "you" || ("filter" in event.what && event.what.filter.control === "you");
      return yours ? { replacement: "mana", mana: { mana: "any-color", amount: one } } : null;
    }
    // A printed amount without "and amount" would leave the amount unstated.
    if (!c.done() || (amount && !("symbol" in type))) return null;
    return { replacement: "mana-type", type };
  }
  const color = c.word("white", "blue", "black", "red", "green");
  if (!color || c.word("mana") === null || !c.done() || (amount && !("symbol" in type))) return null;
  return { replacement: "mana-type", type, only: color as NonNullable<ObjectFilter["colors"]>[number] };
}

/** "If this land would enter, sacrifice X instead. If you do, put this land
 * onto the battlefield. If you don't, put it into its owner's graveyard."
 * (CR 614.12a, 614.13): the two outcome sentences belong to the replacement
 * and test whether its one action was performed. A printed "may" then makes
 * that action optional, not the replacement: declining leads to the
 * graveyard, never to an unmodified entry. */
function parseEntryOutcome(tokens: Token[], start: number, event: ReplacementEvent, replace: ReplacementInstruction["replace"],
  chooser: PlayerRef | undefined): { replace: ReplacementInstruction["replace"]; end: number } | null {
  if (event.event !== "move-zone" || event.from || event.to?.zone !== "battlefield" || (event.what.ref !== "this" && event.what.ref !== "self")) return null;
  if (replace.replacement !== "effects" || replace.effects.length !== 1 || replace.effects[0].steps.length !== 1) return null;
  const action = replace.effects[0].steps[0];
  if ((action.effect !== "sacrifice" && action.effect !== "discard") || action.who.player !== "you" || (chooser && chooser.player !== "you")) return null;
  const watched = event.what;
  const c = new Cursor(tokens.slice(start));
  const entering = (): boolean => c.word("it") !== null || c.selfref()
    || (watched.ref === "this" && c.words("this", watched.noun));
  if (!c.words("if", "you", "do") || !c.punct(",") || c.word("put") === null || !entering() || !c.words("onto", "the", "battlefield") || !c.punct(".")) return null;
  if (!c.words("if", "you", "don't") || !c.punct(",") || c.word("put") === null || !entering() || c.word("into") === null) return null;
  const graveyard = parseZoneRef(c);
  if (graveyard?.zone !== "graveyard" || graveyard.owner !== "its-owner" || graveyard.position) return null;
  const affected = eventObject("affected");
  const effects: Sentence[] = [
    { steps: [chooser ? { ...action, optional: true } : action] },
    { condition: { condition: "you-do" }, conditionKind: "if", steps: [{ effect: "move-zone", what: affected, to: { zone: "battlefield" } }] },
    { condition: { condition: "you-dont" }, conditionKind: "if", steps: [{ effect: "move-zone", what: affected, to: graveyard }] }];
  return { replace: { replacement: "effects", effects }, end: start + c.pos };
}

function watchesCastThisWay(event: ReplacementEvent): boolean {
  if (event.event !== "move-zone" || !("filter" in event.what)) return false;
  const relation = event.what.filter.relation;
  return relation?.relation === "event-result" && relation.event === "cast";
}

/** Invoke before the enclosing parser consumes an ordinary `if` condition.
 * This owns the complete replacement sentence, including its final period. */
export function parseReplacementSentence(c: Cursor, ctx: EffectContext, parseBody: ReplacementBodyParser, options: ReplacementOptions = {}): Sentence | null {
  return c.attempt(c => {
    const parsed = parseCore(c, ctx, parseBody, options);
    // Without "if", only a printed duration marks this as a resolving effect.
    if (parsed?.replacement.event.event === "damage-life-reduction" && !parsed.duration) return null;
    return parsed && { steps: [{ effect: "create-replacement", ...parsed }] };
  });
}

/** "…, it can't be regenerated this turn, and if it would die this turn,
 * exile it instead." (Disintegrate): a replacement coordinated with the
 * preceding instructions of the same sentence shares their leading
 * condition. Leaves the sentence's final period to the caller. */
export function parseCoordinatedReplacement(c: Cursor, ctx: EffectContext, parseBody: ReplacementBodyParser): Effect[] | null {
  return c.attempt(c => {
    c.punct(",");
    if (c.word("and") === null || !c.isWord("if")) return null;
    const sentence = parseReplacementSentence(c, ctx, parseBody);
    if (!sentence) return null;
    const last = c.tokens[c.pos - 1];
    if (last?.kind === "punct" && last.value === ".") c.pos--;
    return sentence.steps;
  });
}

/** CR 701.10g: to double damage a source would deal, that source deals twice
 * that much damage instead; a replacement effect on all of its damage. A
 * plural subject matches each such source (CR 614.1). */
function parseDoubledDamage(c: Cursor): ReplacementInstruction | null {
  return c.attempt(c => {
    if (!c.words("double", "all", "damage")) return null;
    c.word("that");
    const subject = parseObjectRef(c);
    if (!subject || !c.words("would", "deal")) return null;
    c.punct(".");
    let source: ObjectRef;
    if (subject.ref === "each" && Object.keys(subject).every(key => key === "ref" || key === "filter")) source = { ref: "filter", filter: subject.filter, count: one };
    else if (subject.ref === "self" || subject.ref === "this" || subject.ref === "equipped" || subject.ref === "enchanted") source = subject;
    else return null;
    return { event: { event: "damage", kind: "all", source }, occurrence: "each",
      replace: { replacement: "modify-quantity", change: { operation: "multiply", factor: 2 } } };
  });
}

export function parseStaticReplacement(c: Cursor, parseBody: ReplacementBodyParser, options: StaticReplacementOptions = {}): StaticEffect | null {
  return c.attempt(c => {
    const doubled = options.afterCastPermission ? null : parseDoubledDamage(c);
    if (doubled) return c.done() ? { static: "replacement", replacement: doubled } : null;
    const parsed = parseCore(c, { modalOptions: [] }, parseBody, {});
    if (!parsed || !c.done() || parsed.replacement.occurrence === "next") return null;
    if (watchesCastThisWay(parsed.replacement.event) !== Boolean(options.afterCastPermission)) return null;
    const effect: StaticEffect = { static: "replacement", replacement: parsed.replacement };
    if (!parsed.duration) return effect;
    return parsed.duration.duration === "as-long-as"
      ? { static: "conditional", condition: parsed.duration.condition, effect } : null;
  });
}

/** Keep an unsupported static body from becoming a spell that merely creates
 * the same incomplete replacement. Explicit expiry and next-occurrence
 * effects remain resolution instructions. This check does not consume. */
export function looksLikeStaticReplacement(c: Cursor): boolean {
  const start = c.pos;
  const result = c.attempt(c => {
    if (c.word("if") === null) return null;
    const begin = c.pos;
    for (let end = begin; end < c.tokens.length; end++) {
      const token = c.tokens[end];
      if (token.kind === "punct" && token.value === ".") break;
      if (token.kind !== "punct" || token.value !== ",") continue;
      const header = parseHeader(c.tokens.slice(begin, end));
      if (header) return !header.duration;
    }
    return null;
  });
  c.pos = start;
  return result === true;
}
