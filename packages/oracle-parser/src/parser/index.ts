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

import type { Ability, ActivationRestriction, ParseCardResult, ParsedLine, Sentence } from "../ast.ts";
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

class LineSession {
  readonly tokens: Token[];
  readonly cursors: { cursor: Cursor; offset: number }[] = [];
  constructor(tokens: Token[]) { this.tokens = tokens; }
  cursor(tokens: Token[]): Cursor {
    const cursor = new Cursor(tokens);
    const offset = tokens.length ? this.tokens.indexOf(tokens[0]) : this.tokens.length;
    this.cursors.push({ cursor, offset });
    return cursor;
  }
  error(): string {
    const best = this.cursors.reduce((a, b) =>
      b.offset + b.cursor.farthest > a.offset + a.cursor.farthest ? b : a);
    return best.cursor.errorMessage(best.offset);
  }
}

function takeRestriction(c: Cursor): ActivationRestriction | null {
  return c.attempt((c) => {
    if (c.word("activate") === null) return null;
    let restriction: ActivationRestriction;
    if (c.words("only", "as", "a", "sorcery")) restriction = { restriction: "sorcery" };
    else if (c.words("only", "once", "each", "turn")) restriction = { restriction: "once-each-turn" };
    else return c.fail("supported activation restriction");
    c.punct(".");
    if (!c.done()) return c.fail("end of restriction");
    return restriction;
  });
}

function parseEffectsBlock(c: Cursor, options: Token[][]): { sentences: Sentence[]; restriction?: ActivationRestriction } | null {
  return c.attempt((c): { sentences: Sentence[]; restriction?: ActivationRestriction } | null => {
    const ctx: EffectContext = { modalOptions: options };
    const sentences = parseSentences(c, ctx);
    if (!sentences) return null;
    if (c.done()) return { sentences };
    const restriction = takeRestriction(c);
    if (restriction && c.done()) return { sentences, restriction };
    return null;
  });
}

function parseLineTokens(tokens: Token[], options: Token[][], session = new LineSession(tokens)): Ability | null {
  // 1. Loyalty ability
  const loyalty = tryLoyalty(tokens, options, session);
  if (loyalty) return loyalty;

  // 2. Ability word prefix: leading words followed by "—", where what follows
  //    parses as a full ability. (Modal headers also contain "—" but end with it.)
  const abilityWord = tryAbilityWord(tokens, options, session);
  if (abilityWord) return abilityWord;

  return parseLineCore(tokens, options, undefined, session);
}

function tryLoyalty(tokens: Token[], options: Token[][], session: LineSession): Ability | null {
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
  const c = session.cursor(tokens.slice(i + 2));
  const block = parseEffectsBlock(c, options);
  if (!block) return null;
  return {
    kind: "loyalty",
    cost: { sign, amount: num.kind === "number" ? num.value : "x" },
    effects: block.sentences,
    ...(block.restriction ? { restriction: block.restriction } : {}),
  };
}

function tryAbilityWord(tokens: Token[], options: Token[][], session: LineSession): Ability | null {
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
  const ability = parseLineCore(rest, options, word, session);
  return ability;
}

function parseLineCore(tokens: Token[], options: Token[][], abilityWord: string | undefined, session: LineSession): Ability | null {
  // "As an additional cost to cast this spell, <cost>."
  {
    const c = session.cursor(tokens);
    if (c.words("as", "an", "additional", "cost", "to", "cast", "this", "spell") && c.punct(",")) {
      const costs = parseCostList(c);
      c.punct(".");
      if (costs && c.done()) return { kind: "additional-cost", costs };
      return null;
    }
  }

  // 3. Triggered
  if (tokens[0]?.kind === "word" && ["when", "whenever", "at"].includes(tokens[0].value)) {
    const c = session.cursor(tokens);
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
    const costCursor = session.cursor(tokens.slice(0, colonAt));
    const costs = parseCostList(costCursor);
    if (costs && costCursor.done()) {
      const c = session.cursor(tokens.slice(colonAt + 1));
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
    const c = session.cursor(tokens);
    const keywords = parseKeywordLine(c);
    if (keywords) return { kind: "keywords", keywords };
  }

  // 6. Static
  {
    const c = session.cursor(tokens);
    const effect = parseStatic(c);
    if (effect && c.done()) return { kind: "static", abilityWord, effect };
  }

  // 7. Spell text
  {
    const c = session.cursor(tokens);
    const block = parseEffectsBlock(c, options);
    if (block && !block.restriction) return { kind: "spell", abilityWord, effects: block.sentences };
  }

  return null;
}

function containsModal(ability: Ability): boolean {
  return "effects" in ability && ability.effects.some(sentence =>
    sentence.steps.some(step => step.effect === "modal"));
}
registerLineParser((tokens, options) => parseLineTokens(tokens, options));

// ---------------------------------------------------------------------------
// Card-level API
// ---------------------------------------------------------------------------

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
  const session = new LineSession(tokens);
  const ability = parseLineTokens(tokens, optionTokens, session);
  if (ability) {
    if (optionTokens.length && !containsModal(ability))
      return { text, ok: false, error: "modal bullets require a consuming modal header" };
    return { text, ok: true, ability };
  }
  return { text, ok: false, error: session.error() };

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
      const blockText = [line, ...bullets].join("\n");
      try {
        const optionTokens = bullets.map(b => lex(b).slice(1));
        const parsed = parseLineWithDiagnostics(line, optionTokens);
        results.push({ ...parsed, text: blockText });
      } catch (error) {
        if (!(error instanceof LexError)) throw error;
        results.push({ text: blockText, ok: false, error: `modal option: ${error.message} at position ${error.position}` });
      }
      i = j - 1;
      continue;
    }

    results.push(parseLineWithDiagnostics(line, []));
  }

  const name = cardName ?? "";
  const abilities: Ability[] = [];
  for (const result of results) {
    if (!result.ok) return { name, ok: false, lines: results };
    abilities.push(result.ability);
  }
  return { name, ok: true, lines: results, abilities };
}
