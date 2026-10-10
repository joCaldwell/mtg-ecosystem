// What the owner rules on (spec §8, §6): a proposal's items and a proposed
// edit to a document. Rendered in the deck's proposals panel and in the inbox
// from the same components, so the two surfaces cannot drift.

import { useState } from "react";
import { api, type DocumentEdit, type Envelope, type Outcome, type Proposal, type ProposalItem } from "./api.ts";
import { usePeekProps } from "./CardPeek.tsx";
import { ManaCost } from "./Mana.tsx";
import { CardText, Markdown } from "./Markdown.tsx";
import { ago, diffLines } from "./lib.ts";

/** Runs a ruling and hands back whatever the server returned. */
export type Rule = (fn: () => Promise<Envelope>) => Promise<unknown>;

// A ruling's reason is optional and free text (spec §8.2). Rejecting opens
// this so a reason is one keystroke away, but an empty one is a valid answer.
function ReasonBox({ verb, onConfirm, onCancel }: { verb: string; onConfirm: (reason: string) => void; onCancel: () => void }) {
  const [reason, setReason] = useState("");
  return (
    <form
      className="row gap wrap reject-form"
      onSubmit={(e) => {
        e.preventDefault();
        onConfirm(reason);
      }}
    >
      <input
        className="grow"
        value={reason}
        onChange={(e) => setReason(e.target.value)}
        placeholder="Why? (optional — an agent reads this and learns from it)"
        autoFocus
      />
      <button className="small danger" type="submit">
        {verb}
      </button>
      <button className="small" type="button" onClick={onCancel}>
        cancel
      </button>
    </form>
  );
}

// A group is ruled on as a unit — a swap is one decision, not two — so the
// items are folded into units first and each unit gets a single verdict.
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
    // The unit lands where its first item did, so the proposer's order holds.
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

export function ProposalCard({ proposal, rule, showDeck }: { proposal: Proposal; rule: Rule; showDeck?: boolean }) {
  // Which unit (by first item id), or "all", has its reason box open.
  const [rejecting, setRejecting] = useState<number | "all" | null>(null);
  const peekProps = usePeekProps();
  const deckId = proposal.deck_id;
  const pending = proposal.items.filter((i) => i.status === "pending");

  const ruleUnit = (unit: ProposalItem[], outcome: Outcome, reason = "") =>
    rule(() => api.ruleItems(deckId, [unit[0].id], outcome, reason)).then(() => setRejecting(null));
  const ruleAll = (outcome: Outcome, reason = "") =>
    rule(() => api.ruleProposal(deckId, proposal.id, outcome, reason)).then(() => setRejecting(null));

  function ItemLine({ item }: { item: ProposalItem }) {
    return (
      <>
        <span className={`chip ${item.action === "add" ? "ok" : "over"}`}>{item.action}</span>
        <span className="name peekable" {...peekProps(item.card_name)}>
          {item.card_name}
        </span>
        <ManaCost cost={item.mana_cost} />
        {item.action === "add" && <span className="muted">→ {item.slot_name ?? "unslotted"}</span>}
      </>
    );
  }

  return (
    <div className="proposal">
      <div className="proposal-head">
        <span className="proposal-kind">Deck change</span>
        <span>
          #{proposal.id} · {proposal.author} · {ago(proposal.created_at)}
          {showDeck && (
            <>
              {" · "}
              <a href={`#/deck/${deckId}`}>{proposal.deck_name}</a>
            </>
          )}
        </span>
        {proposal.summary && (
          <>
            {" · "}
            <CardText text={proposal.summary} />
          </>
        )}
        {/* Only when there is more than one decision to make. */}
        {units(pending).length > 1 && (
          <span className="row-actions proposal-all">
            <button className="small" onClick={() => ruleAll("accept")}>
              accept all {pending.length}
            </button>
            <button className="small danger" onClick={() => setRejecting(rejecting === "all" ? null : "all")}>
              reject all
            </button>
          </span>
        )}
      </div>
      {rejecting === "all" && (
        <ReasonBox verb="reject all" onConfirm={(r) => ruleAll("reject", r)} onCancel={() => setRejecting(null)} />
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
              <span className="row-actions">
                {first.status !== "pending" ? (
                  <span className={`chip ${first.status === "accepted" ? "ok" : "over"}`}>{first.status}</span>
                ) : (
                  <>
                    <button className="small" onClick={() => ruleUnit(unit, "accept")}>
                      accept
                    </button>
                    <button
                      className="small danger"
                      onClick={() => setRejecting(rejecting === first.id ? null : first.id)}
                    >
                      reject
                    </button>
                  </>
                )}
              </span>
            </div>

            {grouped ? (
              unit.map((item) => (
                <div className="group-item" key={item.id}>
                  <div className="card-main">
                    <ItemLine item={item} />
                  </div>
                  <div className="muted rationale">
                    <Markdown text={item.reason} />
                  </div>
                </div>
              ))
            ) : (
              <div className="muted rationale">
                <Markdown text={first.reason} />
              </div>
            )}

            {rejecting === first.id && (
              <ReasonBox verb="reject" onConfirm={(r) => ruleUnit(unit, "reject", r)} onCancel={() => setRejecting(null)} />
            )}
          </div>
        );
      })}
    </div>
  );
}

function editTitle(edit: DocumentEdit): string {
  switch (edit.kind) {
    case "profile":
      return "Rewrite your profile";
    case "thesis":
      return "Rewrite the deck thesis";
    case "constraints":
      return "Update the deck constraints";
    case "engine_remove":
      return `Remove engine: ${edit.payload.engine_name}`;
    case "engine_set":
      return `Define engine: ${edit.payload.engine_name}`;
  }
}

export function DocumentEditCard({ edit, rule, showDeck }: { edit: DocumentEdit; rule: Rule; showDeck?: boolean }) {
  const [rejecting, setRejecting] = useState(false);
  const decide = (outcome: Outcome, reason = "") =>
    rule(() => api.ruleDocumentEdit(edit.id, outcome, reason)).then(() => setRejecting(false));
  const isText = edit.kind === "profile" || edit.kind === "thesis" || edit.kind === "constraints";

  return (
    <div className="proposal proposal-brief">
      <div className="proposal-head">
        <span className="proposal-kind">{edit.kind === "profile" ? "Profile edit" : "Brief edit"}</span>
        <span>
          #{edit.id} · {edit.author} · {ago(edit.created_at)}
          {showDeck && edit.deck_id != null && (
            <>
              {" · "}
              <a href={`#/deck/${edit.deck_id}`}>{edit.deck_name}</a>
            </>
          )}
        </span>
      </div>
      <div className="card-row">
        <div className="card-main">
          <span className="name">{editTitle(edit)}</span>
          <span className="spacer" />
          <span className="row-actions">
            <button className="small" onClick={() => decide("accept")}>
              accept
            </button>
            <button className="small danger" onClick={() => setRejecting(!rejecting)}>
              reject
            </button>
          </span>
        </div>
        {rejecting && <ReasonBox verb="reject" onConfirm={(r) => decide("reject", r)} onCancel={() => setRejecting(false)} />}
        {edit.stale && (
          <div className="error-banner">
            This text has changed since the edit was proposed. Accepting replaces the current text with the
            proposal below; the diff is against what the agent saw.
          </div>
        )}
        {isText && (
          <pre className="oracle-text proposal-content diff">
            {/* Against what the agent saw when it proposed, if that was recorded. */}
            {diffLines(edit.payload.base ?? edit.current, edit.payload.content ?? "").map((l, i) => (
              <span key={i} className={`diff-${l.kind}`}>
                {l.kind === "add" ? "+ " : l.kind === "del" ? "− " : "  "}
                {l.text}
              </span>
            ))}
          </pre>
        )}
        {edit.kind === "engine_set" && (
          <div className="muted rationale proposal-content">
            {edit.payload.description && <Markdown text={edit.payload.description} />}
            Pieces: {edit.piece_names.join(", ") || "(none)"}
          </div>
        )}
        <div className="muted rationale proposal-reason">
          Why: <CardText text={edit.reason} />
        </div>
      </div>
    </div>
  );
}
