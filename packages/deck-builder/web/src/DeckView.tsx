import { useEffect, useState } from "react";
import { api, type DeckState, type DraftItem } from "./api.ts";
import { SlotPanel } from "./SlotPanel.tsx";
import { SearchPanel } from "./SearchPanel.tsx";
import { CardRow } from "./CardRow.tsx";
import { ProposalSection } from "./ProposalSection.tsx";
import { AuditPanel } from "./AuditPanel.tsx";
import { BriefPanel } from "./BriefPanel.tsx";
import { ChatPanel } from "./ChatPanel.tsx";
import { InteropPanel } from "./InteropPanel.tsx";
import { SessionPanel } from "./SessionPanel.tsx";

export function DeckView({ deckId }: { deckId: number }) {
  const [state, setState] = useState<DeckState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notFound, setNotFound] = useState(false);
  const [draft, setDraft] = useState<DraftItem[]>([]);
  const [rightTab, setRightTab] = useState<"search" | "chat">("search");

  useEffect(() => {
    api
      .getDeck(deckId)
      .then(setState)
      .catch(() => setNotFound(true));
  }, [deckId]);

  async function mutate(fn: () => Promise<DeckState>) {
    setError(null);
    try {
      setState(await fn());
    } catch (e: any) {
      setError(e.message);
    }
  }

  if (notFound)
    return (
      <div className="deck-list-page">
        <p>Deck not found.</p>
        <a href="#/">← back to decks</a>
      </div>
    );
  if (!state) return <div className="deck-list-page">Loading…</div>;

  const { deck, slots, tags, cards, computed } = state;
  const commanders = cards.filter((c) => c.role === "commander");
  const companion = cards.filter((c) => c.role === "companion");
  const mainCards = cards.filter((c) => c.role === "card");

  const countClass = computed.delta_to_100 === 0 ? "ok" : computed.delta_to_100 < 0 ? "under" : "over";
  const violationCount =
    computed.identity_violations.length +
    computed.singleton_violations.length +
    computed.legality_violations.length;

  async function rename() {
    const name = window.prompt("Deck name", deck.name);
    if (name && name !== deck.name) await mutate(() => api.renameDeck(deck.id, name));
  }

  function draftAdd(item: DraftItem) {
    if (draft.some((d) => d.oracle_id === item.oracle_id && d.action === item.action)) return;
    setDraft([...draft, item]);
  }

  const rowProps = { deckId: deck.id, slots, tags, mutate, draftAdd };

  const groups: Array<{ key: string; title: string; cards: typeof mainCards; extra?: string }> = [
    ...slots.map((s) => {
      const delta = computed.slot_deltas.find((d) => d.slot_id === s.id)!;
      const target =
        s.target_min != null || s.target_max != null
          ? ` — ${delta.count}/${s.target_min ?? 0}–${s.target_max ?? "∞"}${delta.status !== "ok" && delta.status !== "untargeted" ? ` (${delta.status} ${Math.abs(delta.delta)})` : ""}`
          : ` — ${delta.count}`;
      return {
        key: `slot-${s.id}`,
        title: s.name,
        extra: target,
        cards: mainCards.filter((c) => c.slot_id === s.id),
      };
    }),
    {
      key: "unslotted",
      title: "Unslotted",
      extra: ` — ${computed.unslotted_count}`,
      cards: mainCards.filter((c) => c.slot_id == null),
    },
  ];

  return (
    <div className="deck-page">
      <header className="deck-header">
        <a href="#/">← decks</a>
        <h1 onClick={rename} title="Click to rename">
          {deck.name}
        </h1>
        <span className="mono identity">{deck.color_identity || "no commander"}</span>
        <span className={`chip ${countClass}`}>
          {computed.card_count}/100
          {computed.delta_to_100 !== 0 &&
            ` (${computed.delta_to_100 > 0 ? "+" : ""}${computed.delta_to_100})`}
        </span>
        <span className="chip">{computed.land_count} lands</span>
        {(computed.pending.adds > 0 || computed.pending.cuts > 0) && (
          <span className="chip under" title="Open proposal items">
            pending +{computed.pending.adds}/−{computed.pending.cuts} → {computed.pending.projected_count}
          </span>
        )}
        {violationCount > 0 && <span className="chip over">{violationCount} violation(s)</span>}
      </header>

      {error && <div className="error-banner">{error}</div>}

      {violationCount > 0 && (
        <div className="violations">
          {computed.identity_violations.map((v) => (
            <div key={v.oracle_id}>
              ⚠ <b>{v.name}</b> ({v.color_identity}) is outside the deck's color identity
            </div>
          ))}
          {computed.singleton_violations.map((v) => (
            <div key={v.oracle_id}>
              ⚠ <b>{v.name}</b> ×{v.quantity} exceeds its copy limit ({v.limit ?? "∞"})
            </div>
          ))}
          {computed.legality_violations.map((v) => (
            <div key={v.oracle_id}>
              ⚠ <b>{v.name}</b> is {v.legality.replace("_", " ")} in Commander
            </div>
          ))}
        </div>
      )}

      <div className="columns">
        <SlotPanel state={state} mutate={mutate} />

        <section className="decklist">
          <BriefPanel deckId={deck.id} />
          <AuditPanel deckId={deck.id} setState={setState} />
          <InteropPanel state={state} mutate={mutate} />
          <SessionPanel state={state} setState={setState} />
          <ProposalSection state={state} draft={draft} setDraft={setDraft} mutate={mutate} />

          <div className="group">
            <h2>
              Command Zone <span className="muted">— {commanders.length}/2</span>
            </h2>
            {commanders.map((c) => (
              <CardRow key={c.oracle_id} card={c} {...rowProps} />
            ))}
            {!commanders.length && (
              <p className="muted">No commander — search for a legendary creature and add it with role “Commander”.</p>
            )}
          </div>

          {groups.map(
            (g) =>
              (g.cards.length > 0 || g.key !== "unslotted") && (
                <div className="group" key={g.key}>
                  <h2>
                    {g.title}
                    <span className="muted">{g.extra}</span>
                  </h2>
                  {g.cards.map((c) => (
                    <CardRow key={c.oracle_id} card={c} {...rowProps} />
                  ))}
                </div>
              ),
          )}

          {companion.length > 0 && (
            <div className="group">
              <h2>Companion</h2>
              {companion.map((c) => (
                <CardRow key={c.oracle_id} card={c} {...rowProps} />
              ))}
            </div>
          )}
        </section>

        <aside className="search-panel">
          <div className="row gap tab-bar">
            <button
              className={`tab ${rightTab === "search" ? "active" : ""}`}
              onClick={() => setRightTab("search")}
            >
              Search
            </button>
            <button
              className={`tab ${rightTab === "chat" ? "active" : ""}`}
              onClick={() => setRightTab("chat")}
            >
              Agent chat
            </button>
          </div>
          {rightTab === "search" ? (
            <SearchPanel
              deckId={deck.id}
              slots={slots}
              hasCommander={commanders.length > 0}
              mutate={mutate}
              draftAdd={draftAdd}
            />
          ) : (
            <ChatPanel deckId={deck.id} setState={setState} />
          )}
        </aside>
      </div>
    </div>
  );
}
