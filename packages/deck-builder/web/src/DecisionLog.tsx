import { api } from "./api.ts";
import { useDeck } from "./store.tsx";

export function DecisionLog() {
  const { state, run } = useDeck();
  const { deck, log, hard_filters, card_notes } = state!;
  return (
  <details className="group">
    <summary>
      Decision log ({log.length}){hard_filters.length > 0 && ` · ${hard_filters.length} hard filter(s)`}
      {card_notes.length > 0 && ` · ${card_notes.length} playtest note(s)`}
    </summary>

    {hard_filters.length > 0 && (
      <>
        <h2>Hard filters</h2>
        {hard_filters.map((f) => (
          <div className="log-row" key={f.oracle_id}>
            <span className="name">{f.card_name}</span>
            <span className="reason">“{f.reason}”</span>
            <button className="small danger" onClick={() => run(() => api.removeHardFilter(deck.id, f.oracle_id))}>
              remove
            </button>
          </div>
        ))}
      </>
    )}

    {card_notes.length > 0 && (
      <>
        <h2>Playtest findings</h2>
        {card_notes.map((n) => (
          <div className="log-row" key={n.id}>
            <span className="name">{n.card_name}</span>
            <span className="reason">“{n.note}”</span>
          </div>
        ))}
      </>
    )}

    <h2>Log</h2>
    {log.map((e) => (
      <div className="log-row" key={e.id}>
        <span className="muted mono">r{e.revision}</span>
        <span className={`chip ${e.kind === "accept" ? "ok" : e.kind === "reject" ? "over" : ""}`}>
          {e.kind}
          {e.action && ` ${e.action}`}
        </span>
        <span className="name">{e.card_name}</span>
        {e.rejection_type && <span className="chip">{e.rejection_type}</span>}
        <span className="reason">
          {e.kind === "reject" ? `“${e.rejection_reason}”` : e.rationale && `“${e.rationale}”`}
        </span>
        {!!e.brief_flag && <span className="chip under" title="Flagged for brief review">brief?</span>}
        {e.kind === "accept" && e.undone_by == null && (
          <button className="small" onClick={() => run(() => api.undoDecision(deck.id, e.id))}>
            undo
          </button>
        )}
      </div>
    ))}
    {!log.length && <div className="muted">No decisions yet.</div>}
  </details>
  );
}
