import { useState } from "react";
import {
  api,
  type DeckState,
  type DraftItem,
  type ProposalItem,
} from "./api.ts";

const REJECTION_TYPES = [
  { value: "hard_filter", label: "Hard filter — never suggest again" },
  { value: "thesis_change", label: "Thesis change — not what this deck is" },
  { value: "playtest_finding", label: "Playtest finding — tried it, know better" },
  { value: "soft", label: "Soft / not now" },
];

export function ProposalSection({
  state,
  draft,
  setDraft,
  mutate,
}: {
  state: DeckState;
  draft: DraftItem[];
  setDraft: (items: DraftItem[]) => void;
  mutate: (fn: () => Promise<DeckState>) => Promise<void>;
}) {
  const { deck, slots, proposals, log, hard_filters, card_notes } = state;
  const [note, setNote] = useState("");
  const [rejecting, setRejecting] = useState<number | null>(null);
  const [rejectType, setRejectType] = useState("soft");
  const [rejectReason, setRejectReason] = useState("");

  function updateDraft(idx: number, patch: Partial<DraftItem>) {
    setDraft(draft.map((d, i) => (i === idx ? { ...d, ...patch } : d)));
  }

  async function submitDraft() {
    await mutate(() => api.createProposal(deck.id, draft, note));
    setDraft([]);
    setNote("");
  }

  async function confirmReject(item: ProposalItem) {
    await mutate(() => api.rejectItem(deck.id, item.id, rejectType, rejectReason));
    setRejecting(null);
    setRejectReason("");
  }

  const slotName = (id: number | null) =>
    id == null ? "unslotted" : (slots.find((s) => s.id === id)?.name ?? "?");

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
                <button className="small danger" onClick={() => setDraft(draft.filter((_, j) => j !== i))}>
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
            <button onClick={submitDraft} disabled={draft.some((d) => !d.rationale.trim())}>
              Submit proposal
            </button>
          </div>
        </div>
      )}

      {state.brief_edits.length > 0 && (
        <div className="group">
          <h2>Brief edits awaiting ruling</h2>
          {state.brief_edits.map((e) => (
            <div className="card-row" key={e.id}>
              <div className="card-main">
                <span className="chip under">{e.kind}</span>
                <span className="name">
                  {e.kind === "thesis" || e.kind === "constraints"
                    ? (e.payload.content ?? "").slice(0, 80)
                    : e.payload.engine_name}
                </span>
                <span className="spacer" />
                <button
                  className="small"
                  onClick={() => mutate(() => api.ruleBriefEdit(deck.id, e.id, "accept").then((r) => r.state))}
                >
                  accept
                </button>
                <button
                  className="small danger"
                  onClick={() => {
                    const reason = window.prompt("Why reject? (required — logged like a rejection)");
                    if (reason?.trim())
                      mutate(() =>
                        api.ruleBriefEdit(deck.id, e.id, "reject", "soft", reason).then((r) => r.state),
                      );
                  }}
                >
                  reject
                </button>
              </div>
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
            <div key={p.id} className="proposal">
              <div className="muted">
                #{p.id} · {p.source}
                {p.note && ` · ${p.note}`}
              </div>
              {p.items.map((item) => (
                <div className="card-row" key={item.id}>
                  <div className="card-main">
                    <span className={`chip ${item.action === "add" ? "ok" : "over"}`}>
                      {item.action}
                    </span>
                    <span className="name">{item.card_name}</span>
                    <span className="mono cost">{item.mana_cost ?? ""}</span>
                    {item.action === "add" && (
                      <span className="muted">→ {slotName(item.slot_id)}</span>
                    )}
                    {item.group_id && (
                      <span className="chip" title="Atomic group — ruled on as a unit">
                        ⛓ {item.group_id}
                      </span>
                    )}
                    <span className="spacer" />
                    {item.status === "pending" ? (
                      <>
                        <button className="small" onClick={() => mutate(() => api.acceptItem(deck.id, item.id))}>
                          accept
                        </button>
                        <button
                          className="small danger"
                          onClick={() => {
                            setRejecting(rejecting === item.id ? null : item.id);
                            setRejectReason("");
                          }}
                        >
                          reject
                        </button>
                      </>
                    ) : (
                      <span className="muted">{item.status}</span>
                    )}
                  </div>
                  <div className="muted rationale">“{item.rationale}”</div>
                  {rejecting === item.id && (
                    <div className="row gap reject-form">
                      <select value={rejectType} onChange={(e) => setRejectType(e.target.value)}>
                        {REJECTION_TYPES.map((t) => (
                          <option key={t.value} value={t.value}>
                            {t.label}
                          </option>
                        ))}
                      </select>
                      <input
                        className="grow"
                        value={rejectReason}
                        onChange={(e) => setRejectReason(e.target.value)}
                        placeholder="Why? (required — this is what the agent learns from)"
                        autoFocus
                      />
                      <button
                        className="small"
                        disabled={!rejectReason.trim()}
                        onClick={() => confirmReject(item)}
                      >
                        confirm
                      </button>
                    </div>
                  )}
                </div>
              ))}
            </div>
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
              <div className="card-main log-row" key={f.oracle_id}>
                <span className="name">{f.card_name}</span>
                <span className="muted">“{f.reason}”</span>
                <span className="spacer" />
                <button className="small danger" onClick={() => mutate(() => api.removeHardFilter(deck.id, f.oracle_id))}>
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
              <div className="card-main log-row" key={n.id}>
                <span className="name">{n.card_name}</span>
                <span className="muted">“{n.note}”</span>
              </div>
            ))}
          </>
        )}

        <h2>Log</h2>
        {log.map((e) => (
          <div className="card-main log-row" key={e.id}>
            <span className="muted mono">r{e.revision}</span>
            <span className={`chip ${e.kind === "accept" ? "ok" : e.kind === "reject" ? "over" : ""}`}>
              {e.kind}
              {e.action && ` ${e.action}`}
            </span>
            <span className="name">{e.card_name}</span>
            {e.rejection_type && <span className="chip">{e.rejection_type}</span>}
            <span className="muted">
              {e.kind === "reject" ? `“${e.rejection_reason}”` : e.rationale && `“${e.rationale}”`}
            </span>
            {!!e.brief_flag && <span className="chip under" title="Flagged for brief review">brief?</span>}
            <span className="spacer" />
            {e.kind === "accept" && e.undone_by == null && (
              <button className="small" onClick={() => mutate(() => api.undoDecision(deck.id, e.id))}>
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
