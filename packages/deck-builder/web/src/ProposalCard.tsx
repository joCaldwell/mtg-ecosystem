// One proposal, rendered wherever it needs to be ruled on: inline above the
// decklist, and inside the agent chat where it was produced. Shared rather
// than duplicated so the two surfaces cannot drift; the rejection questions
// themselves live in RejectionForm.tsx, shared wider still.

import { useState } from "react";
import { api, type Envelope, type Proposal, type ProposalItem } from "./api.ts";
import { useDeck } from "./store.tsx";
import { usePeekProps } from "./CardPeek.tsx";
import { ManaCost } from "./Mana.tsx";
import { RejectionForm } from "./RejectionForm.tsx";

// acceptItem/rejectItem rule on a whole group_id at once — a swap is one
// decision, not two. So the items are folded into units first and each unit
// gets a single verdict; a row per item with its own buttons claimed a choice
// that doesn't exist, and accepting one silently resolved the other.
function units(items: ProposalItem[]): ProposalItem[][] {
  const out: ProposalItem[][] = [];
  const byGroup = new Map<string, ProposalItem[]>();
  for (const item of items) {
    if (!item.group_id) {
      out.push([item]);
      continue;
    }
    const existing = byGroup.get(item.group_id);
    if (existing) existing.push(item);
    // The unit lands where its first item did, so the agent's ordering holds.
    else {
      const unit = [item];
      byGroup.set(item.group_id, unit);
      out.push(unit);
    }
  }
  return out;
}

function unitLabel(unit: ProposalItem[]): string {
  const cuts = unit.filter((i) => i.action === "cut").length;
  const adds = unit.filter((i) => i.action === "add").length;
  if (cuts === 1 && adds === 1) return "swap";
  return `${unit.length} together`;
}

export function ProposalCard({
  proposal,
  rule,
  head = true,
}: {
  proposal: Proposal;
  // Usually the store's run(). The chat wraps it to also refresh the
  // transcript, so a ruling made in one surface shows in both.
  rule: (fn: () => Promise<Envelope>) => Promise<unknown>;
  /** The chat labels the card itself, so it suppresses the "#12 · agent" line. */
  head?: boolean;
}) {
  const { deckId, state } = useDeck();
  const slots = state!.slots;
  const [rejecting, setRejecting] = useState<number | null>(null);
  // The name is the one thing you have to rule on and the one thing the row
  // doesn't explain; the rationale argues for the card without printing it.
  const peekProps = usePeekProps();

  const slotName = (id: number | null) =>
    id == null ? "unslotted" : (slots.find((s) => s.id === id)?.name ?? "?");

  async function confirmReject(itemId: number, type: string, reason: string) {
    await rule(() => api.rejectItem(deckId, itemId, type, reason));
    setRejecting(null);
  }

  // Every item in a unit carries the same status — the server refuses to rule
  // on half a group — so the first speaks for all of them.
  function Verdict({ unit }: { unit: ProposalItem[] }) {
    const [{ id, status }] = unit;
    if (status !== "pending")
      return <span className={`chip ${status === "accepted" ? "ok" : "over"}`}>{status}</span>;
    return (
      <>
        <button className="small" onClick={() => rule(() => api.acceptItem(deckId, id))}>
          accept
        </button>
        <button
          className="small danger"
          onClick={() => setRejecting(rejecting === id ? null : id)}
        >
          reject
        </button>
      </>
    );
  }

  function ItemLine({ item }: { item: ProposalItem }) {
    return (
      <>
        <span className={`chip ${item.action === "add" ? "ok" : "over"}`}>{item.action}</span>
        <span className="name peekable" {...peekProps(item.card_name)}>
          {item.card_name}
        </span>
        <ManaCost cost={item.mana_cost} />
        {item.action === "add" && <span className="muted">→ {slotName(item.slot_id)}</span>}
      </>
    );
  }

  return (
    <div className="proposal">
      {head && (
        <div className="proposal-head">
          #{proposal.id} · {proposal.source}
          {proposal.note && ` · ${proposal.note}`}
        </div>
      )}
      {units(proposal.items).map((unit) => {
        const [first] = unit;
        const grouped = unit.length > 1;
        return (
          <div className={`card-row ${grouped ? "is-group" : ""}`} key={first.id}>
            <div className="card-main">
              {grouped ? (
                <span className="chip" title="Accepted or rejected as one decision">
                  {unitLabel(unit)}
                </span>
              ) : (
                <ItemLine item={first} />
              )}
              <span className="spacer" />
              {/* Kept in one box so the pair wraps together rather than the
                  verdict buttons landing on separate lines. */}
              <span className="row-actions">
                <Verdict unit={unit} />
              </span>
            </div>

            {grouped ? (
              unit.map((item) => (
                <div className="group-item" key={item.id}>
                  <div className="card-main">
                    <ItemLine item={item} />
                  </div>
                  <div className="muted rationale">“{item.rationale}”</div>
                </div>
              ))
            ) : (
              <div className="muted rationale">“{first.rationale}”</div>
            )}

            {rejecting === first.id && (
              <RejectionForm
                placeholder="Why? (required — this is what the agent learns from)"
                onConfirm={(type, reason) => confirmReject(first.id, type, reason)}
              />
            )}
          </div>
        );
      })}
    </div>
  );
}
