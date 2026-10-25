import type { ActivityEntry } from "../api/client.ts";
import { parseServerTime } from "./time.ts";

export interface ActivitySession {
  /** `${actor}:${id of the session's first entry}`; stable as newer entries join. */
  key: string;
  actor: string;
  deckNames: string[];
  /** Timestamp of the oldest entry. */
  start: string;
  /** Timestamp of the newest entry. */
  end: string;
  /** Oldest first. */
  entries: ActivityEntry[];
}

/**
 * The feed arrives newest first. Consecutive entries by the same actor less
 * than gapMinutes apart are one session, so a stretch of work reads as one
 * line instead of forty. Sessions are returned newest first.
 */
export function groupSessions(entries: ActivityEntry[], gapMinutes = 30): ActivitySession[] {
  const limit = gapMinutes * 60_000;
  const runs: ActivityEntry[][] = [];
  for (const entry of entries) {
    const run = runs[runs.length - 1];
    const prev = run?.[run.length - 1];
    // `!(gap < limit)` so an unreadable time starts a new session.
    const joins =
      prev && prev.actor === entry.actor &&
      Math.abs(parseServerTime(prev.ts) - parseServerTime(entry.ts)) < limit;
    if (joins) run.push(entry);
    else runs.push([entry]);
  }
  return runs.map((run) => {
    const oldestFirst = [...run].reverse();
    const deckNames: string[] = [];
    for (const e of oldestFirst) if (e.deck_name && !deckNames.includes(e.deck_name)) deckNames.push(e.deck_name);
    return {
      key: `${oldestFirst[0].actor}:${oldestFirst[0].id}`,
      actor: oldestFirst[0].actor_display ?? oldestFirst[0].actor,
      deckNames,
      start: oldestFirst[0].ts,
      end: oldestFirst[oldestFirst.length - 1].ts,
      entries: oldestFirst,
    };
  });
}
