import { useState, type FormEvent } from "react";
import { api, type CardData, type DeckState, type DraftItem, type Slot } from "./api.ts";

export function SearchPanel({
  deckId,
  slots,
  hasCommander,
  mutate,
  draftAdd,
}: {
  deckId: number;
  slots: Slot[];
  hasCommander: boolean;
  mutate: (fn: () => Promise<DeckState>) => Promise<void>;
  draftAdd?: (item: DraftItem) => void;
}) {
  const [query, setQuery] = useState("");
  const [filterIdentity, setFilterIdentity] = useState(true);
  const [results, setResults] = useState<CardData[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [targetSlot, setTargetSlot] = useState<string>("");
  const [expanded, setExpanded] = useState<string | null>(null);

  async function run(e: FormEvent) {
    e.preventDefault();
    setError(null);
    try {
      setResults(await api.search(query, deckId, filterIdentity));
    } catch (e: any) {
      setError(e.message);
      setResults(null);
    }
  }

  function add(card: CardData, role?: string) {
    mutate(() =>
      api.addCard(deckId, card.oracle_id, targetSlot === "" ? null : Number(targetSlot), role),
    );
  }

  return (
    <div>
      <h2>Search</h2>
      <form onSubmit={run} className="stack gap">
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder='t:creature o:"draw a card" mv<=3'
        />
        <div className="row gap wrap">
          <label title="Exclude cards outside the deck's color identity">
            <input
              type="checkbox"
              checked={filterIdentity}
              onChange={(e) => setFilterIdentity(e.target.checked)}
            />
            deck colors only
          </label>
          <select value={targetSlot} onChange={(e) => setTargetSlot(e.target.value)} title="Add into slot">
            <option value="">→ unslotted</option>
            {slots.map((s) => (
              <option key={s.id} value={s.id}>
                → {s.name}
              </option>
            ))}
          </select>
          <button type="submit" disabled={!query.trim()}>
            Search
          </button>
        </div>
      </form>
      {error && <div className="error-banner">{error}</div>}
      {results && (
        <div className="results">
          <div className="muted">{results.length} result(s)</div>
          {results.map((c) => (
            <div className="result" key={c.oracle_id}>
              <div className="card-main">
                <button
                  className="link name"
                  onClick={() => setExpanded(expanded === c.oracle_id ? null : c.oracle_id)}
                >
                  {c.name}
                </button>
                <span className="mono cost">{c.mana_cost ?? ""}</span>
                <span className="spacer" />
                <button className="small" onClick={() => add(c)}>
                  + add
                </button>
                {draftAdd && (
                  <button
                    className="small"
                    title="Add to draft proposal"
                    onClick={() =>
                      draftAdd({
                        action: "add",
                        oracle_id: c.oracle_id,
                        card_name: c.name,
                        slot_id: targetSlot === "" ? null : Number(targetSlot),
                        rationale: "",
                      })
                    }
                  >
                    + propose
                  </button>
                )}
                {!hasCommander && !!c.is_commander && (
                  <button className="small" onClick={() => add(c, "commander")}>
                    + cmdr
                  </button>
                )}
              </div>
              <div className="muted type">{c.type_line}</div>
              {expanded === c.oracle_id && <pre className="oracle-text">{c.oracle_text}</pre>}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
