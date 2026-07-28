// Segmented context meter (spec §11). Deliberately NOT a single number: the
// point is that "transcript is 40k" and "decision log is 30k" call for
// completely different fixes — compact the chat, versus retune retention N.
// A total alone can't tell those apart, so this reports per segment and says
// which lever to pull.

import type { DatabaseSync } from "node:sqlite";
import { requireDeck } from "../deck/service.ts";
import { assembleContext, type SegmentBehavior } from "./context.ts";
import { countCompacted } from "./chatStore.ts";
import { activeSummary } from "./consolidate.ts";

// No tokenizer dependency: ~4 characters per token is close enough for a
// gauge whose job is "which segment is the problem", and it never disagrees
// with itself between segments. Labelled as an estimate everywhere it shows.
const CHARS_PER_TOKEN = 4;

export function estimateTokens(text: string): number {
  return Math.ceil(text.length / CHARS_PER_TOKEN);
}

export interface MeterSegment {
  key: string;
  label: string;
  chars: number;
  est_tokens: number;
  share: number; // 0–1 of the assembled total
  // What this segment is expected to do over time (spec §10's stability
  // ordering) — this is why a big number here is or isn't a problem.
  // Declared on the segment itself in context.ts.
  behavior: SegmentBehavior;
}

export interface MeterResult {
  revision: number;
  retention_n: number;
  est_tokens: number;
  chars: number;
  segments: MeterSegment[];
  transcript_messages: number;
  compacted_messages: number;
  has_active_summary: boolean;
  // Which lever to pull, if any.
  advice: Array<{ segment: string; severity: "info" | "warn"; message: string }>;
}

// Thresholds are about which fix applies, not about a hard budget.
const TRANSCRIPT_WARN_TOKENS = 25_000;
const LOG_WARN_TOKENS = 12_000;
const DECKLIST_NOTE_TOKENS = 15_000;

export function contextMeter(db: DatabaseSync, deckId: number, retentionN: number): MeterResult {
  const ctx = assembleContext(db, deckId, retentionN);

  const raw = ctx.segments.map((s) => ({
    key: s.key,
    label: s.label,
    chars: s.text.length,
    est_tokens: estimateTokens(s.text),
    behavior: s.behavior,
  }));
  const totalTokens = raw.reduce((n, s) => n + s.est_tokens, 0);
  const segments: MeterSegment[] = raw.map((s) => ({
    ...s,
    share: totalTokens ? s.est_tokens / totalTokens : 0,
  }));

  const { revision } = requireDeck(db, deckId);
  const compacted = countCompacted(db, deckId);
  const summary = activeSummary(db, deckId);

  const by = (key: string) => segments.find((s) => s.key === key)?.est_tokens ?? 0;
  const advice: MeterResult["advice"] = [];
  if (by("transcript") > TRANSCRIPT_WARN_TOKENS)
    advice.push({
      segment: "transcript",
      severity: "warn",
      message: `Transcript is ~${Math.round(by("transcript") / 1000)}k. Run consolidate — this is the segment compaction is for.`,
    });
  if (by("log") > LOG_WARN_TOKENS)
    advice.push({
      segment: "log",
      severity: "warn",
      message: `Decision log is ~${Math.round(by("log") / 1000)}k with retention N=${retentionN}. Compaction will not help this — lower N, or the log has accumulated hard filters and playtest findings that are kept forever by design.`,
    });
  if (by("decklist") > DECKLIST_NOTE_TOKENS)
    advice.push({
      segment: "decklist",
      severity: "info",
      message: `Decklist is ~${Math.round(by("decklist") / 1000)}k. Expected for a full deck with oracle text; it sits above the transcript so it stays in the cached prefix.`,
    });
  if (!advice.length)
    advice.push({
      segment: "total",
      severity: "info",
      message: "Nothing is out of shape. No compaction needed.",
    });

  return {
    revision,
    retention_n: retentionN,
    est_tokens: totalTokens,
    chars: raw.reduce((n, s) => n + s.chars, 0),
    segments,
    transcript_messages: ctx.transcript.length,
    compacted_messages: compacted,
    has_active_summary: !!summary,
    advice,
  };
}
