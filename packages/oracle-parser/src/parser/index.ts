// Line-level classification and the card-level entry point.
//
// One oracle-text line = one ability. Classification order:
//   1. loyalty prefix ("+1:", "-X:", "0:")
//   2. ability word prefix ("Landfall — …")
//   3. triggered ("When/Whenever/At …")
//   4. activated (a top-level ":" splitting costs from effects)
//   5. keyword line ("Flying, ward {2}")
//   6. static ("Creatures you control get +1/+1.")
//   7. spell text (imperative sentences)
// Anything else fails loudly with the farthest-failure diagnostic.

import type { Ability, ParseCardResult, ParsedLine, Sentence } from "../ast.ts";
import { LexError, lex, type Token } from "../lexer.ts";
import { normalizeOracleText } from "../normalize.ts";
import { Cursor } from "./cursor.ts";
import { parseCostList } from "./costs.ts";
import { type EffectContext, parseSentences, registerLineParser } from "./effects.ts";
import { parseKeywordLine } from "./keywords.ts";
import { parseCondition } from "./refs.ts";
import { parseStatic } from "./statics.ts";
import { parseTrigger } from "./triggers.ts";

// ---------------------------------------------------------------------------

/** Words that end an effects block and become an activation restriction. */
function takeRestriction(c: Cursor): string | null {
  if (!c.isWord("activate")) return null;
  const rest: string[] = [];
  while (!c.done()) {
    const t = c.next()!;
    if (t.kind === "word") rest.push(t.raw);
    else if (t.kind === "number") rest.push(t.raw);
    else if (t.kind === "symbol") rest.push(`{${t.value}}`);
    else if (t.kind === "selfref") rest.push("~");
    else if (t.value !== ".") rest.push(t.value);
  }
  return rest.join(" ");
}

function parseEffectsBlock(c: Cursor, options: Token[][]): { sentences: Sentence[]; restriction?: string } | null {
  const ctx: EffectContext = { modalOptions: options };
  const sentences = parseSentences(c, ctx);
  if (!sentences) return null;
  if (c.done()) return { sentences };
  const restriction = takeRestriction(c);
  if (restriction && c.done()) return { sentences, restriction };
  return null;
}

function parseLineTokens(tokens: Token[], options: Token[][]): Ability | null {
  // 1. Loyalty ability
  const loyalty = tryLoyalty(tokens, options);
  if (loyalty) return loyalty;

  // 2. Ability word prefix: leading words followed by "—", where what follows
  //    parses as a full ability. (Modal headers also contain "—" but end with it.)
  const abilityWord = tryAbilityWord(tokens, options);
  if (abilityWord) return abilityWord;

  return parseLineCore(tokens, options, undefined);
}

function tryLoyalty(tokens: Token[], options: Token[][]): Ability | null {
  let sign: 1 | -1 | 0 = 0;
  let i = 0;
  const first = tokens[0];
  if (first?.kind === "punct" && (first.value === "+" || first.value === "-")) {
    sign = first.value === "+" ? 1 : -1;
    i = 1;
  }
  const num = tokens[i];
  const colon = tokens[i + 1];
  const isAmount =
    num && ((num.kind === "number") || (num.kind === "word" && num.value === "x"));
  if (!isAmount || colon?.kind !== "punct" || colon.value !== ":") return null;
  if (sign === 0 && !(num.kind === "number" && num.value === 0)) return null;
  const c = new Cursor(tokens.slice(i + 2));
  const block = parseEffectsBlock(c, options);
  if (!block) return null;
  return {
    kind: "loyalty",
    cost: { sign, amount: num.kind === "number" ? num.value : "x" },
    effects: block.sentences,
  };
}

function tryAbilityWord(tokens: Token[], options: Token[][]): Ability | null {
  // Find an early "—" (within the first 4 tokens), preceded only by words.
  let dash = -1;
  for (let i = 1; i <= 4 && i < tokens.length; i++) {
    const t = tokens[i];
    if (t.kind === "punct" && t.value === "—") {
      dash = i;
      break;
    }
    if (t.kind !== "word") return null;
  }
  if (dash < 1 || tokens[0].kind !== "word") return null;
  const word = tokens
    .slice(0, dash)
    .map((t) => (t.kind === "word" ? t.raw : ""))
    .join(" ");
  const rest = tokens.slice(dash + 1);
  if (rest.length === 0) return null;
  const ability = parseLineCore(rest, options, word);
  return ability;
}

function parseLineCore(tokens: Token[], options: Token[][], abilityWord: string | undefined): Ability | null {
  // "As an additional cost to cast this spell, <cost>."
  {
    const c = new Cursor(tokens);
    if (c.words("as", "an", "additional", "cost", "to", "cast", "this", "spell") && c.punct(",")) {
      const costs = parseCostList(c);
      c.punct(".");
      if (costs && c.done()) return { kind: "additional-cost", costs };
      return null;
    }
  }

  // 3. Triggered
  if (tokens[0]?.kind === "word" && ["when", "whenever", "at"].includes(tokens[0].value)) {
    const c = new Cursor(tokens);
    const trigger = parseTrigger(c);
    if (trigger && c.punct(",")) {
      // Intervening if: "When ~ dies, if <cond>, <effects>"
      const condition = c.attempt((cc) => {
        if (cc.word("if") === null) return null;
        const cond = parseCondition(cc);
        if (!cond || !cc.punct(",")) return null;
        return cond;
      });
      const block = parseEffectsBlock(c, options);
      if (block && !block.restriction) {
        return {
          kind: "triggered",
          abilityWord,
          trigger,
          condition: condition ?? undefined,
          effects: block.sentences,
        };
      }
    }
    return null;
  }

  // 4. Activated: top-level colon outside quotes.
  let depth = 0;
  let colonAt = -1;
  for (let i = 0; i < tokens.length; i++) {
    const t = tokens[i];
    if (t.kind === "punct" && t.value === '"') depth ^= 1;
    else if (depth === 0 && t.kind === "punct" && t.value === ":") {
      colonAt = i;
      break;
    } else if (depth === 0 && t.kind === "punct" && t.value === ".") break;
  }
  if (colonAt > 0) {
    const costCursor = new Cursor(tokens.slice(0, colonAt));
    const costs = parseCostList(costCursor);
    if (costs && costCursor.done()) {
      const c = new Cursor(tokens.slice(colonAt + 1));
      const block = parseEffectsBlock(c, options);
      if (block) {
        return {
          kind: "activated",
          abilityWord,
          costs,
          effects: block.sentences,
          restriction: block.restriction,
        };
      }
    }
    return null;
  }

  // 5. Keyword line
  {
    const c = new Cursor(tokens);
    const keywords = parseKeywordLine(c);
    if (keywords) return { kind: "keywords", keywords };
  }

  // 6. Static
  {
    const c = new Cursor(tokens);
    const effect = parseStatic(c);
    if (effect && c.done()) return { kind: "static", abilityWord, effect };
  }

  // 7. Spell text
  {
    const c = new Cursor(tokens);
    const block = parseEffectsBlock(c, options);
    if (block && !block.restriction) return { kind: "spell", abilityWord, effects: block.sentences };
  }

  return null;
}

registerLineParser(parseLineTokens);

// ---------------------------------------------------------------------------
// Card-level API
// ---------------------------------------------------------------------------

function isBulletLine(tokens: Token[]): boolean {
  return tokens[0]?.kind === "punct" && tokens[0].value === "•";
}

/** Diagnostic-bearing single-line parse. */
function parseLineWithDiagnostics(text: string, optionTokens: Token[][]): ParsedLine {
  let tokens: Token[];
  try {
    tokens = lex(text);
  } catch (e) {
    if (e instanceof LexError) {
      return { text, ok: false, error: `${e.message} at position ${e.position}` };
    }
    throw e;
  }
  const ability = parseLineTokens(tokens, optionTokens);
  if (ability) return { text, ok: true, ability };

  // Re-run the plausible paths end-to-end and report whichever got farthest,
  // so the diagnostic points at the actual gap (usually in the effect text),
  // not at backtracking noise inside an earlier clause.
  const candidates: Cursor[] = [];
  if (tokens[0]?.kind === "word" && ["when", "whenever", "at"].includes(tokens[0].value)) {
    const c = new Cursor(tokens);
    const trigger = parseTrigger(c);
    if (trigger && c.punct(",")) {
      c.attempt((cc) => {
        if (cc.word("if") === null) return null;
        const cond = parseCondition(cc);
        if (!cond || !cc.punct(",")) return null;
        return cond;
      });
      parseSentences(c, { modalOptions: optionTokens });
    }
    candidates.push(c);
  } else {
    const cStatic = new Cursor(tokens);
    parseStatic(cStatic);
    candidates.push(cStatic);
    const cSpell = new Cursor(tokens);
    parseSentences(cSpell, { modalOptions: optionTokens });
    candidates.push(cSpell);
  }
  const best = candidates.reduce((a, b) => (b.farthest > a.farthest ? b : a));
  return { text, ok: false, error: best.errorMessage() };
}

export function parseOracleText(text: string, cardName?: string): ParseCardResult {
  const lines = normalizeOracleText(text, cardName);
  const results: ParsedLine[] = [];

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    // Group modal bullet lines under their header line.
    const bullets: string[] = [];
    let j = i + 1;
    while (j < lines.length && lines[j].startsWith("•")) {
      bullets.push(lines[j]);
      j++;
    }
    if (bullets.length > 0) {
      let optionTokens: Token[][] | null = [];
      try {
        for (const b of bullets) {
          optionTokens.push(lex(b).slice(1)); // drop the bullet
        }
      } catch {
        optionTokens = null;
      }
      if (optionTokens) {
        const parsed = parseLineWithDiagnostics(line, optionTokens);
        // Represent the whole block as one line result covering header+bullets.
        results.push({ ...parsed, text: [line, ...bullets].join(" ") });
        i = j - 1;
        continue;
      }
    }

    results.push(parseLineWithDiagnostics(line, []));
  }

  const ok = results.every((r) => r.ok);
  return {
    name: cardName ?? "",
    ok,
    lines: results,
    abilities: ok ? results.map((r) => r.ability!) : undefined,
  };
}
