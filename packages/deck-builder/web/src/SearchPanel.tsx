import { useCallback, useEffect, useRef, useState, type FormEvent } from "react";
import { api, type CardData } from "./api.ts";
import { useDeck } from "./store.tsx";
import { ManaCost } from "./Mana.tsx";

const LIVE_SEARCH_DELAY_MS = 250;

export function SearchPanel() {
  const { deckId, state, run } = useDeck();
  const slots = state!.slots;
  const hasCommander = state!.cards.some((c) => c.role === "commander");
  // Where each card already stands in this deck, so a result can say so.
  const standing = new Map(state!.cards.map((c) => [c.oracle_id, c.maybe ? "maybe" : "in deck"]));
  const [query, setQuery] = useState("");
  const [filterIdentity, setFilterIdentity] = useState(true);
  const [results, setResults] = useState<CardData[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [searching, setSearching] = useState(false);
  const [targetSlot, setTargetSlot] = useState<string>("");
  const [expanded, setExpanded] = useState<string | null>(null);
  const debounceRef = useRef<number | null>(null);
  const requestRef = useRef<AbortController | null>(null);
  const requestGeneration = useRef(0);

  const search = useCallback(async () => {
    const normalizedQuery = query.trim();
    const generation = ++requestGeneration.current;

    if (debounceRef.current !== null) {
      window.clearTimeout(debounceRef.current);
      debounceRef.current = null;
    }
    requestRef.current?.abort();

    if (!normalizedQuery) {
      requestRef.current = null;
      setResults(null);
      setError(null);
      setSearching(false);
      return;
    }

    const request = new AbortController();
    requestRef.current = request;
    setSearching(true);
    setError(null);

    try {
      const nextResults = await api.search(normalizedQuery, deckId, filterIdentity, request.signal);
      if (generation === requestGeneration.current) setResults(nextResults);
    } catch (error: unknown) {
      if (generation !== requestGeneration.current || (error as { name?: string }).name === "AbortError") {
        return;
      }
      setError(error instanceof Error ? error.message : "Search failed");
      setResults(null);
    } finally {
      if (generation === requestGeneration.current) {
        requestRef.current = null;
        setSearching(false);
      }
    }
  }, [deckId, filterIdentity, query]);

  useEffect(() => {
    requestRef.current?.abort();
    requestRef.current = null;
    requestGeneration.current++;

    if (!query.trim()) {
      setResults(null);
      setError(null);
      setSearching(false);
      return;
    }

    setError(null);
    setSearching(true);
    debounceRef.current = window.setTimeout(() => void search(), LIVE_SEARCH_DELAY_MS);

    return () => {
      if (debounceRef.current !== null) {
        window.clearTimeout(debounceRef.current);
        debounceRef.current = null;
      }
      requestRef.current?.abort();
      requestRef.current = null;
      requestGeneration.current++;
    };
  }, [query, filterIdentity, deckId, search]);

  function submit(e: FormEvent) {
    e.preventDefault();
    void search();
  }

  function add(card: CardData, role?: string) {
    run(() =>
      api.addCard(deckId, card.oracle_id, {
        slotId: targetSlot === "" ? null : Number(targetSlot),
        role,
      }),
    );
  }

  return (
    <div className="search-panel">
      <form onSubmit={submit} className="stack gap">
        <div className="row gap">
          <input
            className="grow"
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder='t:creature o:"draw a card" mv<=3'
            aria-label="Search cards"
          />
          <button className="primary" type="submit" disabled={!query.trim()}>
            Search
          </button>
        </div>
        <div className="row gap wrap">
          <label
            className="own-toggle"
            title="Exclude cards outside the deck's color identity"
            style={{ display: "inline-flex" }}
          >
            <input
              type="checkbox"
              checked={filterIdentity}
              onChange={(e) => setFilterIdentity(e.target.checked)}
            />
            deck colors only
          </label>
          <span className="spacer" />
          <select
            value={targetSlot}
            onChange={(e) => setTargetSlot(e.target.value)}
            title="Adds land in this slot"
          >
            <option value="">→ unslotted</option>
            {slots.map((s) => (
              <option key={s.id} value={s.id}>
                → {s.name}
              </option>
            ))}
          </select>
        </div>
      </form>

      {error && <div className="error-banner">{error}</div>}

      {(results || searching) && (
        <div className="results" aria-busy={searching}>
          <div id="search-status" className="result-count" role="status" aria-live="polite">
            {searching
              ? results
                ? `Updating ${results.length} result(s)…`
                : "Searching…"
              : `${results?.length ?? 0} result(s)`}
          </div>
          {results?.map((c) => (
            <div className="result" key={c.oracle_id}>
              <div className="card-main">
                <span className="card-id">
                  <button
                    className="link name"
                    onClick={() => setExpanded(expanded === c.oracle_id ? null : c.oracle_id)}
                  >
                    {c.name}
                  </button>
                  <ManaCost cost={c.mana_cost} />
                </span>
                <span className="row-actions">
                  {!hasCommander && !!c.is_commander && (
                    <button className="small" title="Add as commander" onClick={() => add(c, "commander")}>
                      cmdr
                    </button>
                  )}
                  {standing.has(c.oracle_id) && (
                    <span className="chip" title="Already on this deck's list">
                      {standing.get(c.oracle_id)}
                    </span>
                  )}
                  {!standing.has(c.oracle_id) && (
                    <button
                      className="small"
                      title="Shortlist on the maybe list, outside the 100"
                      onClick={() => run(() => api.addCard(deckId, c.oracle_id, { maybe: true }))}
                    >
                      maybe
                    </button>
                  )}
                  <button className="small primary" title="Add to the deck now" onClick={() => add(c)}>
                    + add
                  </button>
                </span>
              </div>
              <div className="type">{c.type_line}</div>
              {expanded === c.oracle_id && <pre className="oracle-text">{c.oracle_text}</pre>}
            </div>
          ))}
          {!searching && results?.length === 0 && (
            <div className="muted rationale">Nothing matched that query.</div>
          )}
        </div>
      )}
    </div>
  );
}
