// One agent-authored brief edit, rendered wherever the owner can rule on it:
// the proposals panel and the chat turn that produced it. Keeping the ruling
// controls shared means both surfaces show the same live status and ask the
// same typed rejection question.

import { useState } from "react";
import { api, type BriefEdit, type Envelope } from "./api.ts";
import { useDeck } from "./store.tsx";
import { RejectionForm } from "./RejectionForm.tsx";

function briefEditTitle(edit: BriefEdit): string {
  if (edit.kind === "thesis") return "Rewrite the deck thesis";
  if (edit.kind === "constraints") return "Update deck constraints";
  if (edit.kind === "engine_remove") return `Remove engine: ${edit.payload.engine_name}`;
  return `Update engine: ${edit.payload.engine_name}`;
}

export function BriefEditCard({
  edit,
  rule,
  head = true,
}: {
  edit: BriefEdit;
  rule: (fn: () => Promise<Envelope>) => Promise<unknown>;
  /** The chat supplies its own compact heading. */
  head?: boolean;
}) {
  const { deckId } = useDeck();
  const [rejecting, setRejecting] = useState(false);

  async function confirmReject(type: string, reason: string) {
    await rule(() => api.ruleBriefEdit(deckId, edit.id, "reject", type, reason));
    setRejecting(false);
  }

  function Verdict() {
    if (edit.status !== "pending") {
      return (
        <span className={`chip ${edit.status === "accepted" ? "ok" : "over"}`}>
          {edit.status}
        </span>
      );
    }
    return (
      <>
        <button className="small" onClick={() => rule(() => api.ruleBriefEdit(deckId, edit.id, "accept"))}>
          accept
        </button>
        <button className="small danger" onClick={() => setRejecting(!rejecting)}>
          reject
        </button>
      </>
    );
  }

  return (
    <div className="proposal proposal-brief">
      {head && (
        <div className="proposal-head">
          <span className="proposal-kind">Brief update</span>
          <span>{edit.kind} · {edit.source}</span>
        </div>
      )}
      <div className="card-row">
        <div className="card-main">
          <span className="name">{briefEditTitle(edit)}</span>
          <span className="spacer" />
          <span className="row-actions">
            <Verdict />
          </span>
        </div>
        {rejecting && edit.status === "pending" && (
          <RejectionForm
            placeholder="Why reject? (required — logged like a rejection)"
            onConfirm={confirmReject}
          />
        )}
        {(edit.kind === "thesis" || edit.kind === "constraints") && (
          <pre className="oracle-text proposal-content">{edit.payload.content}</pre>
        )}
        {edit.kind === "engine_set" && (
          <div className="muted rationale proposal-content">
            {edit.payload.description} — pieces: {(edit.payload.pieces ?? []).length}
          </div>
        )}
        <div className="muted rationale proposal-reason">“{edit.rationale}”</div>
      </div>
    </div>
  );
}
