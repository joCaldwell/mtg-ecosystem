import type { LogEntry } from "../api/client.ts";

type CardChange = Pick<LogEntry["cards"][number], "card_name" | "action">;

const LOG_PAGE = 40; // DeckState.log's limit (src/state.ts)

/** Card changes in words ("added A; cut B and C"), or null when there are none. */
export function cardChanges(cards: CardChange[]): string | null {
  const added = new Set<string>(), cut = new Set<string>(), moved = new Set<string>();
  for (const c of cards) (c.action === "add" ? added : c.action === "cut" ? cut : moved).add(c.card_name);
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

/**
 * What changed in a deck since a proposal was made, in words ("added A; cut B
 * and C"), read from the deck's log entries after its base revision. Null when
 * the log in hand does not reach back that far, or names no card changes, so
 * the caller says only that the deck changed.
 */
export function changesSince(log: LogEntry[], baseRevision: number | null): string | null {
  const cards = changedCardsSince(log, baseRevision);
  return cards ? cardChanges(cards) : null;
}

/** The card changes after a base revision, or null when the log in hand
 *  cannot say (no base, or the log does not reach back that far). */
function changedCardsSince(log: LogEntry[], baseRevision: number | null): LogEntry["cards"] | null {
  if (baseRevision == null) return null;
  const complete = log.length < LOG_PAGE || log.some((e) => e.revision <= baseRevision);
  if (!complete) return null;
  return log.filter((e) => e.revision > baseRevision && e.kind !== "reject").flatMap((e) => e.cards);
}

/**
 * Whether the changes since a proposal's base revision touch any card it adds
 * or cuts. Presentation only: the server's stale flag and the acknowledgement
 * it requires are unchanged. When the log cannot say, assume they do.
 */
export function staleTouches(log: LogEntry[], baseRevision: number | null, oracleIds: Iterable<string>): boolean {
  const cards = changedCardsSince(log, baseRevision);
  if (!cards) return true;
  const ids = new Set(oracleIds);
  return cards.some((c) => ids.has(c.oracle_id));
}

/** Plain words for a stale proposal; revision numbers stay in History. */
export function staleNote(changes: string | null): string {
  return changes ? `The deck changed since this was proposed: ${changes}.` : "The deck changed since this was proposed.";
}

/** The toast after accepting: what the acceptance did to the deck, not its revision. */
export function acceptedNote(cards: CardChange[]): string {
  const changes = cardChanges(cards);
  return changes ? `Accepted: ${changes}.` : "Accepted.";
}
