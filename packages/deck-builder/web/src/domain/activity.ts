import type { ActivityEntry } from "../api/client.ts";
import { dayLabel, parseServerTime } from "./time.ts";

export interface ActivityDay {
  /** The local calendar date ("2026-3-2"), or the raw time when unreadable. */
  key: string;
  /** "Today", "Yesterday" or a short date (`dayLabel`). */
  label: string;
  /** Newest first, as the feed arrives. */
  entries: ActivityEntry[];
}

function dayKey(iso: string): string {
  const t = parseServerTime(iso);
  if (Number.isNaN(t)) return iso;
  const d = new Date(t);
  return `${d.getFullYear()}-${d.getMonth() + 1}-${d.getDate()}`;
}

/**
 * The feed arrives newest first. Consecutive entries on the same local
 * calendar day share one day heading, so every row can be a single line
 * carrying its own time, agent and deck. Days are returned newest first; an
 * unreadable time gets a heading of its own.
 */
export function groupByDay(entries: ActivityEntry[], now: number = Date.now()): ActivityDay[] {
  const days: ActivityDay[] = [];
  for (const entry of entries) {
    const key = dayKey(entry.ts);
    const last = days[days.length - 1];
    if (last && last.key === key && !Number.isNaN(parseServerTime(entry.ts))) last.entries.push(entry);
    else days.push({ key, label: dayLabel(entry.ts, now), entries: [entry] });
  }
  return days;
}
