// Prevention shields are continuous effects with explicit expiry, when
// given (CR 615.1, 615.3, 615.7, 615.11). A static prevention ability does not
// gain an invented "this turn" lifetime.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type { Amount, Comparison, Condition, DamagePrevention, DamageSourceChoice, Duration, Effect, ObjectFilter, ObjectRef, PlayerRef, PreventionQuantity, Sentence, StaticEffect } from "../ast.ts";
import { Cursor } from "./cursor.ts";
import type { EffectContext } from "./effects.ts";
import { isPluralPlayerRef, parseAmount, parseCondition, parseDuration, parseFilter, parseObjectRef, parsePlayerRef } from "./refs.ts";
import { parseKeywordName } from "./keywords.ts";

type BodyParser = (c: Cursor, ctx: EffectContext) => Sentence[] | null;
interface Options { leadingDuration?: Duration }
const incoming: Amount = { amount: "prevention-event-quantity", quantity: "incoming" };
const prevented: Amount = { amount: "prevention-event-quantity", quantity: "prevented" };

function sourceChoice(c: Cursor): DamageSourceChoice | null {
  return c.attempt(c => {
    const ref = parseObjectRef(c);
    if (ref?.ref !== "filter" || ref.count.amount !== "fixed" || ref.count.value !== 1 || ref.upTo
      || !c.words("of", "your", "choice")) return null;
    const filter = structuredClone(ref.filter);
    if (c.words("of", "the", "chosen", "color")) filter.chosenColor = { choice: "chosen-value", category: "color", which: "chosen" };
    if (c.word("with") !== null) {
      const keyword = parseKeywordName(c);
      if (!keyword) return null;
      (filter.withAbility ??= []).push(keyword);
    }
    return { choice: "damage-source", who: { player: "you" }, filter, when: "effect-created",
      universe: "damage-sources", recheck: "when-damage-would-be-dealt" };
  });
}

export function pluralObject(ref: ObjectRef): boolean {
  return ["all", "each", "them", "zone-cards", "rest"].includes(ref.ref) || ref.ref === "that" && !!ref.plural
    || "number" in ref && ref.number === "plural" || "count" in ref && !!ref.count
      && (ref.count.amount !== "fixed" || ref.count.value !== 1 || "upTo" in ref && !!ref.upTo);
}

function recipients(c: Cursor): { refs: (ObjectRef | PlayerRef)[]; collective: boolean; distributive: boolean; unmarkedPlural: boolean } | null {
  return c.attempt(c => {
    const refs: (ObjectRef | PlayerRef)[] = [];
    let collective = false;
    let distributive = false;
    let unmarkedPlural = false;
    for (;;) {
      const oneOrMore = c.words("one", "or", "more");
      if (oneOrMore) collective = true;
      const each = c.isWord("each");
      if (each) distributive = true;
      // One established recipient in either domain, not that permanent
      // plus a newly matched player (Orim's Touch).
      let ref: ObjectRef | PlayerRef | null = c.words("that", "permanent", "or", "player")
        ? { ref: "that", filter: { cls: "permanent", orPlayer: true } }
        : c.word("players") !== null ? { player: "each-player" } : parseObjectRef(c) ?? parsePlayerRef(c);
      if (!ref && refs.length && c.word("player") !== null) ref = { player: "a-player" };
      if (!ref) return null;
      if (!oneOrMore && !each && ("ref" in ref ? pluralObject(ref) : isPluralPlayerRef(ref))) unmarkedPlural = true;
      if (oneOrMore && !("ref" in ref ? ref.ref === "each" || ref.ref === "all" : isPluralPlayerRef(ref))) return null;
      refs.push(ref);
      if (c.word("or", "and") === null) break;
      if (c.punct("/")) { if (c.word("or") === null) return null; }
    }
    return { refs, collective, distributive, unmarkedPlural };
  });
}

export type Match = Pick<DamagePrevention, "by" | "to" | "kind" | "incomingAmount" | "condition" | "eventScope"> & { duration?: Duration; unmarkedPluralRecipients?: true };
export function damageHeader(c: Cursor): Match | null {
  return c.attempt(c => {
    const result: Match = { kind: "all", eventScope: { sources: "simultaneous", recipients: "each" } };
    if (c.isWord("damage", "combat", "noncombat")) {
      result.kind = (c.word("combat", "noncombat") ?? "all") as Match["kind"];
      if (!c.words("damage", "would", "be", "dealt")) return null;
    } else {
      const from = sourceChoice(c) ?? parseObjectRef(c);
      if (!from || !c.word("would")) return null;
      // Plural prospective subjects may define a collective event (700.1).
      // Do not infer independent applications from a plural noun alone.
      if ("ref" in from && pluralObject(from)) return null;
      if (c.words("be", "dealt")) {
        if (!("ref" in from)) return null;
        if ((from.ref === "filter" || from.ref === "target")
          && (from.count.amount !== "fixed" || from.count.value !== 1 || from.upTo)) return null;
        result.to = [from];
        result.kind = (c.word("combat", "noncombat") ?? "all") as Match["kind"];
        if (!c.word("damage")) return null;
      } else {
        if (!c.word("deal")) return null;
        result.by = from;
        result.eventScope.sources = "each";
        if ("ref" in from && (from.ref === "filter" || from.ref === "target")
          && (from.count.amount !== "fixed" || from.count.value !== 1 || from.upTo)) return null;
        const quantity = c.attempt((c): Comparison | null => {
          const value = parseAmount(c);
          if (!value) return null;
          const op = c.words("or", "less") ? "le" : c.words("or", "more") ? "ge" : "eq";
          return { op, value };
        });
        if (quantity) result.incomingAmount = quantity;
        result.kind = (c.word("combat", "noncombat") ?? "all") as Match["kind"];
        if (!c.word("damage")) return null;
      }
    }
    for (;;) {
      if (!result.duration) {
        const duration = parseDuration(c);
        if (duration) { result.duration = duration; continue; }
      }
      if (!result.to && c.word("to") !== null) {
        const to = recipients(c);
        if (!to) return null;
        result.to = to.refs;
        if (to.collective) result.eventScope.recipients = "simultaneous";
        if (to.unmarkedPlural) result.unmarkedPluralRecipients = true;
        if (to.refs.some(ref => "ref" in ref && (ref.ref === "filter" || ref.ref === "target")
          && (ref.count.amount !== "fixed" || ref.count.value !== 1 || ref.upTo))) return null;
        continue;
      }
      if (!result.condition && c.word("while") !== null) {
        const condition = parseCondition(c);
        if (!condition) return null;
        result.condition = condition; continue;
      }
      break;
    }
    return result;
  });
}

function quantity(c: Cursor): PreventionQuantity | null {
  return c.attempt(c => {
    if (c.words("that", "damage")) return { prevent: "all" };
    if (c.word("half") !== null) {
      if (!c.words("that", "damage") || !c.punct(",") || !c.word("rounded")) return null;
      const round = c.word("up", "down");
      return round === "up" || round === "down" ? { prevent: "half", round } : null;
    }
    const allBut = c.words("all", "but");
    const amount = parseAmount(c);
    if (!amount || !c.words("of", "that", "damage")) return null;
    return { prevent: allBut ? "all-but" : "up-to", amount };
  });
}

function matchesNoun(filter: ObjectFilter, ref: ObjectRef): boolean {
  if (Object.keys(filter).length !== 1) return false;
  if ("filter" in ref) {
    if (filter.cls) return ref.filter.cls === filter.cls;
    return Boolean(filter.types?.length && filter.types.every(type => ref.filter.types?.includes(type)));
  }
  if ("noun" in ref) return filter.cls === ref.noun || filter.types?.length === 1 && filter.types[0] === ref.noun;
  return false;
}

/** Bind only instructions with known direct scopes. In particular, never
 * capture words inside a token's intrinsic abilities or a delayed trigger.
 * CR 615.5/12: "that many" in the prevention instruction denotes incoming
 * damage; an explicit prevented quantity/condition denotes actual prevention. */
function eventBindings(prevention: DamagePrevention) {
  const singleton = (ref: ObjectRef) => ["self", "this", "it", "enchanted", "equipped", "fortified"].includes(ref.ref)
    || ref.ref === "that" && !ref.plural || ref.ref === "any-target" && (!ref.count || ref.count.amount === "fixed" && ref.count.value === 1)
    || ref.ref === "target" && ref.count.amount === "fixed" && ref.count.value === 1;
  const candidates: { ref: ObjectRef; role: "source" | "recipient"; singleton: boolean }[] = [];
  if (prevention.by && "ref" in prevention.by) candidates.push({ ref: prevention.by, role: "source", singleton: singleton(prevention.by) });
  else if (prevention.by && "choice" in prevention.by) candidates.push({
    ref: { ref: "filter", filter: prevention.by.filter, count: { amount: "fixed", value: 1 } }, role: "source", singleton: true,
  });
  for (const ref of prevention.to ?? []) if ("ref" in ref) candidates.push({ ref, role: "recipient", singleton: singleton(ref) });
  const object = (ref: ObjectRef): ObjectRef | null => {
    if (ref.ref === "it" || ref.ref === "that") {
      const matching = candidates.filter(candidate => ref.ref === "it" || !ref.plural
        && (ref.filter.cls === "source" && Object.keys(ref.filter).length === 1 && candidate.role === "source"
          || matchesNoun(ref.filter, candidate.ref)));
      if (matching.length !== 1) return null;
      const match = matching[0], scope = match.role === "source" ? prevention.eventScope.sources : prevention.eventScope.recipients;
      return scope === "each" || match.singleton ? { ref: "prevention-event-object", role: match.role } : null;
    }
    if (ref.ref === "them") return null;
    if (ref.ref === "library-cards") {
      const count = amount(ref.count, incoming);
      return count && { ...ref, count };
    }
    // This source and explicit targets retain their lexical identity.
    return ref;
  };
  const player = (ref: PlayerRef): PlayerRef | null => {
    if (ref.player === "controller" || ref.player === "owner") {
      const of = object(ref.of);
      return of && { ...ref, of };
    }
    if (ref.player === "that-player" || ref.player === "they") {
      const to = prevention.to;
      return to?.length === 1 && "player" in to[0] && (prevention.eventScope.recipients === "each"
        || !isPluralPlayerRef(to[0]) && !["a-player", "an-opponent", "filtered"].includes(to[0].player))
        ? { player: "prevention-event-player", role: "recipient" } : null;
    }
    return ref;
  };
  const amount = (value: Amount, anaphor: Amount | null): Amount | null => {
    if (value.amount === "that-much") return anaphor;
    if (value.amount === "that-number") return null;
    if (value.amount === "twice" || value.amount === "half") {
      const of = amount(value.of, anaphor);
      return of && { ...value, of };
    }
    if (value.amount === "plus" || value.amount === "minus") {
      const a = amount(value.a, anaphor), b = amount(value.b, anaphor);
      return a && b && { ...value, a, b };
    }
    if (value.amount === "attribute") {
      const of = object(value.of);
      return of && { ...value, of };
    }
    if (value.amount === "counter-count") {
      const on = "ref" in value.on ? object(value.on) : player(value.on);
      return on && { ...value, on };
    }
    return value;
  };
  const condition = (value: Condition): Condition | null => {
    if (value.condition === "is-filter" || value.condition === "status") {
      const what = object(value.what);
      return what && { ...value, what };
    }
    if (value.condition === "damage-prevented") return value;
    return null;
  };
  return { object, player, amount, condition };
}

/** `reflexive`: the body of a "when damage is prevented this way" trigger,
 * where every "that much" is the prevented amount (CR 615.5, 603.12). */
function bindBody(body: Sentence[], prevention: DamagePrevention, reflexive = false): Sentence[] | null {
  const bind = eventBindings(prevention), output: Sentence[] = [];
  // A later anaphor must not jump over a new result (drawn cards, tokens,
  // exiled cards, or a new damage event) and silently bind to incoming damage.
  const prior = (prevention.after ?? []).flatMap(sentence => sentence.steps);
  let priorResults = prior.some(step => step.effect !== "remove-counters");
  let anaphorAvailable = prior.every(step => step.effect === "remove-counters"
    && step.count !== "all" && step.count.amount === "prevention-event-quantity" && step.count.quantity === "incoming");
  for (const sentence of body) {
    if (sentence.otherwise || sentence.bindings || sentence.condition && sentence.condition.condition !== "damage-prevented") return null;
    const steps: Effect[] = [];
    for (const step of sentence.steps) {
      if (priorResults) return null;
      const anaphor = sentence.condition || reflexive ? prevented : anaphorAvailable ? incoming : null;
      if (step.duration || step.forEach) return null;
      switch (step.effect) {
        case "draw": case "gain-life": case "mill": {
          const who = bind.player(step.who), amount = bind.amount(step.amount, anaphor);
          if (!who || !amount) return null;
          steps.push({ ...step, who, amount }); break;
        }
        case "put-counters": {
          const on = bind.object(step.on), count = bind.amount(step.count, anaphor), who = step.who && bind.player(step.who);
          if (!on || !count || step.who && !who) return null;
          steps.push({ ...step, on, count, ...(who && { who }) }); break;
        }
        case "remove-counters": {
          const from = bind.object(step.from), count = step.count === "all" ? "all" : bind.amount(step.count, anaphor);
          if (!from || !count) return null;
          steps.push({ ...step, from, count }); break;
        }
        case "create-token": {
          if ("copyOf" in step.token) return null;
          const count = bind.amount(step.count, anaphor), who = step.who && bind.player(step.who);
          if (!count || step.who && !who) return null;
          steps.push({ ...step, count, ...(who && { who }) }); break;
        }
        case "exile": {
          // Quantified top-library cards are one move of the printed count;
          // "a card from your graveyard for each 1 damage" is one choice of
          // that many cards from the acting player's own graveyard.
          if (step.what.ref !== "library-cards" && !(step.what.ref === "filter" && step.what.filter.cls === "card"
            && (step.what.filter.zone?.zone === "library" && step.what.filter.zone.position === "top"
              || step.what.filter.zone?.zone === "graveyard" && step.what.filter.zone.owner === "your" && !step.who
                && step.what.count.amount === "prevention-event-quantity"))) return null;
          const count = bind.amount(step.what.count, anaphor), who = step.who && bind.player(step.who);
          if (!count || step.who && !who) return null;
          steps.push({ ...step, what: { ...step.what, count }, ...(who && { who }) }); break;
        }
        case "damage": {
          const source = bind.object(step.source), amount = bind.amount(step.amount, anaphor);
          if (!source || !amount) return null;
          if (step.to.to === "player") {
            const ref = bind.player(step.to.ref);
            if (!ref) return null;
            steps.push({ ...step, source, amount, to: { ...step.to, ref } });
          } else if (step.to.to === "object") {
            const ref = bind.object(step.to.ref);
            if (!ref) return null;
            steps.push({ ...step, source, amount, to: { ...step.to, ref } });
          } else if (step.to.to === "any-target") steps.push({ ...step, source, amount });
          else return null;
          break;
        }
        default: return null;
      }
      priorResults = step.effect !== "remove-counters";
      if (step.effect === "remove-counters") anaphorAvailable = step.count !== "all" && (step.count.amount === "that-much"
        || step.count.amount === "prevention-event-quantity" && step.count.quantity === "incoming");
    }
    output.push({ ...sentence, steps });
  }
  return output;
}

function sentenceEnd(c: Cursor): number {
  let quoted = false;
  for (let i = c.pos; i < c.tokens.length; i++) {
    const token = c.tokens[i];
    if (token.kind === "punct" && token.value === '"') quoted = !quoted;
    if (!quoted && token.kind === "punct" && token.value === ".") return i;
  }
  return c.tokens.length;
}

function hasPreventionLink(c: Cursor): boolean {
  const end = sentenceEnd(c);
  return c.tokens.slice(c.pos, end).some((token, offset) => {
    const next = c.tokens[c.pos + offset + 1], last = c.tokens[c.pos + offset + 2];
    return token.kind === "word" && token.value === "prevented" && next?.kind === "word" && next.value === "this"
      && last?.kind === "word" && last.value === "way";
  });
}

/** "damage [from a red source] is prevented this way": the prevention
 * event of this same effect, optionally limited to the prevented damage's
 * source (CR 615.5). */
function preventedEvent(c: Cursor): { source?: ObjectFilter } | null {
  return c.attempt(c => {
    if (c.word("damage") === null) return null;
    let source: ObjectFilter | undefined;
    if (c.word("from") !== null) {
      if (c.word("a", "an") === null) return null;
      source = parseFilter(c) ?? undefined;
      if (!source) return null;
    }
    return c.words("is", "prevented", "this", "way") ? (source ? { source } : {}) : null;
  });
}

function parseRider(c: Cursor, ctx: EffectContext, parseBody: BodyParser, prevention: DamagePrevention, duration?: Duration): Sentence[] | null {
  return c.attempt(c => {
    const end = sentenceEnd(c), body = new Cursor(c.tokens.slice(c.pos, end));
    let condition: Condition | undefined;
    let reflexive: { source?: ObjectFilter } | undefined;
    const lead = body.word("if", "when", "whenever");
    if (lead !== null) {
      const event = preventedEvent(body);
      if (!event) return null;
      // CR 603.12: "When damage is prevented this way" is a triggered ability
      // created by each prevention; its own "this turn" can only restate a
      // shield that ends this turn.
      if (lead !== "if" && body.isWord("this") && !(duration && ["end-of-turn", "this-turn"].includes(duration.duration)
        && body.words("this", "turn"))) return null;
      if (!body.punct(",")) return null;
      if (lead === "if") condition = { condition: "damage-prevented", ...event };
      else reflexive = event;
    }
    let multiplier = body.attempt(c => c.words("for", "each") && c.number() === 1
      && c.words("damage", "prevented", "this", "way") && c.punct(",") ? prevented : null);
    let suffix = -1;
    if (!multiplier) {
      for (let i = body.pos; i < body.tokens.length; i++) {
        const probe = new Cursor(body.tokens.slice(i));
        if (probe.words("for", "each") && probe.number() === 1 && probe.words("damage", "prevented", "this", "way") && probe.done()) {
          suffix = i; multiplier = prevented; break;
        }
      }
    }
    const payload = new Cursor(body.tokens.slice(body.pos, suffix < 0 ? undefined : suffix));
    const parsed = parseBody(payload, { modalOptions: ctx.modalOptions });
    if (!parsed || !payload.done() || !parsed.length || condition && parsed.some(sentence => sentence.condition)) return null;
    if (multiplier) {
      if (parsed.length !== 1 || parsed[0].condition || parsed[0].steps.length !== 1) return null;
      const step = parsed[0].steps[0];
      // These wordings specify a simultaneous quantity, not repeated actions.
      if (step.optional || step.duration || step.forEach) return null;
      if (step.effect === "exile" && step.what.ref === "filter" && !step.what.upTo
        && step.what.count.amount === "fixed" && step.what.count.value === 1) step.what = { ...step.what, count: multiplier };
      else if ((step.effect !== "put-counters" && step.effect !== "create-token")
        || step.count.amount !== "fixed" || step.count.value !== 1) return null;
      else step.count = multiplier;
    }
    if (condition) parsed[0] = { ...parsed[0], condition, conditionKind: "if" };
    if (reflexive && (multiplier || parsed.some(sentence => sentence.condition))) return null;
    const bound = bindBody(parsed, prevention, reflexive !== undefined);
    if (!bound) return null;
    c.pos = end; c.punct(".");
    return reflexive ? [{ steps: [{ effect: "create-reflexive-trigger", event: { event: "damage-prevented", ...reflexive }, effects: bound }] }] : bound;
  });
}

function parseCore(c: Cursor, ctx: EffectContext, parseBody: BodyParser, options: Options, staticMode: boolean): Extract<Effect, { effect: "prevent-damage" }> | null {
  return c.attempt(c => {
    let duration = options.leadingDuration;
    if (!duration) duration = c.attempt(c => { const d = parseDuration(c); return d && c.punct(",") ? d : null; }) ?? undefined;
    const occurrence = c.words("the", "next", "time") ? "next" : c.word("if") !== null ? "each" : null;
    let prevention: DamagePrevention;
    if (occurrence) {
      let header: Match | null = null;
      for (let i = c.pos; i < c.tokens.length; i++) {
        const token = c.tokens[i];
        if (token.kind === "punct" && token.value === ".") break;
        if (token.kind !== "punct" || token.value !== ",") continue;
        const probe = new Cursor(c.tokens.slice(c.pos, i)), match = damageHeader(probe);
        if (match && probe.done()) { header = match; c.pos = i + 1; break; }
      }
      if (!header || duration && header.duration || occurrence === "each" && header.unmarkedPluralRecipients) return null;
      duration ??= header.duration;
      const optional = c.attempt(c => {
        const who = parsePlayerRef(c);
        return who && c.word("may") !== null ? { who } : null;
      });
      if (c.word("prevent") === null) return null;
      const prevent = quantity(c);
      if (!prevent) return null;
      const { duration: _duration, unmarkedPluralRecipients: _unmarked, ...match } = header;
      prevention = { shield: { shield: "events", occurrence, prevent }, ...match, ...(optional && { optional }) };
      if (occurrence === "next") prevention.eventScope.recipients = "simultaneous";
      if (prevention.condition) {
        const condition = eventBindings(prevention).condition(prevention.condition);
        if (!condition) return null;
        prevention.condition = condition;
      }
    } else {
      const steps = parsePrevention(c), effect = steps?.[0];
      if (!effect || effect.effect !== "prevent-damage" || duration && effect.duration) return null;
      duration ??= effect.duration;
      prevention = effect.prevention;
    }
    const binding = c.attempt(c => {
      if (!c.punct(",") || !c.words("where", "x", "is")) return null;
      return parseAmount(c);
    });
    if (binding) prevention.bindings = [{ variable: "x", value: binding,
      when: staticMode ? "event-application" : "effect-created" }];
    const coordinated = c.attempt(c => {
      if (!c.punct(",") && c.word("and") === null) return null;
      c.word("and");
      return parseRider(c, ctx, parseBody, prevention, duration);
    });
    if (coordinated) prevention.after = coordinated;
    else if (!c.punct(".") && !c.done()) return null;
    while (!c.done()) {
      const linked = hasPreventionLink(c);
      if (!staticMode && !linked) break;
      const after = parseRider(c, ctx, parseBody, prevention, duration);
      if (!after) return null;
      (prevention.after ??= []).push(...after);
    }
    return { effect: "prevent-damage", prevention, ...(duration && { duration }) };
  });
}

/** Own the whole prospective-event sentence before ordinary `if` parsing.
 * Target declarations in a rider remain declarations of the original spell
 * or ability (CR 601.2c); source-of-choice is selected when creating a shield. */
export function parsePreventionSentence(c: Cursor, ctx: EffectContext, parseBody: BodyParser, options: Options = {}): Sentence | null {
  return c.attempt(c => {
    const effect = parseCore(c, ctx, parseBody, options, false);
    return effect && { steps: [effect] };
  });
}

export function parsePrevention(c: Cursor): Effect[] | null {
  return c.attempt(c => {
    if (c.word("prevent") === null) return null;
    const clause = parseDamageClause(c);
    if (!clause) return null;
    // CR 601.2d: "divided as you choose" among the targets is announced with
    // them; each target gets a shield of its share (615.7), at least 1.
    const divided = c.attempt(c => c.punct(",") && c.words("divided", "as", "you", "choose") ? true : null);
    if (divided) {
      const shield = clause.prevention.shield, to = clause.prevention.to;
      if (shield.shield !== "damage-budget" || shield.allocation || to?.length !== 1 || !("ref" in to[0])
        || (to[0].ref !== "any-target" && to[0].ref !== "target")) return null;
      shield.allocation = "divided";
      clause.prevention.eventScope.recipients = "each";
    }
    return [{ effect: "prevent-damage", prevention: clause.prevention, ...(clause.duration ? { duration: clause.duration } : {}) }];
  });
}

/** "all / the next N [combat] damage that would be dealt …": the damage a
 * shield (615.7) or a redirection (614.9) applies to, with its expiry. */
export function parseDamageClause(c: Cursor): { prevention: DamagePrevention; duration?: Duration } | null {
  return c.attempt(c => {
    const amount = c.word("all") !== null ? "all" as const : c.words("the", "next") ? parseAmount(c) : null;
    if (!amount) return null;
    const kind = c.word("combat", "noncombat") as "combat" | "noncombat" | null;
    if (c.word("damage") === null) return null;
    let by: ObjectRef | DamageSourceChoice | null = null;
    if (!c.words("that", "would", "be", "dealt")) {
      c.word("that");
      by = sourceChoice(c) ?? parseObjectRef(c);
      if (!by || !c.words("would", "deal")) return null;
    }
    const prevention: DamagePrevention = { eventScope: { sources: "simultaneous", recipients: "simultaneous" }, shield: amount === "all"
      ? { shield: "events", occurrence: "each", prevent: { prevent: "all" } }
      : { shield: "damage-budget", amount }, kind: kind ?? "all", ...(by && { by }) };
    let duration: Duration | undefined;
    for (;;) {
      if (!duration) {
        const found = parseDuration(c);
        if (found) { duration = found; continue; }
      }
      if (!prevention.involving && !prevention.to && !prevention.by && c.words("to", "and", "dealt", "by")) {
        const what = parseObjectRef(c);
        if (!what) return null;
        prevention.involving = what;
        continue;
      }
      if (!prevention.to && !prevention.involving && c.word("to") !== null) {
        const to = recipients(c);
        if (!to) return null;
        prevention.to = to.refs;
        // A combined recipient set shares one budget (Divine Deflection).
        // A single universal quantifier creates separate shields (615.11).
        if (amount !== "all" && (to.refs.length === 1 || to.distributive) && !to.collective && to.refs.some(ref => "ref" in ref
          ? ref.ref === "all" || ref.ref === "each"
          : isPluralPlayerRef(ref)) && prevention.shield.shield === "damage-budget") {
          prevention.shield.allocation = "per-recipient";
          prevention.eventScope.recipients = "each";
        }
        continue;
      }
      if (!prevention.by && !prevention.involving && c.word("by") !== null) {
        const by = sourceChoice(c) ?? parseObjectRef(c);
        if (!by) return null;
        prevention.by = by;
        continue;
      }
      break;
    }
    return { prevention, ...(duration ? { duration } : {}) };
  });
}

function hasTarget(value: unknown): boolean {
  if (!value || typeof value !== "object") return false;
  if (Array.isArray(value)) return value.some(hasTarget);
  const object = value as Record<string, unknown>;
  // A reflexive ability announces its own targets (CR 603.12, 603.3d).
  if (object.effect === "create-reflexive-trigger") return false;
  if (object.to === "any-target" || object.ref === "target" || object.ref === "any-target" || object.player === "target-player" || object.player === "target-opponent") return true;
  return Object.values(object).some(hasTarget);
}

export function parseStaticPrevention(c: Cursor, parseBody: BodyParser): StaticEffect | null {
  // "Prevent all damage that would be dealt to ~ during your turn" states the
  // same continuously checked condition as a leading "During your turn,".
  const turnScoped = c.attempt((c): StaticEffect | null => {
    let end = c.tokens.length;
    const last = c.tokens[end - 1];
    if (last?.kind === "punct" && last.value === ".") end--;
    const tail = new Cursor(c.tokens.slice(end - 3, end));
    if (end - 3 <= c.pos || !tail.words("during", "your", "turn")) return null;
    const inner = new Cursor(c.tokens.slice(c.pos, end - 3));
    const effect = parseStaticPreventionClause(inner, parseBody);
    if (!effect || effect.static !== "prevent-damage" || !inner.done()) return null;
    c.pos = c.tokens.length;
    return { static: "conditional", condition: { condition: "your-turn" }, effect };
  });
  return turnScoped ?? parseStaticPreventionClause(c, parseBody);
}

function parseStaticPreventionClause(c: Cursor, parseBody: BodyParser): StaticEffect | null {
  return c.attempt(c => {
    const effect = parseCore(c, { modalOptions: [] }, parseBody, {}, true);
    if (!effect || effect.effect !== "prevent-damage" || (effect.duration && effect.duration.duration !== "as-long-as")
      || effect.prevention.shield.shield !== "events" || effect.prevention.shield.occurrence !== "each" || hasTarget(effect.prevention)
      || effect.prevention.by && "choice" in effect.prevention.by) return null;
    if (!effect.prevention.by && !effect.prevention.to && !effect.prevention.involving) return null;
    c.punct(".");
    if (!c.done()) return null;
    const prevention: StaticEffect = { static: "prevent-damage", prevention: effect.prevention };
    return effect.duration?.duration === "as-long-as" ? { static: "conditional", condition: effect.duration.condition, effect: prevention } : prevention;
  });
}
