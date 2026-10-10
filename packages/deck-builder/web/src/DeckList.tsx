import { useEffect, useState, type FormEvent } from "react";
import { api, type DeckSummary } from "./api.ts";
import { ConfirmDialog } from "./ConfirmDialog.tsx";
import { useLive } from "./live.tsx";

export function DeckList() {
  const [decks, setDecks] = useState<DeckSummary[]>([]);
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<DeckSummary | null>(null);

  const live = useLive();
  const reload = () => api.listDecks().then(setDecks).catch((e) => setError(e.message));
  useEffect(() => {
    reload();
  }, [live]);

  async function create(e: FormEvent) {
    e.preventDefault();
    setError(null);
    try {
      const { state } = await api.createDeck(name);
      window.location.hash = `#/deck/${state.deck.id}`;
    } catch (e: any) {
      setError(e.message);
    }
  }

  async function remove(deck: DeckSummary) {
    setDeleteTarget(null);
    setError(null);
    try {
      await api.deleteDeck(deck.id);
      reload();
    } catch (e: any) {
      setError(e.message);
    }
  }

  return (
    <div className="deck-list-page">
      <header className="deck-list-heading">
        <h1>Your decks</h1>
        <span className="muted">Commander workspace</span>
      </header>
      {error && <div className="error-banner">{error}</div>}
      <form onSubmit={create} className="row gap deck-create">
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="New deck name"
          aria-label="New deck name"
        />
        <button className="primary" type="submit" disabled={!name.trim()}>
          Create
        </button>
      </form>
      <table className="deck-table">
        <thead>
          <tr>
            <th>Name</th>
            <th>Commander</th>
            <th>Identity</th>
            <th>Cards</th>
            <th>Status</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {decks.map((d) => {
            const waiting = d.pending_items + d.pending_document_edits;
            return (
              <tr key={d.id}>
                <td>
                  <a href={`#/deck/${d.id}`}>{d.name}</a>
                  {d.parent_name && <span className="muted"> · fork of {d.parent_name}</span>}
                  {waiting > 0 && (
                    <span className="tool-count" title="Awaiting your ruling">
                      {waiting}
                    </span>
                  )}
                </td>
                <td className="muted">{d.commanders.join(" + ") || "—"}</td>
                <td className="mono">{d.color_identity || "—"}</td>
                <td>{d.card_count}/100</td>
                <td>
                  <span
                    className={`chip ${d.status === "draft" ? "under" : "ok"}`}
                    title={
                      d.status === "draft"
                        ? "Draft: an agent may edit it directly"
                        : "Active: an agent's changes come to you as proposals"
                    }
                  >
                    {d.status}
                  </span>
                </td>
                <td>
                  <button className="danger small" onClick={() => setDeleteTarget(d)} aria-haspopup="dialog">
                    delete
                  </button>
                </td>
              </tr>
            );
          })}
          {!decks.length && (
            <tr>
              <td colSpan={6} className="muted">
                No decks yet — create one above, or ask an agent to build one.
              </td>
            </tr>
          )}
        </tbody>
      </table>
      {deleteTarget && (
        <ConfirmDialog
          title="Delete deck"
          confirmLabel="Delete deck"
          destructive
          onClose={() => setDeleteTarget(null)}
          onConfirm={() => void remove(deleteTarget)}
        >
          <p>
            Delete <strong>{deleteTarget.name}</strong>, its history and its memories?
          </p>
          <p className="muted">This cannot be undone.</p>
        </ConfirmDialog>
      )}
    </div>
  );
}
