import { useEffect, useState, type FormEvent } from "react";
import { api, type DeckSummary } from "./api.ts";

export function DeckList() {
  const [decks, setDecks] = useState<DeckSummary[]>([]);
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);

  const reload = () => api.listDecks().then(setDecks).catch((e) => setError(e.message));
  useEffect(() => {
    reload();
  }, []);

  async function create(e: FormEvent) {
    e.preventDefault();
    setError(null);
    try {
      const state = await api.createDeck(name);
      window.location.hash = `#/deck/${state.deck.id}`;
    } catch (e: any) {
      setError(e.message);
    }
  }

  async function remove(deck: DeckSummary) {
    if (!window.confirm(`Delete deck '${deck.name}'? This cannot be undone.`)) return;
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
      <h1>Decks</h1>
      {error && <div className="error-banner">{error}</div>}
      <form onSubmit={create} className="row gap">
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="New deck name"
        />
        <button type="submit" disabled={!name.trim()}>
          Create
        </button>
      </form>
      <table className="deck-table">
        <thead>
          <tr>
            <th>Name</th>
            <th>Identity</th>
            <th>Cards</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {decks.map((d) => (
            <tr key={d.id}>
              <td>
                <a href={`#/deck/${d.id}`}>{d.name}</a>
              </td>
              <td className="mono">{d.color_identity || "—"}</td>
              <td>{d.card_count}/100</td>
              <td>
                <button className="danger small" onClick={() => remove(d)}>
                  delete
                </button>
              </td>
            </tr>
          ))}
          {!decks.length && (
            <tr>
              <td colSpan={4} className="muted">
                No decks yet — create one above.
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
