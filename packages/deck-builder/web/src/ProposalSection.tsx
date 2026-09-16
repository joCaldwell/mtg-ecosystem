import { useState } from "react";
import { api, type DraftItem } from "./api.ts";
import { useDeck } from "./store.tsx";
import { BriefEditCard } from "./BriefEditCard.tsx";
import { ProposalCard } from "./ProposalCard.tsx";

export function ProposalSection({
  draft,
  setDraft,
}: {
  draft: DraftItem[];
  setDraft: (items: DraftItem[]) => void;
}) {
  const { state, run } = useDeck();
  const { deck, slots, proposals, brief_edits } = state!;
  const [note, setNote] = useState("");

  function updateDraft(idx: number, patch: Partial<DraftItem>) {
    setDraft(draft.map((d, i) => (i === idx ? { ...d, ...patch } : d)));
  }

  async function submitDraft() {
    await run(() => api.createProposal(deck.id, draft, note));
    setDraft([]);
    setNote("");
  }

  const empty = draft.length === 0 && brief_edits.length === 0 && proposals.length === 0;

  return (
    <div className="proposals-panel">
      {draft.length > 0 && (
        <div className="proposal proposal-draft">
          <div className="proposal-head">
            <span className="proposal-kind">Draft</span>
            <span>
              {draft.length} card {draft.length === 1 ? "change" : "changes"} · not submitted
            </span>
          </div>
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

      {brief_edits.map((e) => <BriefEditCard key={e.id} edit={e} rule={run} />)}

      {proposals.map((p) => (
        <ProposalCard key={p.id} proposal={p} rule={run} />
      ))}

      {empty && (
        <div className="proposal-empty">
          <h3>No proposals waiting</h3>
          <p>Agent changes, brief updates, and drafts from search will appear here.</p>
        </div>
      )}
    </div>
  );
}
