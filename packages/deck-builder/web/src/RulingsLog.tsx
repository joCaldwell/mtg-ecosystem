import { useState } from "react";
import { api, type LogEntry } from "./api.ts";
import { useDeck } from "./store.tsx";
import { ago } from "./lib.ts";

const VERB: Record<LogEntry["kind"], string> = {
  accept: "accepted",
  reject: "rejected",
  edit: "edit",
  import: "import",
  undo: "undo",
};

/** The deck's rulings log (spec §8.2): the version history, with undo. */
export function RulingsLog() {
  const { state, run } = useDeck();
  const { deck, log } = state!;
  const [skipped, setSkipped] = useState<string[]>([]);

  async function undo(entry: LogEntry) {
    const result = await run(() => api.undo(deck.id, entry.id));
    setSkipped(result?.skipped ?? []);
  }

  return (
    <details className="group">
      <summary>Rulings and changes ({log.length})</summary>
      {skipped.length > 0 && (
        <div className="error-banner">
          Undone, except what the deck had already moved past: {skipped.join("; ")}.
        </div>
      )}
      {log.map((e) => (
        <div className="log-row" key={e.id}>
          <span className="muted mono">r{e.revision}</span>
          <span className={`chip ${e.kind === "accept" ? "ok" : e.kind === "reject" ? "over" : ""}`}>
            {VERB[e.kind]}
          </span>
          <span className="name">{e.summary}</span>
          <span className="reason">
            {e.ruling_reason && `“${e.ruling_reason}” · `}
            {e.author} · {ago(e.ts)}
            {e.undone_by != null && " · undone"}
            {e.reviewed_at == null && (
              <span className="chip under" title="No agent has read this yet">
                unread by agents
              </span>
            )}
          </span>
          {e.kind !== "reject" && e.kind !== "undo" && e.undone_by == null && e.cards.length > 0 && (
            <button className="small" onClick={() => undo(e)}>
              undo
            </button>
          )}
        </div>
      ))}
      {!log.length && <div className="muted">Nothing yet.</div>}
    </details>
  );
}
