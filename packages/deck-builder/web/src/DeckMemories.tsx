import { api } from "./api.ts";
import { useDeck } from "./store.tsx";
import { useLive } from "./live.tsx";
import { useLoaded } from "./lib.ts";
import { AddMemory, MemoryRow } from "./MemoryRow.tsx";

/** This deck's memories, beside the deck: what an agent opening it is shown. */
export function DeckMemories() {
  const { deckId, state } = useDeck();
  const live = useLive();
  const [list, error, reload] = useLoaded(() => api.listMemories({ scope: "deck", deck: deckId }), [deckId, live]);
  const budget = state!.memory_budget;

  return (
    <div className="search-panel memories-panel">
      <p className="muted rationale">
        Notes about this deck, written by agents without asking and by you. Nothing here is hidden from
        you, and you can edit or retire any of it.
      </p>
      {budget.omitted > 0 && (
        <div className="error-banner">
          {budget.omitted} of these no longer fit what an agent is shown when it opens this deck. Ask an
          agent to consolidate them, or retire some.
        </div>
      )}
      <AddMemory scope="deck" deckId={deckId} onAdded={reload} />
      {error && <div className="error-banner">{error}</div>}
      {list?.memories.map((m) => (
        <MemoryRow key={m.id} memory={m} onChanged={reload} showScope={false} />
      ))}
      {list && !list.memories.length && <div className="muted rationale">No memories for this deck yet.</div>}
      <a className="muted" href="#/memories">
        All memories →
      </a>
    </div>
  );
}
