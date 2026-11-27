import type { ByteRange, Edit } from "./schema.ts";

export class Utf8Index {
  readonly byteLength: number;
  private readonly boundaries: Uint32Array;

  constructor(text: string) {
    this.boundaries = new Uint32Array(text.length + 1);
    let bytes = 0;
    for (let unit = 0; unit < text.length;) {
      const scalar = text.codePointAt(unit)!;
      if (scalar >= 0xd800 && scalar <= 0xdfff) throw new RangeError(`Lone surrogate at UTF-16 offset ${unit}`);
      this.boundaries[unit++] = bytes;
      if (scalar > 0xffff) unit++; // A zero entry after offset 0 marks a split surrogate pair.
      bytes += scalar < 0x80 ? 1 : scalar < 0x800 ? 2 : scalar < 0x10000 ? 3 : 4;
      if (bytes > 0xffffffff) throw new RangeError("UTF-8 length exceeds u32");
    }
    this.boundaries[text.length] = this.byteLength = bytes;
  }

  at(unit: number): number {
    if (!Number.isInteger(unit) || unit < 0 || unit >= this.boundaries.length) {
      throw new RangeError(`Invalid UTF-16 offset ${unit}`);
    }
    const byte = this.boundaries[unit];
    if (unit > 0 && byte === 0) throw new RangeError(`Split surrogate pair at UTF-16 offset ${unit}`);
    return byte;
  }

  range([start, end]: [number, number]): ByteRange {
    if (end < start) throw new RangeError("Reversed UTF-16 range");
    return [this.at(start), this.at(end)];
  }
}

export type EditTag = {
  [Kind in Edit["edit"]]: Omit<Extract<Edit, { edit: Kind }>, "o" | "n" | "reminder">
}[Edit["edit"]];

interface Origin {
  start: number;
  end: number;
  tag: EditTag | null;
}

export interface MappedNormalization {
  status: "mapped";
  lines: string[];
  lineRanges: ByteRange[];
  originalBytes: number;
  normalizedBytes: number;
  edits: Edit[];
  /** Indices of literal edits whose enclosing quotes are generated. Local projection metadata. */
  generatedQuotes: number[];
}

export type TrackedNormalization = MappedNormalization | {
  status: "unmappable";
  lines: string[];
  reason: "raw-tilde" | "raw-nul" | "invalid-unicode" | "composed-edit" | "unrestored-marker";
};

export function isBalancedParenthetical(text: string): boolean {
  if (!text.startsWith("(") || !text.endsWith(")")) return false;
  let depth = 0;
  for (let i = 0; i < text.length; i++) {
    if (text[i] === "(") depth++;
    if (text[i] === ")" && --depth === 0) return i === text.length - 1;
  }
  return false;
}

/** Origins are carried in UTF-16 until the final sparse map is emitted. */
export class TrackedText {
  private readonly origins: Origin[];
  private readonly reminders: [number, number][] = [];
  private readonly quoteTags = new Set<EditTag>();
  private readonly markers = new Set<EditTag>();
  private composed = false;

  constructor(original: string) {
    this.original = original;
    this.origins = Array.from({ length: original.length }, (_, start) => ({ start, end: start + 1, tag: null }));
  }

  readonly original: string;

  splice(start: number, end: number, replacement: string, tag: EditTag, mode?: "reminder" | "protect" | "restore" | "quote"): void {
    if (start >= end || !replacement || end > this.origins.length) {
      this.composed = true;
      return;
    }
    if (this.composed) return;
    const first = this.origins[start], last = this.origins[end - 1];
    if (mode === "reminder") {
      if (first.tag || last.tag || !isBalancedParenthetical(this.original.slice(first.start, last.end))) {
        this.composed = true;
      }
      this.reminders.push([first.start, last.end]);
    }
    for (let i = start; i < end; i++) {
      const previous = this.origins[i].tag;
      if (mode === "reminder") {
        if (previous) this.markers.delete(previous);
      } else if (mode === "restore" || mode === "quote") {
        if (previous !== tag) this.composed = true;
      } else if (previous && previous.edit !== "punct" && !(tag.edit === "gap" && previous.edit === "gap")) {
        this.composed = true;
      }
    }
    if (mode === "protect") this.markers.add(tag);
    if (mode === "restore" || mode === "quote") this.markers.delete(tag);
    if (mode === "quote") this.quoteTags.add(tag);
    const origin = { start: first.start, end: last.end, tag };
    // Avoid argument-count limits for long inputs.
    if (replacement.length > 1024) {
      const tail = this.origins.splice(start);
      for (let i = 0; i < replacement.length; i++) this.origins.push(origin);
      for (let i = end - start; i < tail.length; i++) this.origins.push(tail[i]);
    } else {
      this.origins.splice(start, end - start, ...Array<Origin>(replacement.length).fill(origin));
    }
  }

  invalidate(): void {
    this.composed = true;
  }

  finish(lines: string[], starts: number[]): TrackedNormalization {
    if (this.composed) return { status: "unmappable", lines, reason: "composed-edit" };
    if (this.markers.size) return { status: "unmappable", lines, reason: "unrestored-marker" };
    const normalized = lines.join("\n");
    let oIndex: Utf8Index, nIndex: Utf8Index;
    try {
      oIndex = new Utf8Index(this.original);
      nIndex = new Utf8Index(normalized);
    } catch (error) {
      if (!(error instanceof RangeError)) throw error;
      return { status: "unmappable", lines, reason: "invalid-unicode" };
    }

    const edits: Edit[] = [];
    const lineRanges: ByteRange[] = [];
    const generatedQuotes: number[] = [];
    const reminders = this.reminders.sort((a, b) => a[0] - b[0]);
    let reminderIndex = 0;
    let o = 0, n = 0;
    const emitGap = (end: number, replacement: string) => {
      if (end < o || end === o && replacement) { this.composed = true; return; }
      if (this.original.slice(o, end) !== replacement) {
        const reminder: ByteRange[] = [];
        while (reminders[reminderIndex]?.[1] <= end) {
          const range = reminders[reminderIndex++];
          if (range[0] < o) { this.composed = true; return; }
          reminder.push(oIndex.range(range));
        }
        edits.push({ edit: "gap", reminder, o: oIndex.range([o, end]), n: nIndex.range([n, n + replacement.length]) });
      }
      o = end;
      n += replacement.length;
    };
    try {
      lines.forEach((line, lineNumber) => {
        let unit = starts[lineNumber];
        const end = unit + line.length;
        emitGap(this.origins[unit].start, lineNumber === 0 ? "" : "\n");
        const lineStart = n;
        while (unit < end) {
          const origin = this.origins[unit];
          if (origin.start !== o) { this.composed = true; break; }
          if (!origin.tag) {
            // Keep adjacent copies as one run, including both halves of a surrogate pair.
            do { o = this.origins[unit++].end; n++; } while (
              unit < end && !this.origins[unit].tag && this.origins[unit].start === o
            );
            continue;
          }
          let next = unit + 1;
          while (next < end && this.origins[next] === origin) next++;
          if (origin.tag.edit === "gap") {
            emitGap(origin.end, line.slice(unit - starts[lineNumber], next - starts[lineNumber]));
          } else {
            const oRange = oIndex.range([o, origin.end]), nRange = nIndex.range([n, n + next - unit]);
            if (this.quoteTags.has(origin.tag)) generatedQuotes.push(edits.length);
            edits.push({ ...origin.tag, o: oRange, n: nRange });
            o = origin.end;
            n += next - unit;
          }
          unit = next;
        }
        if (!this.composed) lineRanges.push(nIndex.range([lineStart, n]));
      });
      if (!this.composed) emitGap(this.original.length, "");
    } catch (error) {
      if (!(error instanceof RangeError)) throw error;
      return { status: "unmappable", lines, reason: "composed-edit" };
    }
    if (this.composed || reminderIndex !== reminders.length || n !== normalized.length) {
      return { status: "unmappable", lines, reason: "composed-edit" };
    }
    return { status: "mapped", lines, lineRanges, originalBytes: oIndex.byteLength, normalizedBytes: nIndex.byteLength, edits, generatedQuotes };
  }
}

function byteBoundary(bytes: Uint8Array, at: number): boolean {
  return Number.isInteger(at) && at >= 0 && at <= bytes.length && (at === bytes.length || (bytes[at] & 0xc0) !== 0x80);
}

export interface ProjectedRange {
  original: ByteRange[];
  anchors: number[];
}

function project(trace: MappedNormalization, [start, end]: ByteRange, projectQuotes: boolean): ProjectedRange {
  const normalized = Buffer.from(trace.lines.join("\n"));
  if (end < start || !byteBoundary(normalized, start) || !byteBoundary(normalized, end)) {
    throw new RangeError("Invalid normalized UTF-8 range");
  }
  const original: ByteRange[] = [];
  const anchors: number[] = [];
  let o = 0, n = 0;
  const anchor = (offset: number) => { if (anchors.at(-1) !== offset) anchors.push(offset); };
  const append = (from: number, to: number) => {
    const last = original.at(-1);
    if (last?.[1] === from) last[1] = to;
    else original.push([from, to]);
  };
  const copyThrough = (limit: number) => {
    const from = Math.max(start, n), to = Math.min(end, limit);
    if (from < to) append(o + from - n, o + to - n);
    if (start === end && n <= start && start <= limit) anchor(o + start - n);
  };
  trace.edits.forEach((edit, index) => {
    const [from, to] = edit.n;
    copyThrough(from);
    if (start === end && from < start && start < to) append(...edit.o);
    if (from < end && start < to && from < to && start < end) {
      if (projectQuotes && trace.generatedQuotes.includes(index)) {
        if (start <= from) anchor(edit.o[0]);
        if (start < to - 1 && from + 1 < end) append(...edit.o);
        if (end >= to) anchor(edit.o[1]);
      } else append(...edit.o);
    }
    [o, n] = [edit.o[1], to];
  });
  copyThrough(trace.normalizedBytes);
  return { original, anchors };
}

export function projectNormalizedRange(trace: MappedNormalization, range: ByteRange): ProjectedRange {
  return project(trace, range, true);
}

/** A nonempty selection inside a rewrite claims the entire original spelling. */
export function mapToOriginal(trace: MappedNormalization, range: ByteRange): ByteRange {
  const { original, anchors } = project(trace, range, false);
  return original.length ? [original[0][0], original[original.length - 1][1]] : [anchors[0], anchors[anchors.length - 1]];
}

/** Check the axes and copied runs independently of the trace builder, then replay each typed edit. */
export function replayEdits(
  text: string,
  trace: MappedNormalization,
  replacementFor: (edit: Edit, printed: string, normalizedPrefix: string) => string,
): string {
  const normalized = trace.lines.join("\n");
  const originalBytes = new Utf8Index(text).byteLength;
  const normalizedIndex = new Utf8Index(normalized);
  if (originalBytes !== trace.originalBytes || normalizedIndex.byteLength !== trace.normalizedBytes) {
    throw new Error("Normalization byte lengths disagree");
  }
  let unit = 0;
  if (trace.lineRanges.length !== trace.lines.length || trace.lines.some((line, i) => {
    const [start, end] = normalizedIndex.range([unit, unit + line.length]);
    unit += line.length + 1;
    return !line || line.includes("\n") || trace.lineRanges[i][0] !== start || trace.lineRanges[i][1] !== end;
  })) throw new Error("Invalid normalized physical line ranges");

  const original = Buffer.from(text), expected = Buffer.from(normalized);
  let o = 0, n = 0, replayed = "";
  const copy = (oEnd: number, nEnd: number) => {
    const bytes = original.subarray(o, oEnd);
    if (!bytes.equals(expected.subarray(n, nEnd))) throw new Error("Unchanged runs differ");
    replayed += bytes.toString();
  };
  for (const edit of trace.edits) {
    const [oStart, oEnd] = edit.o, [nStart, nEnd] = edit.n;
    if (oStart < o || oEnd <= oStart || nStart < n || nEnd < nStart ||
        !byteBoundary(original, oStart) || !byteBoundary(original, oEnd) ||
        !byteBoundary(expected, nStart) || !byteBoundary(expected, nEnd)) {
      throw new Error("Invalid edit ranges or nonmonotone edit map");
    }
    copy(oStart, nStart);
    const replacement = replacementFor(edit, original.toString("utf8", oStart, oEnd), replayed);
    if (!Buffer.from(replacement).equals(expected.subarray(nStart, nEnd))) {
      throw new Error(`Invalid ${edit.edit} replacement at original byte ${oStart}`);
    }
    replayed += replacement;
    [o, n] = [oEnd, nEnd];
  }
  copy(original.length, expected.length);
  return replayed;
}
