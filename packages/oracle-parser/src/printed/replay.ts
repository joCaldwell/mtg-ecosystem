import { canonicalizeNamePunctuation, type CardNameCatalog } from "../card-names.ts";
import { catalogLiteralSpans, normalizeOracleText, shortNameForm } from "../normalize.ts";
import { namesLegendaryCharacter, pronounReplacement, sourcePronounRole } from "../source-pronouns.ts";
import type { Edit } from "./schema.ts";
import { isBalancedParenthetical, replayEdits, Utf8Index, type MappedNormalization } from "./text.ts";

type Gap = Extract<Edit, { edit: "gap" }>;

function gapReplacement(bytes: Buffer, gap: Gap, printed: string): string {
  const [start, end] = gap.o;
  let cursor = start;
  for (const [from, to] of gap.reminder) {
    if (!Number.isInteger(from) || !Number.isInteger(to) || from < cursor || to <= from || to > end ||
        (bytes[from] & 0xc0) === 0x80 || (bytes[to] & 0xc0) === 0x80 ||
        /\S/u.test(bytes.toString("utf8", cursor, from)) ||
        !isBalancedParenthetical(bytes.toString("utf8", from, to))) {
      throw new Error(`Invalid reminder range at original byte ${from}`);
    }
    cursor = to;
  }
  if (/\S/u.test(bytes.toString("utf8", cursor, end))) throw new Error(`Non-whitespace gap at original byte ${cursor}`);
  const replacement = start === 0 || end === bytes.length ? "" : printed.includes("\n") ? "\n" : " ";
  if (printed === replacement) throw new Error(`Unchanged whitespace encoded as gap at original byte ${start}`);
  return replacement;
}

/** Replay the wire edits from their original slices, without using the tracked normalizer. */
export function replayNormalizationEdits(
  text: string, trace: MappedNormalization, cardName?: string, catalog?: CardNameCatalog, typeLine?: string,
): string {
  if (/[~\0]/u.test(text)) throw new Error("Reserved character in original text");
  const original = Buffer.from(text);
  const normalized = Buffer.from(trace.lines.join("\n"));
  const name = cardName ? canonicalizeNamePunctuation(cardName) : "";
  const article = name.startsWith("The ") ? `the ${name.slice(4)}` : undefined;
  const short = name ? shortNameForm(name, typeLine) : undefined;
  const catalogSpans = new Map<number, ReturnType<typeof catalogLiteralSpans>[number] & { byteEnd: number }>();
  const literals = catalog ? catalogLiteralSpans(canonicalizeNamePunctuation(text), catalog) : [];
  if (literals.length) {
    const index = new Utf8Index(text);
    for (const span of literals) catalogSpans.set(index.at(span.start), { ...span, byteEnd: index.at(span.end) });
  }
  const replayed = replayEdits(text, trace, (edit, printed, prefix) => {
    const [start, end] = edit.o;
    const canonical = canonicalizeNamePunctuation(printed);
    const invalid = () => new Error(`Inadmissible ${edit.edit} at original byte ${start}`);
    switch (edit.edit) {
      case "punct":
        if (!/^[’“”−–]$/u.test(printed) || printed.length !== 1) throw invalid();
        return canonical;
      case "gap":
        return gapReplacement(original, edit, printed);
      case "self-name": {
        const before = original.toString("utf8", 0, start), after = original.toString("utf8", end);
        const spelling = edit.form === "full" ? name : edit.form === "article" ? article
          : short?.[1] === edit.form ? short[0] : undefined;
        if (!spelling || canonical !== spelling ||
            /[\p{L}\p{M}\p{N}_]$/u.test(before) || /^[\p{L}\p{M}\p{N}_]/u.test(after) ||
            /\bnamed\s+"?$/i.test(prefix)) throw invalid();
        return "~";
      }
      case "pronoun": {
        const after = normalized.toString("utf8", edit.n[1]).split("\n", 1)[0];
        if (!name || !namesLegendaryCharacter(typeLine) ||
            sourcePronounRole(canonical.toLowerCase(), after) !== edit.role ||
            (edit.as !== "name" && edit.as !== "neuter")) throw invalid();
        return pronounReplacement(canonical, edit.role, edit.as);
      }
      case "literal-name": {
        if (edit.via === "own-name") {
          if (edit.slot !== "named" || !/\bnamed\s+$/i.test(prefix) ||
              !name || (canonical !== name && canonical !== article)) throw invalid();
          return JSON.stringify(canonical);
        }
        const span = catalogSpans.get(start);
        if (edit.via !== "catalog" || !span || span.byteEnd !== end || span.tag.slot !== edit.slot) throw invalid();
        const value = span.quoted ? JSON.parse(canonical) : canonical;
        if (canonicalizeNamePunctuation(span.name) !== canonicalizeNamePunctuation(value)) throw invalid();
        catalogSpans.delete(start);
        return JSON.stringify(span.name);
      }
      default:
        throw invalid();
    }
  });
  // An eligible quoted literal may be byte-identical. Its proof may only be omitted
  // when the entire literal was removed as part of a reminder.
  for (const [start, span] of catalogSpans) {
    if (!trace.edits.some(edit => edit.edit === "gap" &&
      edit.reminder.some(([from, to]) => from <= start && span.byteEnd <= to))) {
      throw new Error(`Missing literal-name edit at original byte ${start}`);
    }
  }
  if (replayed !== normalizeOracleText(text, cardName, catalog, typeLine).join("\n")) {
    throw new Error("Replayed text differs from legacy normalization");
  }
  return replayed;
}
