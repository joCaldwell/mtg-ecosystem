import { createHash } from "node:crypto";
import type { ParseCardResult } from "../ast.ts";
import { lex, LexError, type Token } from "../lexer.ts";
import { normalizeOracleTextTracked } from "../normalize.ts";
import { parseNormalizedOracleText, type ParseContext } from "../parser/index.ts";
import { PrintedCapture } from "./capture.ts";
import { collectFacts, type LexedOccurrence } from "./facts.ts";
import { locateNodes, type CaptureDefects } from "./nodes.ts";
import type { ByteRange, PrintedBlock, PrintedFace, PrintedFact, PrintedLine, PrintedToken } from "./schema.ts";
import { mapToOriginal, Utf8Index, type MappedNormalization } from "./text.ts";

type TextFace = Extract<PrintedFace, { status: "read" }>;
type FaceEvidence = TextFace | Extract<PrintedFace, { status: "unmappable" }>;

function sha256(text: string): string {
  return createHash("sha256").update(text).digest("hex");
}

function tokenShape(token: Token): Omit<PrintedToken, "o" | "n" | "grammarIndex"> {
  if (token.kind === "word") return Object.assign({ t: token.kind }, token.possessiveSuffix ? { possessive: token.possessiveSuffix } : {});
  if (token.kind === "selfref") return Object.assign({ t: token.kind }, token.possessive ? { possessive: true } : {});
  return { t: token.kind };
}

function lineSourceId(index: number): string { return `line:${index}`; }

function lexedLines(trace: MappedNormalization, context: ParseContext) {
  const lines: PrintedLine[] = [];
  const surfaces: LexedOccurrence[][] = [];
  const units: Token[][] = [];
  trace.lines.forEach((text, line) => {
    const sourceId = lineSourceId(line);
    const n = trace.lineRanges[line];
    const bytes = new Utf8Index(text);
    try {
      const tokens = lex(text, sourceId, context.cardNames);
      const ordered = tokens.slice().sort((a, b) => a.provenance!.start - b.provenance!.start);
      const words = ordered.map(token => {
        const { start, end, index } = token.provenance!;
        const normalized: ByteRange = [n[0] + bytes.at(start), n[0] + bytes.at(end)];
        const wire = { ...tokenShape(token), grammarIndex: index, n: normalized, o: mapToOriginal(trace, normalized) } as PrintedToken;
        return { token, wire, selectionIds: new Set<string>() };
      });
      units.push(tokens);
      surfaces.push(words);
      lines.push({ sourceId, n, status: "lexed", tokens: words.map(word => word.wire) });
    } catch (error) {
      if (!(error instanceof LexError)) throw error;
      units.push([]);
      surfaces.push([]);
      lines.push({ sourceId, n, status: "failed", error: error.message, at: n[0] + bytes.at(error.position) });
    }
  });
  return { lines, surfaces, units };
}

/** Same archive-context parser result plus the inert evidence for one face.
 * A4 can use that exact result for the existing binding/resolution passes. */
export function parseOracleTextWithPrintedEvidence(text: string, cardName?: string,
  context: Omit<ParseContext, "includeSourceProvenance"> = {}, sourceFaceIndex = 0):
  { result: ParseCardResult; printed: FaceEvidence; defects: CaptureDefects } {
  const archiveContext: ParseContext = { ...context, includeSourceProvenance: true };
  const trace = normalizeOracleTextTracked(text, cardName, context.cardNames, context.typeLine);
  const defects: CaptureDefects = { nesting: 0, overlap: 0, conflict: 0, unlocatedSelections: 0 };
  if (trace.status === "unmappable") return {
    result: parseNormalizedOracleText(trace.lines, text, cardName, archiveContext),
    printed: { sourceFaceIndex, status: "unmappable", reason: trace.reason },
    defects,
  };

  const removedReminders: TextFace["removedReminders"] = trace.edits.flatMap((edit, editIndex) => edit.edit === "gap"
    ? edit.reminder.map(o => ({ fact: "removed-reminder", editIndex, o })) : []);
  const stationReminders = new Map<number, Extract<PrintedFact, { fact: "reminder-citation" }>>();
  const capture = new PrintedCapture();
  const { lines, surfaces, units } = lexedLines(trace, archiveContext);
  const bind = (tokens: Token[], line: number, offset: number): Token[] => {
    for (const token of tokens) {
      const p = token.provenance!;
      const index = surfaces[line].findIndex(({ token: source }) => p.start + offset === source.provenance!.start &&
        p.end + offset === source.provenance!.end && token.kind === source.kind);
      if (index < 0) continue;
      capture.register(token, { line, index });
      const word = surfaces[line][index];
      if (token.kind === "word" && !token.possessive && (token.value === "target" || token.value === "targets"))
        word.selectionIds.add(`${p.sourceId}:${p.index}`);
    }
    return tokens;
  };
  const readLine: typeof lex = (lineText, sourceId, catalog) => {
    const line = Number(sourceId?.slice("line:".length));
    const full = trace.lines[line];
    if (full === lineText && lines[line].status === "lexed") return bind(units[line], line, 0);
    const tokens = lex(lineText, sourceId, catalog);
    // Station and sticker bodies are explicit suffixes of a physical line.
    // The parser retains its legacy grammar indices local to the suffix.
    if (full?.endsWith(lineText)) return bind(tokens, line, full.length - lineText.length);
    return tokens;
  };
  const result = capture.run(() => parseNormalizedOracleText(trace.lines, text, cardName, archiveContext, readLine, (line, start, end) => {
    const o = new Utf8Index(text).range([start, end]);
    const reminderIndex = removedReminders.findIndex(reminder => reminder.o[0] <= o[0] && o[1] <= reminder.o[1]);
    if (reminderIndex >= 0) stationReminders.set(line, {
      fact: "reminder-citation", role: "station-creature-threshold", reminderIndex, o, field: "/effect/creature/basePT",
    });
  }));

  let first = 0;
  const blocks: PrintedBlock[] = result.lines.map(block => {
    const last = first + (block.text ? block.text.split("\n").length : 0);
    if (trace.lines.slice(first, last).join("\n") !== block.text)
      throw new Error(`Printed block does not partition normalized lines at ${first}`);
    const n: ByteRange = first === last ? [trace.normalizedBytes, trace.normalizedBytes]
      : [trace.lineRanges[first][0], trace.lineRanges[last - 1][1]];
    const blockLines: ByteRange = [first, last];
    const nodes = block.ok ? locateNodes(block.ability, capture, blockLines, defects) : [];
    const facts: PrintedFact[] = collectFacts(surfaces, first, last, nodes, trace.edits, defects);
    const station = stationReminders.get(first);
    if (block.ok && station) facts.push(station);
    const lexFailure = lines.slice(first, last).find(line => line.status === "failed");
    const tokenIndex = !block.ok && /\(token (\d+)\)$/.exec(block.error);
    const failedToken = tokenIndex && last === first + 1 &&
      surfaces[first].find(({ wire }) => wire.grammarIndex === Number(tokenIndex[1]));
    first = last;
    return block.ok ? { status: "read", lines: blockLines, n, facts, nodes: nodes.map(({ node }) => node) }
      : { status: "failed", lines: blockLines, n, facts, error: block.error, at: lexFailure?.at ?? (failedToken ? failedToken.wire.n[0] : n[1]),
        location: lexFailure ? "lex-error" : failedToken ? "token" : "end" };
  });
  if (first !== trace.lines.length) throw new Error("Printed blocks leave normalized lines uncovered");

  return { result, defects, printed: {
    sourceFaceIndex, status: "read",
    original: { bytes: trace.originalBytes, sha256: sha256(text) },
    normalized: { bytes: trace.normalizedBytes, sha256: sha256(trace.lines.join("\n")) },
    edits: trace.edits,
    lines,
    blocks,
    removedReminders,
  } };
}
