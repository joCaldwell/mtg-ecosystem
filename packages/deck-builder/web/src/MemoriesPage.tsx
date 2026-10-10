import { useEffect, useState } from "react";
import { api, type DeckSummary } from "./api.ts";
import { useLive } from "./live.tsx";
import { useLoaded } from "./lib.ts";
import { AddMemory, MemoryRow } from "./MemoryRow.tsx";

/** Every memory (spec §7.5): browse by scope and deck, search, edit, retire, restore. */
export function MemoriesPage() {
  const live = useLive();
  const [scope, setScope] = useState<"" | "global" | "deck">("");
  const [deck, setDeck] = useState<number | "">("");
  const [status, setStatus] = useState<"active" | "any">("active");
  const [query, setQuery] = useState("");
  const [decks, setDecks] = useState<DeckSummary[]>([]);
  useEffect(() => {
    void api.listDecks().then(setDecks);
  }, [live]);

  const [list, error, reload] = useLoaded(
    () =>
      api.listMemories({
        scope: deck !== "" ? "deck" : scope,
        deck: deck === "" ? undefined : deck,
        status,
        q: query.trim(),
      }),
    [scope, deck, status, query, live],
  );
  const g = list?.global_budget;

  return (
    <div className="page">
      <header className="deck-list-heading">
        <h1>Memories</h1>
        <span className="muted">What agents have noted about you and your decks</span>
      </header>
      <p className="muted">
        Agents write these without asking. You can read all of them, and edit or retire any. One you write
        or edit becomes yours, and no agent can change it after that.
      </p>
      {g && (
        <p className={g.omitted ? "error-banner" : "muted"}>
          Global memories shown to an agent at the start of a session: about {g.used} of {g.budget} tokens
          {g.omitted > 0 && ` — ${g.omitted} did not fit. Ask an agent to consolidate them, or retire some.`}
        </p>
      )}

      <div className="row gap wrap filters">
        <input
          className="grow"
          type="search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search memories"
          aria-label="Search memories"
        />
        <select
          value={deck !== "" ? `deck:${deck}` : scope}
          onChange={(e) => {
            const v = e.target.value;
            if (v.startsWith("deck:")) {
              setDeck(Number(v.slice(5)));
              setScope("deck");
            } else {
              setDeck("");
              setScope(v as "" | "global" | "deck");
            }
          }}
          aria-label="Scope"
        >
          <option value="">everything</option>
          <option value="global">global only</option>
          <option value="deck">every deck</option>
          {decks.map((d) => (
            <option key={d.id} value={`deck:${d.id}`}>
              {d.name}
            </option>
          ))}
        </select>
        <label className="own-toggle" style={{ display: "inline-flex" }}>
          <input type="checkbox" checked={status === "any"} onChange={(e) => setStatus(e.target.checked ? "any" : "active")} />
          include retired and superseded
        </label>
      </div>

      {deck === "" && scope !== "deck" && <AddMemory scope="global" onAdded={reload} />}
      {deck !== "" && <AddMemory scope="deck" deckId={deck} onAdded={reload} />}

      {error && <div className="error-banner">{error}</div>}
      {list?.memories.map((m) => (
        <MemoryRow key={m.id} memory={m} onChanged={reload} />
      ))}
      {list && !list.memories.length && <p className="muted">No memories match.</p>}
    </div>
  );
}
