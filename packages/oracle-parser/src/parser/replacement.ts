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
import { parseAmount, parseCondition, parseCount, parseCounterSpec, parseDuration,
  parseFilter, parseObjectRef, parsePlayerRef, parseZoneRef } from "./refs.ts";

export type ReplacementBodyParser = (c: Cursor, ctx: EffectContext) => Sentence[] | null;
export interface ReplacementOptions { leadingDuration?: Duration }
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
    const value = parseCount(c);
    if (!value) return null;
    const op = c.words("or", "more") ? "ge" : "eq";
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
    return first;
  });
}

function parseRecipients(c: Cursor): (ObjectRef | PlayerRef)[] | null {
  return c.attempt(c => {
    const refs: (ObjectRef | PlayerRef)[] = [];
    for (;;) {
      let ref: ObjectRef | PlayerRef | null = parsePlayerRef(c) ?? parseEligibleObject(c);
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
      if (!other && c.word("a", "an") === null) return null;
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
    const playerEvent = c.attempt((c): ReplacementEvent | null => {
      const who = parsePlayerRef(c);
      if (!who || c.word("would") === null) return null;
      const life = c.word("gain", "lose");
      if (life && c.word("life") !== null) return { event: life === "gain" ? "gain-life" : "lose-life", who };
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
        const counter = c.isWord("counter", "counters") ? "any" : parseCounterSpec(c);
        if (!quantity || !counter || c.word("counter", "counters") === null) return null;
        return { event: "put-counters", on: [who], counter, cause: "any", quantity };
      }
      return null;
    });
    if (playerEvent) return playerEvent;

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
  const event = parsedEvent && bindWatchedExclusions(parsedEvent);
  if (!event) return null;
  const duration = parseDuration(c) ?? undefined;
  let condition: ReplacementInstruction["condition"];
  if (c.word("while") !== null) {
    const parsed = parseCondition(c);
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
      if (!c.words("that", "many") && !c.words("that", "much")) return null;
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
  const watched = event.event === "move-zone" ? event.what : event.event === "damage" ? event.source : undefined;
  if (ref.ref === "that") {
    return Boolean(watched && matchesDefiniteFilter(ref.filter, watched));
  }
  return same(ref, watched);
}

function matchesDefiniteFilter(definite: ObjectFilter, watched: ObjectRef): boolean {
  if (Object.keys(definite).length !== 1) return false;
  const filter: ObjectFilter | undefined = "filter" in watched ? watched.filter : watched.ref === "this"
    ? ["card", "permanent", "spell", "token"].includes(watched.noun) ? { cls: watched.noun as ObjectFilter["cls"] } : { types: [watched.noun] }
    : undefined;
  if (!filter) return false;
  if (definite.cls) {
    if (filter.cls === definite.cls) return true;
    if (definite.cls === "card") return "nonToken" in filter && filter.nonToken === true;
    if (definite.cls === "permanent") return Boolean(filter.types?.some(type => ["creature", "artifact", "enchantment", "land", "planeswalker", "battle"].includes(type)));
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
      if (!change || change.operation === "half") return null;
      // "double/triple that damage" already consumed the damage noun.
      const previous = c.tokens[c.pos - 1];
      if (!word(previous, "damage") && c.word("damage") === null) return null;
      if (c.word("plus") !== null) {
        if (change.operation !== "set" || change.amount.amount !== "replacement-event-quantity") return null;
        const amount = parseAmount(c);
        if (!amount) return null;
        change = { operation: "add", amount };
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
  if (c.words("of", "those", "counters") || c.words("of", "each", "of", "those", "kinds", "of", "counters")) return true;
  const repeated = parseCounterSpec(c);
  return repeated !== null && same(repeated, counter) && c.word("counter", "counters") !== null;
}

function parseRepeatedRecipients(c: Cursor, watched: (ObjectRef | PlayerRef)[] | undefined): boolean {
  if (!watched?.length) return false;
  // These definite phrases identify the single affected recipient of the
  // prospective event, even if its eligible set contains different classes.
  if (c.words("that", "permanent", "or", "player")) return watched.some(ref => "player" in ref) && watched.some(ref => "ref" in ref);
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

/** Only known direct instructions are rebound here. Traversing arbitrary
 * JSON would incorrectly capture pronouns inside quoted or delayed abilities. */
function bindBody(body: Sentence[], event: ReplacementEvent): Sentence[] | null {
  const bindObject = (ref: ObjectRef): ObjectRef | null => {
    if (event.event === "move-zone" && isEventObject(ref, event)) return eventObject("affected");
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
  let quantityStillUnambiguous = event.event !== "move-zone";
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
      if (step.optional || step.duration || step.forEach) return null;
      switch (step.effect) {
        case "exile": case "return-to-hand": case "move-zone": case "reveal": {
          if (event.event !== "move-zone") return null;
          const what = bindObject(step.what);
          if (!what) return null;
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
          if (event.event !== "move-zone" && event.event !== "draw" && event.event !== "damage") return null;
          { const on = bindObject(step.on); if (!on) return null;
            const count = bindAmount(step.count); if (!count) return null;
            const who = step.who && bindPlayer(step.who); if (step.who && !who) return null;
            steps.push({ ...step, on, count, ...(who && { who }) }); quantityStillUnambiguous = false; } break;
        default: return null;
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
    const occurrence = c.words("the", "next", "time") ? "next" : c.word("if") !== null ? "each" : null;
    if (!occurrence) return null;
    const headerStart = c.pos;
    let header: ParsedHeader | null = null;
    for (let end = headerStart; end < c.tokens.length; end++) {
      const token = c.tokens[end];
      if (token.kind === "punct" && token.value === ".") break;
      if (token.kind !== "punct" || token.value !== ",") continue;
      header = parseHeader(c.tokens.slice(headerStart, end));
      if (header) { c.pos = end + 1; break; }
    }
    if (!header) return null;
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
    const positions = tokens.flatMap((token, index) => word(token, "instead") ? [index] : []);
    if (positions.length !== 1) return null;
    const position = positions[0];
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
    let replace = parseQuantityReplacement(direct, header.event) ?? parseTokenReplacement(tokens, header.event, ctx, parseBody);
    if (!replace) {
      const nested = new Cursor(tokens);
      const sentences = parseBody(nested, { modalOptions: ctx.modalOptions });
      const effects = sentences && nested.done() ? bindBody(sentences, header.event) : null;
      if (!effects) return null;
      replace = { replacement: "effects", effects };
    }
    // A following linked conditional or reflexive trigger can belong inside
    // the replacement body; reject until that scope is represented explicitly.
    const next = c.tokens[end + 1];
    if (next && !(next.kind === "word" && next.value === "activate")) return null;
    c.pos = end;
    c.punct(".");
    return { replacement: { event: header.event, occurrence, replace,
      ...(header.condition ? { condition: header.condition } : {}),
      ...(optional ? { optional: { who: isEventPlayer(optional.who, header.event)
        && ["that-player", "they"].includes(optional.who.player) ? eventPlayer("actor") : optional.who } } : {}) }, ...(duration ? { duration } : {}) };
  });
}

/** Invoke before the enclosing parser consumes an ordinary `if` condition.
 * This owns the complete replacement sentence, including its final period. */
export function parseReplacementSentence(c: Cursor, ctx: EffectContext, parseBody: ReplacementBodyParser, options: ReplacementOptions = {}): Sentence | null {
  return c.attempt(c => {
    const parsed = parseCore(c, ctx, parseBody, options);
    return parsed && { steps: [{ effect: "create-replacement", ...parsed }] };
  });
}

export function parseStaticReplacement(c: Cursor, parseBody: ReplacementBodyParser): StaticEffect | null {
  return c.attempt(c => {
    const parsed = parseCore(c, { modalOptions: [] }, parseBody, {});
    if (!parsed || !c.done() || parsed.replacement.occurrence === "next") return null;
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
