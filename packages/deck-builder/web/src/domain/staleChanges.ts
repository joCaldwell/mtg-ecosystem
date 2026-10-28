import type { LogEntry } from "../api/client.ts";

const LOG_PAGE = 40; // DeckState.log's limit (src/state.ts)

/**
 * What changed in a deck since a proposal was made, in words ("added A; cut B
 * and C"), read from the deck's log entries after its base revision. Null when
 * the log in hand does not reach back that far, or names no card changes, so
 * the caller says only that the deck changed.
 */
export function changesSince(log: LogEntry[], baseRevision: number | null): string | null {
  if (baseRevision == null) return null;
  const complete = log.length < LOG_PAGE || log.some((e) => e.revision <= baseRevision);
  if (!complete) return null;
  const added = new Set<string>(), cut = new Set<string>(), moved = new Set<string>();
  for (const entry of log) {
    if (entry.revision <= baseRevision || entry.kind === "reject") continue;
    for (const c of entry.cards) (c.action === "add" ? added : c.action === "cut" ? cut : moved).add(c.card_name);
  }
  const list = (names: Set<string>) => {
    const all = [...names];
    return all.length > 3 ? `${all.slice(0, 3).join(", ")} and ${all.length - 3} more` : all.length > 1 ? `${all.slice(0, -1).join(", ")} and ${all.at(-1)}` : all[0];
  };
  const parts = [
    added.size ? `added ${list(added)}` : "",
    cut.size ? `cut ${list(cut)}` : "",
    moved.size ? `moved ${list(moved)} in the command zone` : "",
  ].filter(Boolean);
  return parts.length ? parts.join("; ") : null;
}

/** Plain words for a stale proposal; revision numbers stay in History. */
export function staleNote(changes: string | null): string {
  return changes ? `The deck changed since this was proposed: ${changes}.` : "The deck changed since this was proposed.";
}
