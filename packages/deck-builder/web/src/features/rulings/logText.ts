// What a log entry says, in words; shared by the log row and the per-card row.

import type { LogEntry } from "../../api/client.ts";

/**
 * Mirrors the refusals in the server's `undo` (src/deck/membership.ts): a
 * rejection changed nothing, an undo cannot be undone, an entry already undone
 * cannot be again, and an entry that changed no cards has nothing to reverse.
 * Showing the button only when it can work saves a round trip to an error.
 */
export function canUndo(entry: LogEntry): boolean {
  return (
    entry.deck_id != null &&
    entry.kind !== "reject" &&
    entry.kind !== "undo" &&
    entry.undone_by == null &&
    entry.cards.length > 0
  );
}

/** The owner's rulings are written as "owner" (or "owner (from v1)"); anything else is an agent. */
export function isAgentAuthor(author: string): boolean {
  return !/^owner\b/.test(author);
}

/** "Brief edit: thesis" from "Brief edit: thesis accepted — proposed because: …". */
export function documentEditLabel(summary: string): string | null {
  const m = /^(.*?) (?:accepted|rejected) — /.exec(summary);
  return m ? m[1] : null;
}
