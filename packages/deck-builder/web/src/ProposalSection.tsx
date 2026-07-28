import { useState } from "react";
import { api, type DraftItem } from "./api.ts";
import { useDeck } from "./store.tsx";
import { ProposalCard } from "./ProposalCard.tsx";
import { RejectionForm } from "./RejectionForm.tsx";

export function ProposalSection({
  draft,
  setDraft,
}: {
  draft: DraftItem[];
  setDraft: (items: DraftItem[]) => void;
}) {
  const { state, run } = useDeck();
  const { deck, slots, proposals, brief_edits, log, hard_filters, card_notes } = state!;
  const [note, setNote] = useState("");
  /** Id of the brief edit whose rejection form is open. */
  const [rejecting, setRejecting] = useState<number | null>(null);

  function updateDraft(idx: number, patch: Partial<DraftItem>) {
    setDraft(draft.map((d, i) => (i === idx ? { ...d, ...patch } : d)));
  }

  async function submitDraft() {
    await run(() => api.createProposal(deck.id, draft, note));
    setDraft([]);
    setNote("");
  }

  return (
    <>
      {draft.length > 0 && (
        <div className="group draft">
          <h2>Draft proposal</h2>
          {draft.map((d, i) => (
            <div className="card-row" key={`${d.action}-${d.oracle_id}`}>
              <div className="card-main">
                <span className={`chip ${d.action === "add" ? "ok" : "over"}`}>{d.action}</span>
                <span className="name">{d.card_name}</span>
                {d.action === "add" && (
                  <select
                    value={d.slot_id ?? ""}
                    onChange={(e) =>
                      updateDraft(i, { slot_id: e.target.value === "" ? null : Number(e.target.value) })
                    }
                  >
                    <option value="">unslotted</option>
                    {slots.map((s) => (
                      <option key={s.id} value={s.id}>
                        {s.name}
                      </option>
                    ))}
                  </select>
                )}
                <input
                  className="grow"
                  value={d.rationale}
                  onChange={(e) => updateDraft(i, { rationale: e.target.value })}
                  placeholder="Rationale (required)"
                />
                <input
                  value={d.group_id ?? ""}
                  onChange={(e) => updateDraft(i, { group_id: e.target.value || null })}
                  placeholder="group"
                  size={6}
                  title="Items sharing a group id are accepted/rejected as a unit"
                />
                <button
                  className="icon danger"
                  title="Drop from the draft"
                  onClick={() => setDraft(draft.filter((_, j) => j !== i))}
                >
                  ✕
                </button>
              </div>
            </div>
          ))}
          <div className="row gap" style={{ marginTop: 6 }}>
            <input
              className="grow"
              value={note}
              onChange={(e) => setNote(e.target.value)}
              placeholder="Proposal note (optional)"
            />
            <button
              className="primary"
              onClick={submitDraft}
              disabled={draft.some((d) => !d.rationale.trim())}
            >
              Submit proposal
            </button>
          </div>
        </div>
      )}

      {brief_edits.length > 0 && (
        <div className="group">
          <h2>Brief edits awaiting ruling</h2>
          {brief_edits.map((e) => (
            <div className="card-row" key={e.id}>
              <div className="card-main">
                <span className="chip under">{e.kind}</span>
                <span className="muted mono">{e.source}</span>
                <span className="name">
                  {e.kind === "thesis" || e.kind === "constraints"
                    ? (e.payload.content ?? "").slice(0, 80)
                    : e.payload.engine_name}
                </span>
                <span className="spacer" />
                <button
                  className="small"
                  onClick={() => run(() => api.ruleBriefEdit(deck.id, e.id, "accept"))}
                >
                  accept
                </button>
                <button
                  className="small danger"
                  onClick={() => setRejecting(rejecting === e.id ? null : e.id)}
                >
                  reject
                </button>
              </div>
              {rejecting === e.id && (
                <RejectionForm
                  placeholder="Why reject? (required — logged like a rejection)"
                  onConfirm={(type, reason) =>
                    run(() => api.ruleBriefEdit(deck.id, e.id, "reject", type, reason)).then(() =>
                      setRejecting(null),
                    )
                  }
                />
              )}
              {(e.kind === "thesis" || e.kind === "constraints") && (
                <pre className="oracle-text">{e.payload.content}</pre>
              )}
              {e.kind === "engine_set" && (
                <div className="muted rationale">
                  {e.payload.description} — pieces: {(e.payload.pieces ?? []).length}
                </div>
              )}
              <div className="muted rationale">“{e.rationale}”</div>
            </div>
          ))}
        </div>
      )}

      {proposals.length > 0 && (
        <div className="group">
          <h2>Open proposals</h2>
          {proposals.map((p) => (
            <ProposalCard key={p.id} proposal={p} rule={run} />
          ))}
        </div>
      )}

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
    </>
  );
}
