// Shared reference grammar: amounts, filters, object/player refs, zones,
// durations, conditions, counters. Everything here is strictly typed and
// backtracking — a function either returns a complete node or null.

import type {
  Amount, Color, Comparison, Condition, CounterSpec, Duration, ObjectFilter,
  ObjectRef, PlayerRef, PropertyConstraint, ZoneRef,
} from "../ast.ts";
import {
  CARD_TYPES, COLORS, OBJECT_CLASSES, STATUS_WORDS, SUPERTYPES, ZONES,
  singularize, wordNumber,
} from "../vocab.ts";
import type { Cursor } from "./cursor.ts";

// ---------------------------------------------------------------------------
// Amounts
// ---------------------------------------------------------------------------

/** "a"/"an"/"three"/"3"/"X" — counting words used before nouns. */
export function parseCount(c: Cursor): Amount | null {
  const n = c.number();
  if (n !== null) return { amount: "fixed", value: n };
  if (c.word("a", "an") !== null) return { amount: "fixed", value: 1 };
  if (c.word("x") !== null) return { amount: "x" };
  const t = c.peek();
  if (t?.kind === "word" && !t.possessive) {
    const wn = wordNumber(t.value);
    if (wn !== undefined) {
      c.pos++;
      return { amount: "fixed", value: wn };
    }
  }
  return c.fail("count");
}

/** Amount in effect position: "3", "X", "twice X", "half your life total, rounded up" … */
export function parseAmount(c: Cursor): Amount | null {
  if (c.words("twice")) {
    const of = parseAmount(c);
    if (!of) return null;
    return { amount: "twice", of };
  }
  if (c.words("half")) {
    const of = parseAmount(c);
    if (!of) return null;
    let round: "up" | "down" = "down";
    c.attempt((c) => {
      c.punct(",");
      if (!c.words("rounded")) return null;
      const dir = c.word("up", "down");
      if (!dir) return null;
      round = dir as "up" | "down";
      return true;
    });
    return { amount: "half", of, round };
  }
  if (c.words("that", "much") || c.words("that", "many")) return { amount: "that-much" };
  const attr = c.attempt(parseAttributeAmount);
  if (attr) return attr;
  if (c.attempt((c) => (c.words("the", "number", "of") ? true : null))) {
    const of = parseFilter(c);
    if (!of) return null;
    return { amount: "count", of };
  }
  const base = c.attempt(parseCount);
  if (base) {
    // "X plus 1" and friends
    if (c.attempt((c) => (c.words("plus") ? true : null))) {
      const b = parseAmount(c);
      if (!b) return null;
      return { amount: "plus", a: base, b };
    }
    return base;
  }
  return c.fail("amount");
}

/** "its power", "~'s power", "your life total" */
function parseAttributeAmount(c: Cursor): Amount | null {
  if (c.words("your", "life", "total")) return { amount: "life-total", of: { player: "you" } };
  let of: ObjectRef | null = null;
  if (c.selfrefPossessive()) of = { ref: "self" };
  else if (c.word("its") !== null) of = { ref: "it" };
  if (!of) return null;
  const attr = c.words("mana", "value")
    ? "mana-value"
    : (c.word("power", "toughness") as "power" | "toughness" | null);
  if (!attr) return null;
  return { amount: "attribute", of, attribute: attr };
}

/** "equal to <amount>" tail used by damage/life effects. */
export function parseEqualTo(c: Cursor): Amount | null {
  if (!c.words("equal", "to")) return null;
  return parseAmount(c);
}

// ---------------------------------------------------------------------------
// Comparisons
// ---------------------------------------------------------------------------

/** "3 or greater", "2 or less", "X", "less than 4", "exactly 2" */
export function parseComparison(c: Cursor): Comparison | null {
  if (c.words("less", "than")) {
    const v = parseAmount(c);
    return v && { op: "lt", value: v };
  }
  if (c.words("greater", "than")) {
    const v = parseAmount(c);
    return v && { op: "gt", value: v };
  }
  if (c.words("exactly")) {
    const v = parseAmount(c);
    return v && { op: "eq", value: v };
  }
  const v = c.attempt(parseAmount);
  if (!v) return c.fail("comparison");
  if (c.attempt((c) => (c.word("or") !== null && c.word("greater", "more") !== null ? true : null)))
    return { op: "ge", value: v };
  if (c.attempt((c) => (c.word("or") !== null && c.word("less", "fewer") !== null ? true : null)))
    return { op: "le", value: v };
  return { op: "eq", value: v };
}

// ---------------------------------------------------------------------------
// Zones
// ---------------------------------------------------------------------------

/** "the battlefield", "your graveyard", "its owner's hand", "a graveyard" … */
export function parseZoneRef(c: Cursor): ZoneRef | null {
  return c.attempt((c): ZoneRef | null => {
    if (c.words("the", "battlefield")) return { zone: "battlefield" };
    let owner: ZoneRef["owner"];
    if (c.word("your") !== null) owner = "your";
    else if (c.word("their") !== null || c.word("its") !== null) {
      // "their owner's hand" / "its owner's graveyard"
      if (c.possessive("owner", "owners") !== null) owner = "its-owner";
      else owner = "their";
    } else if (c.attempt((c) => (c.word("an", "each") !== null && c.possessive("opponent") !== null ? true : null)))
      owner = "an-opponent";
    else if (c.attempt((c) => (c.word("each") !== null && c.possessive("player") !== null ? true : null)))
      owner = "each-player";
    else if (c.attempt((c) => (c.word("that") !== null && c.possessive("player") !== null ? true : null)))
      owner = "that-player";
    else if (c.word("a", "an", "any") !== null) owner = "any";
    else if (c.word("the") !== null) owner = undefined;
    const zoneWord = c.anyWord();
    if (!zoneWord) return null;
    const zone = singularize(zoneWord.value);
    if (!ZONES.has(zone)) return c.fail("zone");
    return { zone: zone as ZoneRef["zone"], owner };
  });
}

// ---------------------------------------------------------------------------
// Filters
// ---------------------------------------------------------------------------

function isCapitalized(raw: string): boolean {
  return /^[A-Z]/.test(raw);
}

type NominalClass = "type" | "class" | "subtype" | "player";

/** Canonical subtype form: singularized, first letter capitalized ("Elves" → "Elf"). */
export function canonicalSubtype(raw: string): string {
  const singular = singularize(raw.toLowerCase());
  return singular.charAt(0).toUpperCase() + singular.slice(1);
}

function classifyNominal(value: string, raw: string): { cls: NominalClass; word: string } | null {
  const singular = singularize(value);
  if (CARD_TYPES.has(singular)) return { cls: "type", word: singular };
  if (OBJECT_CLASSES.has(singular)) return { cls: "class", word: singular };
  if (singular === "player") return { cls: "player", word: "player" };
  if (isCapitalized(raw)) return { cls: "subtype", word: canonicalSubtype(raw) };
  return null;
}

/** May these two nominal classes appear in one or-list? ("creature or player") */
function orCompatible(a: NominalClass, b: NominalClass): boolean {
  if (a === "class" || b === "class") return false;
  if (a === b) return true;
  return a === "player" || b === "player";
}

/**
 * Structured card/object filter: "tapped artifact", "nonbasic land",
 * "instant or sorcery card in your graveyard", "Elves you control", …
 * Consumes at least one nominal (type/class/subtype) or fails.
 */
export function parseFilter(c: Cursor): ObjectFilter | null {
  return c.attempt((c): ObjectFilter | null => {
    const f: ObjectFilter = {};
    let sawNominal = false;

    const addColor = (color: Color) => {
      (f.colors ??= []).push(color);
    };

    // Prenominal + nominal loop.
    outer: for (;;) {
      // Comma between prenominal qualifiers: "noncreature, nonland permanent".
      if (c.isPunct(",") && !sawNominal) {
        const consumed = c.attempt((c) => {
          c.punct(",");
          const nxt = c.peek();
          if (nxt?.kind !== "word" || nxt.possessive) return null;
          const w = nxt.value;
          const qualifier =
            w.startsWith("non") || COLORS.has(w) || SUPERTYPES.has(w) || STATUS_WORDS.has(w);
          return qualifier ? true : null;
        });
        if (consumed) continue;
        break;
      }
      const t = c.peek();
      if (!t || t.kind !== "word" || t.possessive) break;
      const v = t.value;

      if (v === "other" || v === "another") {
        c.pos++;
        f.other = true;
        continue;
      }
      if (SUPERTYPES.has(v)) {
        c.pos++;
        (f.supertypes ??= []).push(v);
        continue;
      }
      if (STATUS_WORDS.has(v)) {
        c.pos++;
        (f.status ??= []).push(v);
        continue;
      }
      if (COLORS.has(v)) {
        c.pos++;
        addColor(v as Color);
        // "white or blue" / "red and green"
        while (c.attempt((c) => {
          if (c.word("or", "and") === null && !c.isPunct(",")) return null;
          if (c.isPunct(",")) {
            c.punct(",");
            c.word("or", "and");
          }
          const nxt = c.peek();
          if (nxt?.kind === "word" && COLORS.has(nxt.value)) {
            c.pos++;
            addColor(nxt.value as Color);
            return true;
          }
          return null;
        })) { /* keep consuming color list */ }
        continue;
      }
      if (v === "colorless") { c.pos++; f.colorless = true; continue; }
      if (v === "multicolored") { c.pos++; f.multicolored = true; continue; }
      if (v === "monocolored") { c.pos++; f.monocolored = true; continue; }
      if (v === "nontoken") { c.pos++; f.nonToken = true; continue; }
      if (v.startsWith("non")) {
        const rest = v.startsWith("non-") ? v.slice(4) : v.slice(3);
        const restSingular = singularize(rest);
        if (CARD_TYPES.has(restSingular)) {
          c.pos++;
          (f.nonTypes ??= []).push(restSingular);
          continue;
        }
        if (COLORS.has(rest)) {
          c.pos++;
          (f.nonColors ??= []).push(rest as Color);
          continue;
        }
        if (SUPERTYPES.has(rest)) {
          c.pos++;
          (f.nonSupertypes ??= []).push(rest);
          continue;
        }
        if (v.startsWith("non-") && isCapitalized(t.raw.slice(4))) {
          c.pos++;
          (f.nonSubtypes ??= []).push(canonicalSubtype(rest));
          continue;
        }
      }

      const nominal = classifyNominal(v, t.raw);
      if (nominal) {
        // "player" only heads a filter when part of an or-union
        // ("target player or planeswalker") — bare players are PlayerRefs.
        if (nominal.cls === "player") {
          const nxt = c.peek(1);
          if (!(nxt?.kind === "word" && nxt.value === "or")) break;
        }
        c.pos++;
        sawNominal = true;
        const record = (n: { cls: NominalClass; word: string }) => {
          if (n.cls === "type") (f.types ??= []).push(n.word);
          else if (n.cls === "subtype") (f.subtypes ??= []).push(n.word);
          else if (n.cls === "player") f.orPlayer = true;
          else f.cls = n.word as ObjectFilter["cls"];
        };
        record(nominal);

        // Or-joined nominals: "artifact or enchantment",
        // "instant, sorcery, or creature card", "creature or player".
        while (c.attempt((c) => {
          const comma = c.isPunct(",");
          if (comma) c.punct(",");
          if (c.word("or") === null && !comma) return null;
          const nt = c.peek();
          if (nt?.kind !== "word" || nt.possessive) return null;
          const n2 = classifyNominal(nt.value, nt.raw);
          if (!n2 || !orCompatible(nominal.cls, n2.cls)) return null;
          c.pos++;
          record(n2);
          return true;
        })) { /* keep consuming or-list */ }
        continue outer;
      }

      break;
    }

    if (!sawNominal) return c.fail("filter");

    // Postfix qualifiers, any order.
    for (;;) {
      if (c.attempt((c) => (c.words("you", "control") ? true : null))) {
        f.control = "you";
        continue;
      }
      if (c.attempt((c) => (c.words("you", "don't", "control") ? true : null))) {
        f.control = "not-you";
        continue;
      }
      if (
        c.attempt((c) =>
          c.words("an", "opponent", "controls") || c.words("your", "opponents", "control") ? true : null,
        )
      ) {
        f.control = "opponent";
        continue;
      }
      if (c.attempt((c) => (c.words("that", "player", "controls") ? true : null))) {
        f.control = "that-player";
        continue;
      }
      if (c.attempt((c) => (c.words("you", "own") ? true : null))) {
        f.own = "you";
        continue;
      }
      const zone = c.attempt((c): ZoneRef | null => {
        if (c.word("in", "from") === null) return null;
        return parseZoneRef(c);
      });
      if (zone) {
        f.zone = zone;
        continue;
      }
      if (c.attempt((c) => (c.words("on", "the", "battlefield") ? true : null))) {
        f.zone = { zone: "battlefield" };
        continue;
      }
      const prop = c.attempt(parsePropertyConstraint);
      if (prop) {
        (f.properties ??= []).push(prop);
        continue;
      }
      const withAbility = c.attempt((c): string | null => {
        if (!c.words("with")) return null;
        const kw = c.word(
          "flying", "vigilance", "haste", "trample", "deathtouch", "lifelink",
          "defender", "reach", "menace", "flash", "hexproof", "indestructible",
        );
        return kw;
      });
      if (withAbility) {
        (f.withAbility ??= []).push(withAbility);
        continue;
      }
      break;
    }

    return f;
  });
}

/** "with power 2 or less", "with mana value 3 or greater" */
function parsePropertyConstraint(c: Cursor): PropertyConstraint | null {
  if (!c.words("with") && !c.words("of")) return null;
  let property: PropertyConstraint["property"];
  if (c.words("mana", "value")) property = "mana-value";
  else {
    const w = c.word("power", "toughness");
    if (!w) return null;
    property = w as "power" | "toughness";
  }
  const comparison = parseComparison(c);
  if (!comparison) return null;
  return { property, comparison };
}

// ---------------------------------------------------------------------------
// Player refs
// ---------------------------------------------------------------------------

export function parsePlayerRef(c: Cursor): PlayerRef | null {
  return c.attempt((c): PlayerRef | null => {
    if (c.words("each", "player")) return { player: "each-player" };
    if (c.words("each", "opponent")) return { player: "each-opponent" };
    if (c.words("target", "player")) return { player: "target-player" };
    if (c.words("target", "opponent")) return { player: "target-opponent" };
    if (c.words("an", "opponent")) return { player: "an-opponent" };
    if (c.words("that", "player")) return { player: "that-player" };
    if (c.words("defending", "player")) return { player: "defending-player" };
    if (c.word("you") !== null) return { player: "you" };
    if (c.word("its") !== null) {
      if (c.word("controller") !== null) return { player: "controller", of: { ref: "it" } };
      if (c.word("owner") !== null) return { player: "owner", of: { ref: "it" } };
      return null;
    }
    if (c.selfrefPossessive()) {
      if (c.word("controller") !== null) return { player: "controller", of: { ref: "self" } };
      if (c.word("owner") !== null) return { player: "owner", of: { ref: "self" } };
      return null;
    }
    return c.fail("player");
  });
}

// ---------------------------------------------------------------------------
// Object refs
// ---------------------------------------------------------------------------

const THIS_NOUNS = new Set([
  "creature", "spell", "card", "permanent", "artifact", "enchantment", "land",
  "token", "aura", "equipment", "planeswalker",
]);

export function parseObjectRef(c: Cursor): ObjectRef | null {
  return c.attempt((c): ObjectRef | null => {
    if (c.selfref()) return { ref: "self" };
    if (c.word("it", "itself") !== null) return { ref: "it" };
    if (c.word("them") !== null) return { ref: "them" };

    if (c.attempt((c) => (c.words("any", "target") ? true : null))) return { ref: "any-target" };

    const thisRef = c.attempt((c): ObjectRef | null => {
      if (c.word("this") === null) return null;
      const noun = c.anyWord();
      if (!noun || !THIS_NOUNS.has(noun.value)) return null;
      return { ref: "this", noun: noun.value };
    });
    if (thisRef) return thisRef;

    const thatRef = c.attempt((c): ObjectRef | null => {
      if (c.word("that", "those") === null) return null;
      const filter = parseFilter(c);
      if (!filter) return null;
      return { ref: "that", filter };
    });
    if (thatRef) return thatRef;

    const targetRef = c.attempt((c): ObjectRef | null => {
      const upTo = c.words("up", "to");
      const count = c.attempt(parseCount) ?? { amount: "fixed" as const, value: 1 };
      if (upTo && count.amount === "fixed" && count.value === 1 && !c.isWord("target")) return null;
      if (c.word("target") === null) return null;
      const filter = parseFilter(c);
      if (!filter) return null;
      return upTo ? { ref: "target", filter, count, upTo: true } : { ref: "target", filter, count };
    });
    if (targetRef) return targetRef;

    const eachRef = c.attempt((c): ObjectRef | null => {
      if (c.word("each") === null) return null;
      const filter = parseFilter(c);
      if (!filter) return null;
      return { ref: "each", filter };
    });
    if (eachRef) return eachRef;

    const allRef = c.attempt((c): ObjectRef | null => {
      if (c.word("all") === null) return null;
      const filter = parseFilter(c);
      if (!filter) return null;
      return { ref: "all", filter };
    });
    if (allRef) return allRef;

    const equippedRef = c.attempt((c): ObjectRef | null => {
      const kind = c.word("equipped", "enchanted");
      if (!kind) return null;
      const noun = c.anyWord();
      if (!noun || !THIS_NOUNS.has(noun.value)) return null;
      return { ref: kind as "equipped" | "enchanted", noun: noun.value };
    });
    if (equippedRef) return equippedRef;

    const countedRef = c.attempt((c): ObjectRef | null => {
      const upTo = c.words("up", "to");
      const count = parseCount(c);
      if (!count) return null;
      const filter = parseFilter(c);
      if (!filter) return null;
      return upTo ? { ref: "filter", filter, count, upTo: true } : { ref: "filter", filter, count };
    });
    if (countedRef) return countedRef;

    // Bare plural filter: "creatures you control" — semantically "each".
    const bare = parseFilter(c);
    if (bare) return { ref: "each", filter: bare };

    return c.fail("object");
  });
}

// ---------------------------------------------------------------------------
// Durations
// ---------------------------------------------------------------------------

export function parseDuration(c: Cursor): Duration | null {
  return c.attempt((c): Duration | null => {
    if (c.words("until", "end", "of", "turn")) return { duration: "end-of-turn" };
    if (c.words("until", "your", "next", "turn")) return { duration: "your-next-turn" };
    if (c.words("this", "turn")) return { duration: "this-turn" };
    if (c.words("for", "as", "long", "as") || c.words("as", "long", "as")) {
      const condition = parseCondition(c);
      if (!condition) return null;
      return { duration: "as-long-as", condition };
    }
    return c.fail("duration");
  });
}

// ---------------------------------------------------------------------------
// Conditions
// ---------------------------------------------------------------------------

export function parseCondition(c: Cursor): Condition | null {
  return c.attempt((c): Condition | null => {
    if (c.words("you", "do", "so") || c.words("you", "do")) return { condition: "you-do" };
    if (c.words("you", "don't")) return { condition: "you-dont" };
    if (c.words("it's", "your", "turn")) return { condition: "your-turn" };
    if (c.words("it's", "not", "your", "turn")) return { condition: "not-your-turn" };
    if (c.words("it", "is", "your", "turn")) return { condition: "your-turn" };

    const control = c.attempt((c): Condition | null => {
      let who: PlayerRef | null = null;
      if (c.word("you") !== null) who = { player: "you" };
      else if (c.words("an", "opponent")) who = { player: "an-opponent" };
      else if (c.words("that", "player")) who = { player: "that-player" };
      if (!who) return null;
      if (c.word("control", "controls") === null) return null;
      const what = parseObjectRef(c);
      if (!what) return null;
      return { condition: "control", who, what };
    });
    if (control) return control;

    const life = c.attempt((c): Condition | null => {
      const who = parsePlayerRef(c);
      if (!who) return null;
      if (c.word("have", "has") === null) return null;
      const comparison = parseComparison(c);
      if (!comparison) return null;
      if (c.word("life") === null) return null;
      return { condition: "life-total", who, comparison };
    });
    if (life) return life;

    const pays = c.attempt((c): Condition | null => {
      const who = parsePlayerRef(c);
      if (!who) return null;
      if (c.word("pays") === null) return null;
      // Mana symbols or "N life".
      const symbols: string[] = [];
      let s;
      while ((s = c.attempt((c) => c.symbol())) !== null) symbols.push(s);
      if (symbols.length > 0) return { condition: "pays", who, cost: [{ cost: "mana", symbols }] };
      const amount = parseAmount(c);
      if (amount && c.word("life") !== null)
        return { condition: "pays", who, cost: [{ cost: "pay-life", amount }] };
      return null;
    });
    if (pays) return pays;

    const isFilter = c.attempt((c): Condition | null => {
      let what: ObjectRef | null = null;
      if (c.word("it's") !== null) what = { ref: "it" };
      else {
        what = parseObjectRef(c);
        if (!what) return null;
        if (c.word("is", "was") === null) return null;
      }
      c.word("a", "an");
      const status = c.attempt((c) => {
        const w = c.anyWord();
        return w && STATUS_WORDS.has(w.value) ? w.value : null;
      });
      if (status) return { condition: "status", what, status };
      const filter = parseFilter(c);
      if (!filter) return null;
      return { condition: "is-filter", what, filter };
    });
    if (isFilter) return isFilter;

    const remains = c.attempt((c): Condition | null => {
      const what = parseObjectRef(c);
      if (!what) return null;
      if (c.word("remains") === null) return null;
      c.word("on", "in");
      const zone = parseZoneRef(c);
      if (!zone) return null;
      return { condition: "remains", what, zone };
    });
    if (remains) return remains;

    return c.fail("condition");
  });
}

// ---------------------------------------------------------------------------
// Counters
// ---------------------------------------------------------------------------

/** "+1/+1", "-1/-1", "charge", "first strike" … (the words before "counter"). */
export function parseCounterSpec(c: Cursor): CounterSpec | null {
  return c.attempt((c): CounterSpec | null => {
    const pt = c.attempt((c): string | null => {
      const s1 = c.punct("+") ? "+" : c.punct("-") ? "-" : null;
      if (!s1) return null;
      const n1 = c.number();
      if (n1 === null || !c.punct("/")) return null;
      const s2 = c.punct("+") ? "+" : c.punct("-") ? "-" : null;
      if (!s2) return null;
      const n2 = c.number();
      if (n2 === null) return null;
      return `${s1}${n1}/${s2}${n2}`;
    });
    if (pt) return { type: pt };

    // Named counter: words until "counter(s)". Allow up to three words.
    const words: string[] = [];
    for (let i = 0; i < 3; i++) {
      if (c.isWord("counter", "counters")) break;
      const w = c.anyWord();
      if (!w) return null;
      words.push(w.value);
    }
    if (words.length === 0 || !c.isWord("counter", "counters")) return c.fail("counter type");
    return { type: words.join(" ") };
  });
}
