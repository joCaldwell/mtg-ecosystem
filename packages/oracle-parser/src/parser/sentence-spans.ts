// Which printed tokens produced which top-level AST sentence (or which field
// of an earlier sentence a folded clause completed). The sentence loop calls
// `step` before each clause and `finish` once it accepts its sentences; only
// tokens with source provenance are recorded, so ordinary parsing pays nothing.
// The archive's span certificate is built from these records (semantic/spans.ts).
import type { Token } from "../lexer.ts";
import type { Sentence } from "../ast.ts";
import type { Cursor } from "./cursor.ts";

/** One loop step: the provenance indices it consumed (all from one source
 * line) and the effects-relative pointers it produced (`/2` for a new
 * sentence, `/0/steps/1/regenerationForbidden` for a folded rider). */
export interface SentenceSpan { sourceId: string; tokens: number[]; pointers: string[] }

const recorded = new WeakMap<readonly Sentence[], SentenceSpan[]>();

/** The spans recorded for this exact sentences array, if it came straight
 * from the sentence loop with provenance. */
export function sentenceSpans(effects: readonly Sentence[]): readonly SentenceSpan[] | undefined {
  return recorded.get(effects);
}

/** The shallowest pointer under which `before` and `after` differ, or null
 * when they are equal. A single added key is its own pointer. */
export function changed(before: unknown, after: unknown, path = ""): string | null {
  if (JSON.stringify(before) === JSON.stringify(after)) return null;
  if (Array.isArray(before) && Array.isArray(after) && before.length === after.length) {
    const diffs = before.flatMap((item, index) => JSON.stringify(item) === JSON.stringify(after[index]) ? [] : [index]);
    return diffs.length === 1 ? changed(before[diffs[0]], after[diffs[0]], `${path}/${diffs[0]}`) : path;
  }
  if (before && after && typeof before === "object" && typeof after === "object" && !Array.isArray(before) && !Array.isArray(after)) {
    const a = before as Record<string, unknown>, b = after as Record<string, unknown>;
    const keys = [...new Set([...Object.keys(a), ...Object.keys(b)])];
    const diffs = keys.filter(key => JSON.stringify(a[key]) !== JSON.stringify(b[key]));
    if (diffs.length !== 1) return path;
    const key = diffs[0].replace(/~/g, "~0").replace(/\//g, "~1");
    return diffs[0] in a && diffs[0] in b ? changed(a[diffs[0]], b[diffs[0]], `${path}/${key}`) : `${path}/${key}`;
  }
  return path;
}

export class SentenceRecorder {
  private readonly spans: SentenceSpan[] = [];
  private from: number;
  private before: string | null = null;
  private count: number;
  private readonly c: Cursor;
  private readonly sentences: Sentence[];
  private readonly modalOptions: Token[][];

  constructor(c: Cursor, sentences: Sentence[], modalOptions: Token[][]) {
    this.c = c;
    this.sentences = sentences;
    this.modalOptions = modalOptions;
    this.from = c.pos;
    this.count = sentences.length;
    this.snapshot();
  }

  private enabled(): boolean {
    return this.c.tokens[this.from]?.provenance !== undefined;
  }

  private snapshot(): void {
    this.from = this.c.pos;
    this.count = this.sentences.length;
    this.before = this.enabled() ? JSON.stringify(this.sentences) : null;
  }

  /** Close the previous loop step (if it consumed tokens) and open the next. */
  step(): void {
    if (this.c.pos > this.from && this.before !== null) {
      const tokens = this.c.tokens.slice(this.from, this.c.pos);
      const sourceId = tokens[0].provenance?.sourceId;
      const pointers = this.sentences.length > this.count
        ? this.sentences.slice(this.count).map((_, index) => `/${this.count + index}`)
        : [changed(JSON.parse(this.before), this.sentences)].filter((p): p is string => p !== null);
      if (this.c.printed && this.sentences.length === this.count + 1) {
        const sentence = this.c.mark(this.sentences[this.count], this.from);
        if (this.modalOptions.length && sentence.steps.some(step => step.effect === "modal"))
          this.c.foldTokens(sentence, this.modalOptions.flat());
      } else if (this.c.printed && this.sentences.length === this.count && pointers.length === 1) {
        const [, index] = pointers[0].split("/");
        const sentence = this.sentences[Number(index)];
        if (sentence) this.c.foldTokens(sentence, tokens);
      }
      if (sourceId !== undefined && tokens.every(token => token.provenance?.sourceId === sourceId))
        this.spans.push({ sourceId, tokens: tokens.map(token => token.provenance!.index), pointers });
      else this.spans.push({ sourceId: "", tokens: [], pointers: [] });
    }
    this.snapshot();
  }

  /** Register the spans for the accepted sentences, built step by step. */
  finish(result: Sentence[]): Sentence[] {
    this.step();
    if (this.spans.length && this.spans.every(span => span.sourceId)) recorded.set(result, this.spans);
    return result;
  }
}

/** A result the loop did not build step by step (or could not attribute) is
 * one span for everything consumed, naming every sentence it produced. */
export function recordWhole(c: Cursor, start: number, result: Sentence[]): void {
  if (recorded.has(result)) return;
  if (result.length === 1) c.mark(result[0], start);
  const tokens = c.tokens.slice(start, c.pos);
  const sourceId = tokens[0]?.provenance?.sourceId;
  if (sourceId === undefined || tokens.some(token => token.provenance?.sourceId !== sourceId)) return;
  recorded.set(result, [{ sourceId, tokens: tokens.map(token => token.provenance!.index), pointers: result.map((_, i) => `/${i}`) }]);
}

/** A deep copy of an ability (characteristic finalization, a merged
 * paragraph) keeps the spans of the sentences it copied. */
export function carrySentenceSpans<T>(from: T, to: T): T {
  const effects = (value: unknown) => value && typeof value === "object" && "effects" in value && Array.isArray(value.effects) ? value.effects as Sentence[] : undefined;
  const a = effects(from), b = effects(to);
  const spans = a && recorded.get(a);
  if (b && spans && !recorded.has(b) && b.length === a.length) recorded.set(b, spans);
  return to;
}
