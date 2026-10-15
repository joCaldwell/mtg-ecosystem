// Quantity and exchange operations: doubling/tripling, counter distribution
// and movement, exchanges, power/toughness switches, coin flips and random
// selection. Each production returns a complete node or null.
// https://media.wizards.com/2026/downloads/MagicCompRules%2020260925.txt
import type {
  Amount, CoinFlipOutcome, CounterSpec, Effect, ExchangeValue, ObjectRef, PlayerRef, Sentence, ZoneRef,
} from "../ast.ts";
import type { Token } from "../lexer.ts";
import { wordNumber } from "../vocab.ts";
import { Cursor } from "./cursor.ts";
import {
  parseAmount, parseCount, parseCounterSpec, parseEqualTo, parseObjectRef, parseObjectSelection, parsePlayerRef, parsePossessiveObject,
  parsePossessivePlayer, targetSelectionProvenance,
} from "./refs.ts";

export interface QuantityGrammar {
  /** Actor of a verb: explicit player, or "you" for the imperative form. */
  actor(c: Cursor, imperative: string, conjugated: string): PlayerRef | null;
}

export function parseQuantityOp(c: Cursor, g: QuantityGrammar): Effect[] | null {
  return parseDouble(c) ?? parseSwitch(c) ?? parseExchange(c) ?? parseMoveCounters(c)
    ?? parseDistributeCounters(c) ?? parseFlip(c, g);
}

/** Effects whose duration is meaningful: they create continuous effects. */
export function quantityOpAcceptsDuration(effect: Effect): boolean {
  if (effect.effect === "multiply-pt" || effect.effect === "switch-pt") return true;
  // CR 701.12g: exchanging power/toughness creates a continuous effect.
  return effect.effect === "exchange-values" && effect.a.value !== "life-total" && effect.b.value !== "life-total";
}

// ---------------------------------------------------------------------------
// Doubling and tripling (CR 701.10, 701.11)
// ---------------------------------------------------------------------------

function parseDouble(c: Cursor): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    const verb = c.word("double", "triple");
    if (!verb) return null;
    const factor = verb as "double" | "triple";
    const pt = parseMultiplyPt(c, factor);
    if (pt) return [pt];
    // Life totals and counters are only defined for doubling; mana pools too.
    if (factor !== "double") return null;
    const life = c.attempt((c): Effect | null => {
      const who = parsePossessivePlayer(c);
      return who && c.words("life", "total") ? { effect: "double-life-total", who } : null;
    });
    if (life) return [life];
    if (c.words("the", "amount", "of", "each", "type", "of", "unspent", "mana", "you", "have"))
      return [{ effect: "double-mana-pool", who: { player: "you" } }];
    if (!c.words("the", "number", "of")) return null;
    const counter = c.attempt((c): CounterSpec | "each-kind" | null => {
      if (c.words("each", "kind", "of", "counter")) return "each-kind";
      const spec = parseCounterSpec(c);
      return spec && c.word("counter", "counters") !== null ? spec : null;
    });
    if (!counter || c.word("on") === null) return null;
    const on = parseObjectRef(c);
    return on ? [{ effect: "double-counters", on, counter }] : null;
  });
}

/** CR 701.10a-c: "double target creature's power", "double the power and
 * toughness of each creature you control", "double its power and toughness". */
function parseMultiplyPt(c: Cursor, factor: "double" | "triple"): Effect | null {
  return c.attempt((c): Effect | null => {
    const stat = (c: Cursor): "power" | "toughness" | "both" | null =>
      c.words("power", "and", "toughness") ? "both" : c.word("power", "toughness") as "power" | "toughness" | null;
    let what: ObjectRef | null = null;
    let which: "power" | "toughness" | "both" | null = null;
    if (c.words("the")) {
      which = stat(c);
      if (!which || c.word("of") === null) return null;
      what = parseObjectRef(c);
    } else {
      what = parsePossessiveObject(c);
      which = what && stat(c);
    }
    if (!what || !which) return null;
    // "X times": each repetition doubles the then-current value.
    const times = c.attempt((c): Amount | null => {
      const amount = parseAmount(c);
      return amount && c.word("times") !== null ? amount : null;
    });
    return { effect: "multiply-pt", what, stat: which, factor, ...(times ? { times } : {}) };
  });
}

// ---------------------------------------------------------------------------
// Switching power and toughness (CR 613.4d)
// ---------------------------------------------------------------------------

/** Plural possessive "each creature's": split at the possessive head. */
function parsePluralPossessiveObject(c: Cursor): ObjectRef | null {
  return c.attempt((c): ObjectRef | null => {
    if (!c.isWord("each", "all")) return null;
    let offset = 0;
    for (;;) {
      const token = c.peek(offset);
      if (!token || token.kind === "punct") return null;
      if (token.kind === "word" && token.possessive) break;
      offset++;
    }
    const head = c.peek(offset);
    if (head?.kind !== "word") return null;
    const sub = new Cursor([...c.tokens.slice(c.pos, c.pos + offset), { ...head, possessive: false }]);
    const ref = parseObjectRef(sub);
    if (!ref || !sub.done() || (ref.ref !== "each" && ref.ref !== "all")) return null;
    c.pos += offset + 1;
    return ref;
  });
}

function parseSwitch(c: Cursor): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    if (c.word("switch") === null) return null;
    let what: ObjectRef | null = null;
    if (c.words("the", "power", "and", "toughness", "of")) what = parseObjectRef(c);
    else {
      what = parsePossessiveObject(c) ?? parsePluralPossessiveObject(c);
      if (!what || !c.words("power", "and", "toughness")) return null;
    }
    return what ? [{ effect: "switch-pt", what }] : null;
  });
}

// ---------------------------------------------------------------------------
// Exchange (CR 701.12)
// ---------------------------------------------------------------------------

function parseExchange(c: Cursor): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    // "two target players exchange life totals" (subject-first).
    const twoPlayers = c.attempt((c): Effect | null => {
      const token = c.peek(1);
      if (!c.words("two", "target", "players") || !c.words("exchange", "life", "totals")) return null;
      return { effect: "exchange-life-totals", between: { players: "two-target", ...targetSelectionProvenance(token) } };
    });
    if (twoPlayers) return [twoPlayers];
    if (c.word("exchange") === null) return null;
    if (c.words("life", "totals", "with")) {
      const other = parsePlayerRef(c);
      return other ? [{ effect: "exchange-life-totals", between: [{ player: "you" }, other] }] : null;
    }
    if (c.words("control", "of")) {
      // Selections with group constraints: "two target permanents that share a card type".
      const first = parseObjectSelection(c);
      if (!first) return null;
      const second = c.attempt((c): ObjectRef | null => (c.word("and") !== null ? parseObjectSelection(c) : null));
      if (second) return [{ effect: "exchange-control", what: [first, second] }];
      // A single selection must be exactly two objects ("two target creatures").
      const exactlyTwo = first.ref === "target" && first.count.amount === "fixed" && first.count.value === 2 && !first.upTo;
      return exactlyTwo ? [{ effect: "exchange-control", what: [first] }] : null;
    }
    const zones = parseExchangeZones(c);
    if (zones) return [zones];
    const a = parseExchangeValue(c);
    if (!a) return null;
    if (c.word("with") !== null) {
      const b = parseExchangeValue(c);
      return b ? [{ effect: "exchange-values", a, b }] : null;
    }
    // "exchange its power and the power of target creature it's blocking"
    if (a.value !== "life-total" && c.words("and", "the", a.value)) {
      if (c.word("of") === null) return null;
      const of = parseObjectRef(c);
      return of ? [{ effect: "exchange-values", a, b: { value: a.value, of } }] : null;
    }
    return null;
  });
}

function parseExchangeValue(c: Cursor): ExchangeValue | null {
  return c.attempt((c): ExchangeValue | null => {
    const player = c.attempt(c => {
      const of = parsePossessivePlayer(c);
      return of && c.words("life", "total") ? of : null;
    });
    if (player) return { value: "life-total", of: player };
    const of = parsePossessiveObject(c);
    const stat = of && c.word("power", "toughness") as "power" | "toughness" | null;
    return of && stat ? { value: stat, of } : null;
  });
}

/** "Exchange your hand and graveyard": one possessive shared by both zones. */
function parseExchangeZones(c: Cursor): Effect | null {
  return c.attempt((c): Effect | null => {
    const zone = (c: Cursor) => c.word("hand", "graveyard", "library") as "hand" | "graveyard" | "library" | null;
    if (c.word("your") === null) return null;
    const first = zone(c);
    if (!first || c.word("and") === null) return null;
    const second = zone(c);
    if (!second || second === first) return null;
    const a: ZoneRef = { zone: first, owner: "your" }, b: ZoneRef = { zone: second, owner: "your" };
    return { effect: "exchange-zones", a, b };
  });
}

// ---------------------------------------------------------------------------
// Moving counters (CR 122.5)
// ---------------------------------------------------------------------------

function parseMoveCounters(c: Cursor): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    if (c.word("move") === null) return null;
    let count: Amount | "all" | null;
    if (c.word("all") !== null) count = "all";
    else if (c.words("any", "number", "of")) count = { amount: "chosen", minimum: 0 };
    else if (c.words("one", "or", "more")) count = { amount: "chosen", minimum: 1 };
    else count = parseCount(c);
    if (!count) return null;
    const spec = c.attempt(parseCounterSpec);
    if (c.word("counter", "counters") === null) return null;
    if (c.word("from") === null) return null;
    const from = parseObjectRef(c);
    if (!from) return null;
    if (c.word("onto", "to") === null) return null;
    // "a second target X" is a separate target selection that is contrasted
    // with the first; keep that printed contrast as the unresolved `other`
    // marker, exactly as "another target X" does.
    const second = c.words("a", "second");
    const to = parseObjectRef(c);
    if (!to) return null;
    if (second) {
      if (to.ref !== "target" || to.upTo || to.count.amount !== "fixed" || to.count.value !== 1) return null;
      to.filter.other = true;
    }
    // "from each/all": only a complete sweep has an unambiguous per-source meaning.
    if ((from.ref === "each" || from.ref === "all") && count !== "all") return null;
    if (to.ref === "each" || to.ref === "all") return null;
    return [{ effect: "move-counters", counter: spec ?? "any", count, from, to }];
  });
}

// ---------------------------------------------------------------------------
// Distributing counters (CR 601.2d, 608.2d)
// ---------------------------------------------------------------------------

function wordToken(value: string): Token {
  return { kind: "word", value, raw: value, possessive: false };
}

/** Target lists the shared selection grammar does not know: "one, two, or
 * three target creatures", "up to five targets". Delegates the noun phrase
 * to parseObjectRef by presenting it with "any number of", then restores the
 * printed count. */
export function parseDivisionTargets(c: Cursor): ObjectRef | null {
  return c.attempt((c): ObjectRef | null => {
    const upTo = c.attempt((c): ObjectRef | null => {
      if (!c.words("up", "to")) return null;
      const count = parseCount(c);
      const token = c.peek();
      if (!count || count.amount !== "fixed" || c.word("targets") === null) return null;
      return { ref: "any-target", count: { amount: "chosen", minimum: 0, maximum: count }, ...targetSelectionProvenance(token) };
    });
    if (upTo) return upTo;
    // "one, two, or three": a consecutive ascending list of at least three.
    const list = c.attempt((c): number | null => {
      let last = 0;
      for (;;) {
        const t = c.peek();
        const n = t?.kind === "word" && !t.possessive ? wordNumber(t.value) : undefined;
        if (n !== last + 1) return null;
        c.pos++; last = n;
        if (c.punct(",")) { c.word("or"); continue; }
        if (c.word("or") !== null) continue;
        return last >= 3 ? last : null;
      }
    });
    if (list === null) return parseObjectRef(c);
    const rest = c.tokens.slice(c.pos);
    const sub = new Cursor([wordToken("any"), wordToken("number"), wordToken("of"), ...rest]);
    const ref = parseObjectRef(sub);
    if (!ref || (ref.ref !== "target" && ref.ref !== "any-target")) return null;
    c.pos += sub.pos - 3;
    return { ...ref, count: { amount: "chosen", minimum: 1, maximum: { amount: "fixed", value: list } } };
  });
}

function parseDistributeCounters(c: Cursor): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    if (c.word("distribute") === null) return null;
    let count: Amount | null;
    let counter: CounterSpec | null;
    if (c.words("a", "number", "of")) {
      counter = parseCounterSpec(c);
      if (!counter || c.word("counter", "counters") === null) return null;
      count = parseEqualTo(c);
    } else {
      count = parseAmount(c);
      counter = count && parseCounterSpec(c);
      if (!counter || c.word("counter", "counters") === null) return null;
    }
    if (!count || c.word("among") === null) return null;
    const among = parseDivisionTargets(c);
    if (!among) return null;
    // Only selections with a chosen number of recipients can receive a division.
    if (among.ref !== "target" && among.ref !== "any-target" && among.ref !== "filter") return null;
    if (among.ref === "filter" && (among.count.amount !== "chosen" || among.upTo)) return null;
    return [{ effect: "distribute-counters", counter, count, among }];
  });
}

// ---------------------------------------------------------------------------
// Coin flips (CR 705)
// ---------------------------------------------------------------------------

function parseFlip(c: Cursor, g: QuantityGrammar): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    const who = g.actor(c, "flip", "flips");
    if (!who) return null;
    const count = parseCount(c);
    if (!count) return null;
    if (c.word(count.amount === "fixed" && count.value === 1 ? "coin" : "coins") === null) return null;
    if (count.amount === "fixed" && count.value < 1) return null;
    const until = c.attempt(c => (c.words("until", "you", "lose", "a", "flip") ? true : null));
    if (until && who.player !== "you") return null;
    return [{ effect: "flip-coin", who, count, ...(until ? { until: "lose" as const } : {}) }];
  });
}

/** One sentence token window ending at the first top-level full stop. */
function sentenceTokens(c: Cursor): Token[] | null {
  let quoted = false;
  for (let i = c.pos; i < c.tokens.length; i++) {
    const t = c.tokens[i];
    if (t.kind === "punct" && t.value === '"') quoted = !quoted;
    if (!quoted && t.kind === "punct" && t.value === ".") return c.tokens.slice(c.pos, i + 1);
  }
  return null;
}

const COMPLEMENT = { win: "lose", lose: "win", heads: "tails", tails: "heads" } as const;

/** Fold "If you win the flip, …" / "If you lose the flip, …" / "If it comes
 * up heads, …" / "Otherwise, …" into the flip they belong to (CR 705.2).
 * The caller guarantees `previous` is the sentence being continued. The
 * branch must parse completely or the whole line fails. */
export function foldCoinFlipOutcome(
  c: Cursor, previous: Sentence | undefined, parseBody: (body: Cursor) => Sentence[] | null,
): "folded" | "none" | null {
  let flip = previous?.steps.at(-1);
  // The flip scheduled by "flip a coin at the beginning of the next end
  // step" is made by the delayed ability; its outcomes belong inside it.
  if (flip?.effect === "create-delayed-trigger" && flip.effects.length === 1 && flip.effects[0].steps.length === 1
      && flip.effects[0].steps[0].effect === "flip-coin" && !flip.effects[0].otherwise) flip = flip.effects[0].steps[0];
  if (!previous || flip?.effect !== "flip-coin" || previous.otherwise) return "none";
  const header = c.attempt((c): CoinFlipOutcome["result"] | "otherwise" | null => {
    if (c.word("otherwise") !== null) { c.punct(","); return "otherwise"; }
    if (c.word("if") === null) return null;
    const self = flip.who.player === "you";
    // Called flips: only the flipping player wins or loses (CR 705.2).
    const called = c.attempt((c): "win" | "lose" | null => {
      const subject = c.word("you", "they") ?? (c.words("that", "player") ? "that player" : null);
      if (!subject) return null;
      if (self ? subject !== "you" : subject === "you") return null;
      const verb = c.word("win", "wins", "lose", "loses");
      if (!verb || !c.words("the", "flip") || !c.punct(",")) return null;
      return verb.startsWith("win") ? "win" : "lose";
    });
    if (called) return called;
    const result = c.attempt((c): "heads" | "tails" | null => {
      if (!c.words("it", "comes", "up") && !c.words("the", "coin", "comes", "up")) return null;
      const face = c.word("heads", "tails") as "heads" | "tails" | null;
      return face && c.punct(",") ? face : null;
    });
    return result;
  });
  if (!header) return "none";
  const simple = flip.count.amount === "fixed" && flip.count.value === 1 && !flip.until;
  if (!simple) return c.fail("outcome of a single coin flip");
  const body = sentenceTokens(c);
  if (!body) return null;
  const sub = new Cursor(body);
  const effects = parseBody(sub);
  if (!effects || !sub.done()) return null;
  c.pos += body.length;
  const outcomes = flip.outcomes ?? [];
  const last = outcomes.at(-1);
  const result = header === "otherwise" ? (last && outcomes.length === 1 ? COMPLEMENT[last.result] : null) : header;
  if (!result) return c.fail("complementary flip outcome");
  if (outcomes.some(o => o.result === result || COMPLEMENT[o.result] !== result)) return c.fail("one outcome per flip result");
  flip.outcomes = [...outcomes, { result, effects }];
  // A following anaphoric sentence would be read as continuing the branch.
  if (c.isWord("it", "they", "them", "that", "those", "its", "their", "then")) return c.fail("sentence independent of the flip outcome");
  return "folded";
}

// ---------------------------------------------------------------------------
// Random selection ("at random", "chosen at random")
// ---------------------------------------------------------------------------

/** Effects whose `what` (or damage recipient) denotes a selected object. */
function markRandom(effect: Effect): boolean {
  const mark = (ref: ObjectRef): boolean => {
    if ((ref.ref !== "filter" && ref.ref !== "from-pool" && ref.ref !== "target" && ref.ref !== "any-target") || ref.random) return false;
    if ("upTo" in ref && ref.upTo) return false;
    ref.random = true;
    return true;
  };
  switch (effect.effect) {
    case "reveal": case "exile": case "destroy": case "return-to-hand": case "move-zone": case "gain-control":
    case "tap": case "untap": case "look-at": case "sacrifice":
      return typeof effect.what === "object" && "ref" in effect.what ? mark(effect.what) : false;
    case "put-counters":
      return mark(effect.on);
    case "damage":
      if (effect.to.to === "any-target") { effect.to.random = true; return true; }
      return effect.to.to === "object" ? mark(effect.to.ref) : false;
    default:
      return false;
  }
}

/** "X at random", "X at random from Y", "X chosen at random": the game, not a
 * player, makes the selection. The selection phrase is parsed without the
 * random marker, then recorded on the selected reference (or, for "Choose X
 * at random", as a choose-random instruction). */
export function parseRandomSelection(c: Cursor, parseStep: (c: Cursor) => Effect[] | null): Effect[] | null {
  return c.attempt((c): Effect[] | null => {
    // Locate the marker inside the current clause.
    let at = -1, length = 0;
    for (let i = c.pos; i < c.tokens.length; i++) {
      const t = c.tokens[i];
      if (t.kind === "punct") break;
      const word = (offset: number, value: string) => {
        const u = c.tokens[i + offset];
        return u?.kind === "word" && !u.possessive && u.value === value;
      };
      if (word(0, "chosen") && word(1, "at") && word(2, "random")) { at = i; length = 3; break; }
      if (word(0, "at") && word(1, "random")) { at = i; length = 2; break; }
    }
    if (at < 0) return null;
    const rest = c.tokens.slice(at + length);
    const spliced = [...c.tokens.slice(c.pos, at), ...rest];
    const sub = new Cursor(spliced);
    const effects = parseStep(sub);
    const phraseEnd = at - c.pos;
    // The selection must end at, or extend past, the marker's position.
    if (!effects || effects.length !== 1 || sub.pos < phraseEnd) return null;
    const [effect] = effects;
    let result: Effect;
    if (effect.effect === "choose-object") {
      if (effect.who.player !== "you" || effect.optional) return null;
      result = { effect: "choose-random", what: effect.what };
    } else {
      if (!markRandom(effect)) return null;
      result = effect;
    }
    c.pos += sub.pos + length;
    return [result];
  });
}
