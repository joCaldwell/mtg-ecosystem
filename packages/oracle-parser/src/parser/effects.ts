// Effect sentence parsing: the imperative text of spells, trigger bodies,
// and activated-ability bodies.

import type {
  Ability, Amount, CantAction, DamageTarget, Effect, GainedAbility,
  KeywordInstance, ModalCount, ObjectRef, PlayerRef, Sentence, SignedAmount,
  StaticModifier, TokenSpec, ZoneRef,
} from "../ast.ts";
import type { Token } from "../lexer.ts";
import { CARD_TYPES, COLORS, SUPERTYPES, singularize, wordNumber } from "../vocab.ts";
import { Cursor } from "./cursor.ts";
import { parseSymbolCosts } from "./costs.ts";
import { parseKeyword } from "./keywords.ts";
import {
  canonicalSubtype, parseAmount, parseCondition, parseCount, parseCounterSpec,
  parseDuration, parseEqualTo, parseFilter, parseObjectRef, parsePlayerRef,
  parseZoneRef,
} from "./refs.ts";

// Late-bound full-line parser, used for quoted abilities ("gains '{T}: …'").
// Registered by parser/index.ts to avoid a hard import cycle.
let lineParser: ((tokens: Token[], options: Token[][]) => Ability | null) | null = null;
export function registerLineParser(fn: (tokens: Token[], options: Token[][]) => Ability | null): void {
  lineParser = fn;
}

export interface EffectContext {
  /** Modal bullet options (token lists), when the line owns "Choose one —" bullets. */
  modalOptions: Token[][];
  /** Subject of the previous step, for "…and gains flying" with elided subject. */
  lastSubject?: ObjectRef;
}

// ---------------------------------------------------------------------------
// Sentences
// ---------------------------------------------------------------------------

export function parseSentences(c: Cursor, ctx: EffectContext): Sentence[] | null {
  const sentences: Sentence[] = [];
  while (!c.done()) {
    // "Otherwise, …" attaches to the previous sentence.
    if (sentences.length > 0 && c.attempt((c) => (c.word("otherwise") !== null ? true : null))) {
      c.punct(",");
      const steps = parseSteps(c, ctx);
      if (!steps) return null;
      if (!c.punct(".") && !c.done()) return null;
      sentences[sentences.length - 1].otherwise = steps;
      continue;
    }
    const s = parseSentence(c, ctx);
    if (!s) break; // leave the cursor at the failed sentence; caller decides
    sentences.push(s);
  }
  return sentences.length ? sentences : null;
}

function parseSentence(c: Cursor, ctx: EffectContext): Sentence | null {
  return c.attempt((c): Sentence | null => {
    c.word("then");
    const sentence: Sentence = { steps: [] };

    // Leading duration: "Until end of turn, target creature gains flying."
    const leadingDuration = c.attempt((c) => {
      const d = parseDuration(c);
      if (!d || !c.punct(",")) return null;
      return d;
    });

    if (c.attempt((c) => (c.word("if") !== null ? true : null))) {
      const condition = parseConditionClause(c);
      if (!condition) return null;
      if (!c.punct(",")) return null;
      sentence.condition = condition;
      sentence.conditionKind = "if";
    }

    const steps = parseSteps(c, ctx);
    if (!steps) return null;
    if (leadingDuration) {
      for (const s of steps) if (!s.duration) s.duration = leadingDuration;
    }
    sentence.steps = steps;

    if (c.attempt((c) => (c.word("unless") !== null ? true : null))) {
      const condition = parseConditionClause(c);
      if (!condition) return null;
      sentence.condition = condition;
      sentence.conditionKind = "unless";
    }

    if (!c.punct(".") && !c.done()) return null;
    return sentence;
  });
}

function parseConditionClause(c: Cursor) {
  return parseCondition(c);
}

function parseSteps(c: Cursor, ctx: EffectContext): Effect[] | null {
  const steps: Effect[] = [];
  for (;;) {
    let parsed: Effect[] | null;
    let optional: boolean;
    if (steps.length === 0) {
      optional = c.attempt((c) => (c.words("you", "may") ? true : null)) === true;
      parsed = parseStep(c, ctx);
      if (!parsed) return null;
    } else {
      // Continuation requires a connector: ", (then|and)" / "then" / "and".
      const cont = c.attempt((c): { parsed: Effect[]; optional: boolean } | null => {
        if (c.isPunct(",")) {
          c.punct(",");
          if (c.word("then") === null) c.word("and");
        } else if (c.word("then") === null && c.word("and") === null) {
          return null;
        }
        const opt = c.attempt((c) => (c.words("you", "may") ? true : null)) === true;
        const p = parseStep(c, ctx);
        if (!p) return null;
        return { parsed: p, optional: opt };
      });
      if (!cont) break;
      parsed = cont.parsed;
      optional = cont.optional;
    }
    for (const e of parsed) {
      if (optional) e.optional = true;
      attachRiders(c, e);
      steps.push(e);
    }
  }
  return steps.length ? steps : null;
}

function attachRiders(c: Cursor, e: Effect): void {
  for (;;) {
    if (!e.forEach) {
      const forEach = c.attempt((c) => {
        if (!c.words("for", "each")) return null;
        return parseFilter(c);
      });
      if (forEach) {
        e.forEach = forEach;
        continue;
      }
    }
    if (!e.duration) {
      const duration = parseDuration(c);
      if (duration) {
        e.duration = duration;
        continue;
      }
    }
    break;
  }
}

// ---------------------------------------------------------------------------
// Individual effects
// ---------------------------------------------------------------------------

type StepParser = (c: Cursor, ctx: EffectContext) => Effect[] | null;

function parseStep(c: Cursor, ctx: EffectContext): Effect[] | null {
  for (const p of STEP_PARSERS) {
    const r = c.attempt((c) => p(c, ctx));
    if (r) return r;
  }
  return c.fail("effect");
}

/** "target creature and target land" → list; single ref → [ref]. */
function parseObjectRefList(c: Cursor): ObjectRef[] | null {
  const first = parseObjectRef(c);
  if (!first) return null;
  const refs = [first];
  while (
    c.attempt((c) => {
      if (c.word("and") === null) return null;
      const nxt = parseObjectRef(c);
      if (!nxt) return null;
      refs.push(nxt);
      return true;
    })
  ) { /* keep going */ }
  return refs;
}

const parseDraw: StepParser = (c) => {
  let who: PlayerRef = { player: "you" };
  const subject = c.attempt(parsePlayerRef);
  if (subject) who = subject;
  if (c.word("draw", "draws") === null) return null;
  const amount = c.attempt(parseCount) ?? null;
  if (!amount) return null;
  if (c.word("card", "cards") === null) return null;
  return [{ effect: "draw", who, amount }];
};

const parseDamage: StepParser = (c) => {
  const source = parseObjectRef(c);
  if (!source) return null;
  if (c.word("deal", "deals") === null) return null;
  let amount = c.attempt(parseAmount);
  if (c.word("damage") === null) return null;
  if (!amount) {
    amount = parseEqualTo(c);
    if (!amount) return null;
  }
  if (c.word("to") === null) return null;
  const to = parseDamageTarget(c);
  if (!to) return null;
  return [{ effect: "damage", source, amount, to }];
};

function parseDamageTarget(c: Cursor): DamageTarget | null {
  if (c.attempt((c) => (c.words("any", "target") ? true : null))) return { to: "any-target" };
  const each = c.attempt((c): DamageTarget | null => {
    if (c.word("each") === null) return null;
    if (c.word("opponent") !== null) return { to: "player", ref: { player: "each-opponent" } };
    if (c.word("player") !== null) return { to: "player", ref: { player: "each-player" } };
    const filter = parseFilter(c);
    if (!filter) return null;
    return { to: "object", ref: { ref: "each", filter } };
  });
  if (each) return each;
  // "target creature or player" / "target player or planeswalker" — try the
  // object path first when an "or" union is present.
  const player = c.attempt((c) => {
    const p = parsePlayerRef(c);
    if (!p || c.isWord("or")) return null;
    return p;
  });
  if (player) {
    const both = c.attempt((c): DamageTarget | null => {
      if (c.word("and") === null) return null;
      const second = c.attempt(parsePlayerRef) ?? parseObjectRef(c);
      if (!second) return null;
      return { to: "each", refs: [player, second] };
    });
    if (both) return both;
    return { to: "player", ref: player };
  }
  const obj = parseObjectRef(c);
  if (obj) {
    const both = c.attempt((c): DamageTarget | null => {
      if (c.word("and") === null) return null;
      const second = c.attempt(parsePlayerRef) ?? parseObjectRef(c);
      if (!second) return null;
      return { to: "each", refs: [obj, second] };
    });
    if (both) return both;
    return { to: "object", ref: obj };
  }
  return null;
}

const parseDestroy: StepParser = (c) => {
  if (c.word("destroy") === null) return null;
  const refs = parseObjectRefList(c);
  if (!refs) return null;
  return refs.map((what): Effect => ({ effect: "destroy", what }));
};

const parseExile: StepParser = (c) => {
  if (c.word("exile") === null) return null;
  const refs = parseObjectRefList(c);
  if (!refs) return null;
  return refs.map((what): Effect => ({ effect: "exile", what }));
};

const parseCounterSpell: StepParser = (c) => {
  if (c.word("counter") === null) return null;
  const what = parseObjectRef(c);
  if (!what) return null;
  return [{ effect: "counter", what }];
};

const parseReturn: StepParser = (c) => {
  if (c.word("return") === null) return null;
  const refs = parseObjectRefList(c);
  if (!refs) return null;
  if (c.word("to") === null) return null;
  const toHand = c.attempt((c): boolean | null => {
    if (c.words("its", "owner's")) { /* fallthrough */ }
    else if (c.word("their", "its") !== null) {
      if (c.possessive("owner", "owners") === null) return null;
    } else if (c.word("your") !== null) {
      // "return … to your hand"
    } else return null;
    if (c.word("hand", "hands") === null) return null;
    return true;
  });
  if (toHand) return refs.map((what): Effect => ({ effect: "return-to-hand", what }));
  if (c.attempt((c) => (c.words("the", "battlefield") ? true : null))) {
    const tapped = c.word("tapped") !== null;
    c.attempt((c) => {
      if (!c.words("under")) return null;
      c.word("your", "its", "their");
      c.possessive("owner", "owners");
      c.word("control");
      return true;
    });
    return refs.map(
      (what): Effect => ({ effect: "move-zone", what, to: { zone: "battlefield" }, tapped: tapped || undefined }),
    );
  }
  return null;
};

const parseCreateToken: StepParser = (c) => {
  if (c.word("create", "creates") === null) return null;
  const count = parseCount(c) ?? { amount: "fixed" as const, value: 1 };
  const token = parseTokenSpec(c);
  if (!token) return null;
  return [{ effect: "create-token", count, token }];
};

function parseTokenSpec(c: Cursor): TokenSpec | null {
  return c.attempt((c): TokenSpec | null => {
    const spec: TokenSpec = { colors: [], supertypes: [], types: [], subtypes: [], keywords: [] };

    // "1/1", "X/X"
    c.attempt((c) => {
      const p = c.number() ?? (c.word("x") !== null ? ("x" as const) : null);
      if (p === null || !c.punct("/")) return null;
      const t = c.number() ?? (c.word("x") !== null ? ("x" as const) : null);
      if (t === null) return null;
      spec.power = p === "x" ? { amount: "x" } : { amount: "fixed", value: p };
      spec.toughness = t === "x" ? { amount: "x" } : { amount: "fixed", value: t };
      return true;
    });

    // colors / supertypes / subtypes / types up to "token"
    for (;;) {
      const t = c.peek();
      if (!t || t.kind !== "word" || t.possessive) return null;
      if (t.value === "token" || t.value === "tokens") {
        c.pos++;
        break;
      }
      if (t.value === "and") {
        c.pos++;
        continue;
      }
      if (COLORS.has(t.value)) {
        c.pos++;
        spec.colors.push(t.value as TokenSpec["colors"][number]);
        continue;
      }
      if (t.value === "colorless") { c.pos++; continue; }
      if (SUPERTYPES.has(t.value)) { c.pos++; spec.supertypes.push(t.value); continue; }
      const singular = singularize(t.value);
      if (CARD_TYPES.has(singular)) { c.pos++; spec.types.push(singular); continue; }
      if (/^[A-Z]/.test(t.raw)) { c.pos++; spec.subtypes.push(canonicalSubtype(t.raw)); continue; }
      return null;
    }

    // "with flying", "with haste and deathtouch"
    c.attempt((c) => {
      if (!c.words("with")) return null;
      const kws = parseKeywordList(c);
      if (!kws) return null;
      spec.keywords.push(...kws);
      return true;
    });

    // "that's tapped and attacking"
    c.attempt((c) => {
      if (c.word("that's") === null) return null;
      for (;;) {
        if (c.word("tapped") !== null) spec.tapped = true;
        else if (c.word("attacking") !== null) spec.attacking = true;
        else break;
        if (c.word("and") === null) break;
      }
      return spec.tapped || spec.attacking ? true : null;
    });

    return spec;
  });
}

function parseKeywordList(c: Cursor): KeywordInstance[] | null {
  const kws: KeywordInstance[] = [];
  for (;;) {
    const kw = parseKeyword(c);
    if (!kw) return kws.length ? kws : null;
    kws.push(kw);
    if (c.isPunct(",")) {
      c.punct(",");
      c.word("and");
      continue;
    }
    if (c.attempt((c) => (c.word("and") !== null && !c.isPunct('"') ? true : null))) continue;
    break;
  }
  return kws;
}

const parseTapUntap: StepParser = (c) => {
  const verb = c.word("tap", "untap");
  if (!verb) return null;
  const refs = parseObjectRefList(c);
  if (!refs) return null;
  return refs.map((what): Effect => ({ effect: verb as "tap" | "untap", what }));
};

const parseSacrifice: StepParser = (c) => {
  let who: PlayerRef = { player: "you" };
  const subject = c.attempt(parsePlayerRef);
  if (subject) who = subject;
  if (c.word("sacrifice", "sacrifices") === null) return null;
  const what = parseObjectRef(c);
  if (!what) return null;
  return [{ effect: "sacrifice", who, what }];
};

const parseDiscard: StepParser = (c) => {
  let who: PlayerRef = { player: "you" };
  const subject = c.attempt(parsePlayerRef);
  if (subject) who = subject;
  if (c.word("discard", "discards") === null) return null;
  if (c.attempt((c) => (c.words("your", "hand") || c.words("their", "hand") || c.words("their", "hands") ? true : null)))
    return [{ effect: "discard", who, what: "hand" }];
  const what = parseObjectRef(c);
  if (!what) return null;
  const random = c.attempt((c) => (c.words("at", "random") ? true : null)) === true;
  return [{ effect: "discard", who, what, random: random || undefined }];
};

const parseMill: StepParser = (c) => {
  let who: PlayerRef = { player: "you" };
  const subject = c.attempt(parsePlayerRef);
  if (subject) who = subject;
  if (c.word("mill", "mills") === null) return null;
  const amount = parseCount(c);
  if (!amount) return null;
  if (c.word("card", "cards") === null) return null;
  return [{ effect: "mill", who, amount }];
};

const parseScrySurveil: StepParser = (c) => {
  const verb = c.word("scry", "surveil");
  if (!verb) return null;
  const amount = parseCount(c);
  if (!amount) return null;
  return [{ effect: verb as "scry" | "surveil", amount }];
};

const parsePut: StepParser = (c) => {
  if (c.word("put", "puts") === null) return null;

  // counters: "put a +1/+1 counter on target creature"
  const counters = c.attempt((c): Effect[] | null => {
    const count = parseCount(c);
    if (!count) return null;
    const counter = parseCounterSpec(c);
    if (!counter) return null;
    if (c.word("counter", "counters") === null) return null;
    if (c.word("on") === null) return null;
    const on = parseObjectRef(c);
    if (!on) return null;
    return [{ effect: "put-counters", counter, count, on }];
  });
  if (counters) return counters;

  // zone move: "put that card onto the battlefield tapped" / "into your graveyard"
  const refs = parseObjectRefList(c);
  if (!refs) return null;
  if (c.word("onto", "into", "on", "in") === null) return null;
  const to = parseZoneRef(c);
  if (!to) return null;
  const tapped = c.word("tapped") !== null;
  return refs.map((what): Effect => ({ effect: "move-zone", what, to, tapped: tapped || undefined }));
};

const parseAddMana: StepParser = (c) => {
  if (c.word("add", "adds") === null) return null;
  const readSymbols = (c: Cursor): string[] => {
    const out: string[] = [];
    let s: string | null;
    while ((s = c.attempt((c) => c.symbol())) !== null) out.push(s);
    return out;
  };
  if (c.peek()?.kind === "symbol") {
    const first = readSymbols(c);
    // "or" alternatives: "Add {R} or {G}."
    const options: string[][] = [first];
    while (c.attempt((c) => {
      const comma = c.isPunct(",");
      if (comma) c.punct(",");
      if (c.word("or") === null && !comma) return null;
      if (c.peek()?.kind !== "symbol") return null;
      options.push(readSymbols(c));
      return true;
    })) { /* keep going */ }
    if (options.length === 1) return [{ effect: "add-mana", mana: { mana: "fixed", symbols: first } }];
    return [{ effect: "add-mana", mana: { mana: "choice", options } }];
  }
  const amount = parseCount(c);
  if (!amount) return null;
  if (c.word("mana") === null) return null;
  if (c.words("of", "any", "one", "color"))
    return [{ effect: "add-mana", mana: { mana: "any-one-color", amount } }];
  if (c.words("of", "any", "color"))
    return [{ effect: "add-mana", mana: { mana: "any-color", amount } }];
  return null;
};

const parseSearch: StepParser = (c) => {
  let who: PlayerRef = { player: "you" };
  const subject = c.attempt(parsePlayerRef);
  if (subject) who = subject;
  if (c.word("search", "searches") === null) return null;
  const zoneRef = parseZoneRef(c);
  if (!zoneRef) return null;
  if (c.word("for") === null) return null;
  const what = parseObjectRef(c);
  if (!what) return null;
  return [{ effect: "search", who, zone: zoneRef.zone, for: what }];
};

const parseShuffle: StepParser = (c) => {
  let who: PlayerRef = { player: "you" };
  const subject = c.attempt(parsePlayerRef);
  if (subject) who = subject;
  if (c.word("shuffle", "shuffles") === null) return null;
  c.attempt((c) => (c.words("your", "library") || c.words("their", "library") ? true : null));
  return [{ effect: "shuffle", who }];
};

const parseLifeGainLoss: StepParser = (c) => {
  let who: PlayerRef = { player: "you" };
  const subject = c.attempt(parsePlayerRef);
  if (subject) who = subject;
  const verb = c.word("gain", "gains", "lose", "loses");
  if (!verb) return null;
  let amount = c.attempt(parseAmount);
  if (c.word("life") === null) return null;
  if (!amount) {
    amount = parseEqualTo(c);
    if (!amount) return null;
  }
  const effect = verb.startsWith("gain") ? "gain-life" : "lose-life";
  return [{ effect, who, amount }];
};

const parseGainControl: StepParser = (c) => {
  let who: PlayerRef = { player: "you" };
  const subject = c.attempt(parsePlayerRef);
  if (subject) who = subject;
  if (c.word("gain", "gains") === null) return null;
  if (!c.words("control", "of")) return null;
  const what = parseObjectRef(c);
  if (!what) return null;
  return [{ effect: "gain-control", who, what }];
};

function parseSignedAmount(c: Cursor): SignedAmount | null {
  const sign = c.punct("+") ? 1 : c.punct("-") ? -1 : null;
  if (sign === null) return null;
  const amount = parseAmount(c);
  if (!amount) return null;
  return { sign: sign as 1 | -1, amount };
}

/** "+1/+1", "-2/-0", "+X/+X" */
export function parsePtModifier(c: Cursor): { power: SignedAmount; toughness: SignedAmount } | null {
  return c.attempt((c) => {
    const power = parseSignedAmount(c);
    if (!power || !c.punct("/")) return null;
    const toughness = parseSignedAmount(c);
    if (!toughness) return null;
    return { power, toughness };
  });
}

/** keyword-or-quoted ability list after gains/has. */
export function parseGainedAbilities(c: Cursor): GainedAbility[] | null {
  const list: GainedAbility[] = [];
  for (;;) {
    const quoted = c.attempt((c): GainedAbility | null => {
      if (!c.punct('"')) return null;
      const inner: Token[] = [];
      for (;;) {
        const t = c.peek();
        if (!t) return null;
        if (t.kind === "punct" && t.value === '"') {
          c.pos++;
          break;
        }
        inner.push(t);
        c.pos++;
      }
      if (!lineParser) return null;
      const ability = lineParser(inner, []);
      if (!ability) return null;
      return { gained: "quoted", ability };
    });
    if (quoted) list.push(quoted);
    else {
      const kw = parseKeyword(c);
      if (!kw) return list.length ? list : null;
      list.push({ gained: "keyword", keyword: kw });
    }
    if (c.isPunct(",")) {
      c.punct(",");
      c.word("and");
      continue;
    }
    if (c.isWord("and")) {
      const cont = c.attempt((c) => {
        c.word("and");
        const t = c.peek();
        if (t?.kind === "punct" && t.value === '"') return true;
        if (t?.kind === "word") return true;
        return null;
      });
      if (cont) continue;
    }
    break;
  }
  return list;
}

/** Subject-first: "<obj> gets +1/+1" / "<obj> gains flying" / "<obj> has flying". */
const parsePumpOrGain: StepParser = (c, ctx) => {
  let what = c.attempt(parseObjectRef);
  if (!what) {
    // elided subject: "…and gains flying until end of turn"
    if (!ctx.lastSubject) return null;
    what = ctx.lastSubject;
  }
  const verb = c.word("get", "gets", "gain", "gains", "has", "have");
  if (!verb) return null;
  if (verb.startsWith("get")) {
    const pt = parsePtModifier(c);
    if (!pt) return null;
    ctx.lastSubject = what;
    return [{ effect: "pump", what, power: pt.power, toughness: pt.toughness }];
  }
  const abilities = parseGainedAbilities(c);
  if (!abilities) return null;
  ctx.lastSubject = what;
  return [{ effect: "gain-abilities", what, abilities }];
};

const parseBecome: StepParser = (c) => {
  const what = parseObjectRef(c);
  if (!what) return null;
  if (c.word("becomes", "become") === null) return null;
  c.word("a", "an");
  if (c.attempt((c) => (c.words("copy", "of") ? true : null))) {
    const copyOf = parseObjectRef(c);
    if (!copyOf) return null;
    return [{ effect: "become", what, spec: { copyOf } }];
  }
  // "a 4/4 green Bear creature (in addition to its other types)"
  const spec = parseTokenSpecLike(c);
  if (!spec) return null;
  const inAddition =
    c.attempt((c) => (c.words("in", "addition", "to", "its", "other") && c.anyWord() ? true : null)) === true;
  return [{ effect: "become", what, spec: { ...spec, inAddition: inAddition || undefined } }];
};

function parseTokenSpecLike(c: Cursor): { types?: string[]; subtypes?: string[]; colors?: TokenSpec["colors"]; power?: Amount; toughness?: Amount } | null {
  return c.attempt((c) => {
    const out: { types: string[]; subtypes: string[]; colors: TokenSpec["colors"]; power?: Amount; toughness?: Amount } = {
      types: [], subtypes: [], colors: [],
    };
    c.attempt((c) => {
      const p = c.number();
      if (p === null || !c.punct("/")) return null;
      const t = c.number();
      if (t === null) return null;
      out.power = { amount: "fixed", value: p };
      out.toughness = { amount: "fixed", value: t };
      return true;
    });
    let consumed = false;
    for (;;) {
      const t = c.peek();
      if (!t || t.kind !== "word" || t.possessive) break;
      if (COLORS.has(t.value)) { c.pos++; out.colors.push(t.value as TokenSpec["colors"][number]); consumed = true; continue; }
      const singular = singularize(t.value);
      if (CARD_TYPES.has(singular)) { c.pos++; out.types.push(singular); consumed = true; continue; }
      if (/^[A-Z]/.test(t.raw)) { c.pos++; out.subtypes.push(canonicalSubtype(t.raw)); consumed = true; continue; }
      if (t.value === "and") { c.pos++; continue; }
      break;
    }
    // "with base power and toughness 4/4"
    c.attempt((c) => {
      if (!c.words("with", "base", "power", "and", "toughness")) return null;
      const p = c.number();
      if (p === null || !c.punct("/")) return null;
      const t = c.number();
      if (t === null) return null;
      out.power = { amount: "fixed", value: p };
      out.toughness = { amount: "fixed", value: t };
      return true;
    });
    return consumed ? out : null;
  });
}

const parseReveal: StepParser = (c) => {
  let who: PlayerRef = { player: "you" };
  const subject = c.attempt(parsePlayerRef);
  if (subject) who = subject;
  if (c.word("reveal", "reveals") === null) return null;
  if (c.attempt((c) => (c.words("your", "hand") || c.words("their", "hand") ? true : null)))
    return [{ effect: "reveal", who, what: "hand" }];
  const what = parseObjectRef(c);
  if (!what) return null;
  return [{ effect: "reveal", who, what }];
};

const parseLook: StepParser = (c) => {
  let who: PlayerRef = { player: "you" };
  const subject = c.attempt(parsePlayerRef);
  if (subject) who = subject;
  if (c.word("look", "looks") === null) return null;
  if (c.word("at") === null) return null;
  const top = c.attempt((c): Effect[] | null => {
    if (!c.words("the", "top")) return null;
    const topN = c.attempt(parseCount) ?? { amount: "fixed" as const, value: 1 };
    c.word("card", "cards");
    if (c.word("of") === null) return null;
    const of = parseZoneRef(c);
    if (!of) return null;
    return [{ effect: "look-at", who, what: { top: topN, of } }];
  });
  if (top) return top;
  const what = parseObjectRef(c);
  if (!what) return null;
  return [{ effect: "look-at", who, what }];
};

const parseCopySpell: StepParser = (c) => {
  if (c.word("copy", "copies") === null) return null;
  const what = parseObjectRef(c);
  if (!what) return null;
  return [{ effect: "copy-spell", what }];
};

/** "(You may) choose new targets for the copy/it." — its own sentence in oracle text. */
const parseChooseNewTargets: StepParser = (c) => {
  if (!c.words("choose", "new", "targets", "for")) return null;
  if (c.words("the", "copy") || c.words("the", "copies") || c.word("it", "them") !== null)
    return [{ effect: "choose-new-targets" }];
  return null;
};

const parseRemoveCounters: StepParser = (c) => {
  if (c.word("remove", "removes") === null) return null;
  const count: Amount | "all" | null = c.word("all") !== null ? "all" : parseCount(c);
  if (!count) return null;
  const counter = parseCounterSpec(c);
  if (!counter) return null;
  if (c.word("counter", "counters") === null) return null;
  if (c.word("from") === null) return null;
  const from = parseObjectRef(c);
  if (!from) return null;
  return [{ effect: "remove-counters", counter, count, from }];
};

const parseRegenerate: StepParser = (c) => {
  if (c.word("regenerate", "regenerates") === null) return null;
  const what = parseObjectRef(c);
  if (!what) return null;
  return [{ effect: "regenerate", what }];
};

const parseFight: StepParser = (c) => {
  const a = parseObjectRef(c);
  if (!a) return null;
  if (c.word("fight", "fights") === null) return null;
  const b = parseObjectRef(c);
  if (!b) return null;
  return [{ effect: "fight", a, b }];
};

const parseCant: StepParser = (c) => {
  const what = parseObjectRef(c);
  if (!what) return null;
  if (c.word("can't") === null) return null;
  const action = parseCantAction(c);
  if (!action) return null;
  return [{ effect: "cant", what, action }];
};

export function parseCantAction(c: Cursor): CantAction | null {
  if (c.attempt((c) => (c.words("attack", "or", "block") ? true : null))) return "attack-or-block";
  if (c.word("attack") !== null) return "attack";
  if (c.word("block") !== null) return "block";
  if (c.attempt((c) => (c.words("be", "blocked") ? true : null))) return "be-blocked";
  if (c.attempt((c) => (c.words("be", "countered") ? true : null))) return "be-countered";
  if (c.attempt((c) => (c.words("untap", "during", "your", "untap", "step") ? true : null))) return "untap";
  if (c.attempt((c) => {
    if (!c.words("untap", "during")) return null;
    if (c.word("its", "their") === null) return null;
    if (c.possessive("controller", "controllers") === null && c.word("controller's", "controllers'") === null) return null;
    return c.words("untap", "step") || c.words("untap", "steps") ? true : null;
  })) return "untap";
  return c.fail("can't-action");
}

const parseModal: StepParser = (c, ctx) => {
  if (c.word("choose") === null) return null;
  const count = parseModalCount(c);
  if (!count) return null;
  if (!c.punct("—")) return null;
  if (!c.done()) return null;
  if (ctx.modalOptions.length === 0) return null;
  const options: Sentence[][] = [];
  for (const optTokens of ctx.modalOptions) {
    const oc = new Cursor(optTokens);
    const optCtx: EffectContext = { modalOptions: [] };
    const sentences = parseSentences(oc, optCtx);
    if (!sentences || !oc.done()) return null;
    options.push(sentences);
  }
  return [{ effect: "modal", count, options }];
};

function parseModalCount(c: Cursor): ModalCount | null {
  const upTo = c.attempt((c) => (c.words("up", "to") ? true : null)) === true;
  const t = c.peek();
  let n: number | null = null;
  if (t?.kind === "word" && wordNumber(t.value) !== undefined) {
    n = wordNumber(t.value)!;
    c.pos++;
  }
  if (n === null) return c.fail("modal count");
  if (upTo) return { min: 0, max: n };
  if (c.attempt((c) => (c.words("or", "more") ? true : null))) return { atLeast: n };
  if (c.attempt((c) => (c.words("or", "both") ? true : null))) return { min: n, max: 2 };
  return { exactly: n };
}

const parsePreventCombatDamage: StepParser = (c) => {
  if (!c.words("prevent", "all", "combat", "damage")) return null;
  if (!c.words("that", "would", "be", "dealt")) return null;
  const by = c.attempt((c): ObjectRef | null => {
    if (c.word("by") === null) return null;
    return parseObjectRef(c);
  });
  if (!by) {
    if (!c.words("this", "turn")) return null;
    return [{ effect: "prevent-combat-damage", duration: { duration: "this-turn" } }];
  }
  c.words("this", "turn");
  return [{ effect: "prevent-combat-damage", by, duration: { duration: "this-turn" } }];
};

// Order matters: subject-first parsers that begin with parseObjectRef go
// after verb-first parsers that could be mistaken for filters.
const STEP_PARSERS: StepParser[] = [
  parseModal,
  parseDraw,
  parseDestroy,
  parseExile,
  parseCounterSpell,
  parseReturn,
  parseCreateToken,
  parseTapUntap,
  parseSacrifice,
  parseDiscard,
  parseMill,
  parseScrySurveil,
  parsePut,
  parseRemoveCounters,
  parseAddMana,
  parseSearch,
  parseShuffle,
  parseLifeGainLoss,
  parseGainControl,
  parseReveal,
  parseLook,
  parseCopySpell,
  parseChooseNewTargets,
  parseRegenerate,
  parsePreventCombatDamage,
  parseDamage,
  parsePumpOrGain,
  parseBecome,
  parseFight,
  parseCant,
];
