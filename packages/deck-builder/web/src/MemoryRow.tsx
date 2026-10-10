// One memory, as the owner sees it (spec §7.5): what it says, where it came
// from, and the three things the owner can do — edit it, retire it, or
// confirm a hypothesis. Editing one makes it the owner's, after which no
// agent can change it (§7.3).

import { useState } from "react";
import { api, type Memory } from "./api.ts";
import { CardText } from "./Markdown.tsx";
import { ago } from "./lib.ts";

export function MemoryRow({ memory, onChanged, showScope = true }: { memory: Memory; onChanged: () => void; showScope?: boolean }) {
  const [draft, setDraft] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function revise(revision: Parameters<typeof api.reviseMemory>[1]) {
    setError(null);
    try {
      await api.reviseMemory(memory.id, revision);
      setDraft(null);
      onChanged();
    } catch (e: any) {
      setError(e.message);
    }
  }

  return (
    <div className={`memory ${memory.status !== "active" ? "is-retired" : ""}`}>
      <div className="memory-meta">
        {showScope && (
          <span className="chip">{memory.scope === "global" ? "global" : memory.deck_name}</span>
        )}
        <span
          className={`chip ${memory.basis === "inferred" ? "under" : "ok"}`}
          title={memory.basis === "inferred" ? "An agent's conclusion — a hypothesis until you confirm it" : "Something you said"}
        >
          {memory.basis}
        </span>
        {memory.owner_touched && (
          <span className="chip" title="Written or edited by you: agents cannot change it">
            yours
          </span>
        )}
        {memory.status !== "active" && <span className="chip over">{memory.status}</span>}
        <span className="muted">
          #{memory.id} · {memory.author} · {ago(memory.revised_at)}
          {memory.deck_revision != null && ` · at rev ${memory.deck_revision}`}
          {memory.source_log_id != null && ` · from ruling #${memory.source_log_id}`}
          {memory.superseded_by != null && ` · replaced by #${memory.superseded_by}`}
        </span>
      </div>
      {draft == null ? (
        <div className="memory-text">
          <CardText text={memory.text} />
        </div>
      ) : (
        <textarea value={draft} onChange={(e) => setDraft(e.target.value)} rows={3} autoFocus />
      )}
      {error && <div className="error-banner">{error}</div>}
      <div className="row gap memory-actions">
        {draft == null ? (
          <>
            {memory.status === "active" && (
              <button className="small" onClick={() => setDraft(memory.text)}>
                edit
              </button>
            )}
            {memory.status === "active" && memory.basis === "inferred" && (
              <button className="small" onClick={() => revise({ action: "confirm" })} title="This is true: mark it as stated">
                confirm
              </button>
            )}
            {memory.status === "active" ? (
              <button className="small danger" onClick={() => revise({ action: "retire" })} title="Take it out of use. It is kept, not deleted.">
                retire
              </button>
            ) : (
              <button className="small" onClick={() => revise({ action: "restore" })}>
                restore
              </button>
            )}
          </>
        ) : (
          <>
            <button className="small primary" disabled={!draft.trim()} onClick={() => revise({ action: "update", text: draft })}>
              save
            </button>
            <button className="small" onClick={() => setDraft(null)}>
              cancel
            </button>
          </>
        )}
      </div>
    </div>
  );
}

/** Write a memory yourself: it is yours from the start, and stated. */
export function AddMemory({ scope, deckId, onAdded }: { scope: "global" | "deck"; deckId?: number; onAdded: () => void }) {
  const [text, setText] = useState("");
  const [error, setError] = useState<string | null>(null);
  async function submit() {
    setError(null);
    try {
      await api.addMemory({ text, scope, deck_id: deckId ?? null });
      setText("");
      onAdded();
    } catch (e: any) {
      setError(e.message);
    }
  }
  return (
    <div className="stack gap add-memory">
      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        rows={2}
        placeholder={
          scope === "deck"
            ? "Something worth remembering about this deck — how it played, what you decided and why. Write cards as [[Card Name]]."
            : "Something worth remembering about how you build — a preference, a playgroup rule, a standing dislike."
        }
        aria-label="New memory"
      />
      {error && <div className="error-banner">{error}</div>}
      <div className="row gap">
        <span className="spacer" />
        <button className="small primary" disabled={!text.trim()} onClick={submit}>
          Add memory
        </button>
      </div>
    </div>
  );
}
