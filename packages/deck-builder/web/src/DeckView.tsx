import { useEffect, useRef, useState } from "react";
import { api, type DraftItem } from "./api.ts";
import { DeckProvider, useDeck } from "./store.tsx";
import { useLocalStorage } from "./lib.ts";
import { SlotPanel } from "./SlotPanel.tsx";
import { SearchPanel } from "./SearchPanel.tsx";
import { CardRow } from "./CardRow.tsx";
import { ProposalSection } from "./ProposalSection.tsx";
import { AuditSection } from "./AuditSection.tsx";
import { BriefPanel } from "./BriefPanel.tsx";
import { ChatPanel } from "./ChatPanel.tsx";
import { InteropPanel } from "./InteropPanel.tsx";
import { SessionPanel } from "./SessionPanel.tsx";
import { SidePanel } from "./SidePanel.tsx";
import { ColorPips } from "./Mana.tsx";
import { usePeekProps } from "./CardPeek.tsx";
import { Modal } from "./Modal.tsx";

type GroupStatus = "ok" | "under" | "over" | "untargeted";
type GroupBy = "slot" | "type";
// Tool panels open over the page instead of stacking above the decklist —
// they are visit-and-leave surfaces, and inline they pushed the deck itself
// off the screen. Two exceptions stay inline, both because they are things you
// rule on rather than visit: proposals above the list, and the audit below it.
type Tool = "brief" | "interop" | "session";

// Display order for card-type grouping. `primaryType` walks this list, so a
// card with several types files under the first one that matches — an Artifact
// Creature is a Creature. Land is the exception and wins outright, because the
// header's land count is "any type line containing Land" and the two numbers
// have to agree (Dryad Arbor is a land in both).
const TYPE_GROUPS = [
  "Creature",
  "Planeswalker",
  "Battle",
  "Instant",
  "Sorcery",
  "Artifact",
  "Enchantment",
  "Land",
];

function primaryType(typeLine: string): string {
  if (/\bLand\b/.test(typeLine)) return "Land";
  return TYPE_GROUPS.find((t) => new RegExp(`\\b${t}\\b`).test(typeLine)) ?? "Other";
}

function GroupHead({
  title,
  count,
  target,
  status,
  delta,
}: {
  title: string;
  count: number;
  target?: string | null;
  status?: GroupStatus;
  delta?: number;
}) {
  const off = status === "under" || status === "over";
  return (
    <h2 className="group-head">
      <span>{title}</span>
      <span className={`count ${off ? status : ""}`}>
        {count}
        {target && `/${target}`}
        {off && ` ${status === "under" ? "▼" : "▲"}${Math.abs(delta ?? 0)}`}
      </span>
      <span className="rule" />
    </h2>
  );
}

export function DeckView({ deckId }: { deckId: number }) {
  return (
    <DeckProvider deckId={deckId}>
      <DeckPage />
    </DeckProvider>
  );
}

function DeckPage() {
  const { deckId, state, error, apply, run, setSideTab } = useDeck();
  const [notFound, setNotFound] = useState(false);
  const [draft, setDraft] = useState<DraftItem[]>([]);
  const [tool, setTool] = useState<Tool | null>(null);
  const insertChat = useRef<((token: string) => void) | null>(null);
  const openAudit = useRef<(() => void) | null>(null);
  const [groupBy, setGroupBy] = useLocalStorage<GroupBy>(
    "deck.groupBy",
    (raw) => (raw === "type" ? "type" : "slot"),
    (v) => v,
  );
  const peekProps = usePeekProps();

  useEffect(() => {
    api
      .getDeck(deckId)
      .then(apply)
      .catch(() => setNotFound(true));
  }, [deckId, apply]);

  if (notFound)
    return (
      <div className="deck-list-page">
        <p>Deck not found.</p>
        <a href="#/">← back to decks</a>
      </div>
    );
  if (!state) return <div className="deck-list-page muted">Loading…</div>;

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
    if (name && name !== deck.name) await run(() => api.renameDeck(deck.id, name));
  }

  // "ask agent" on a finding: the token goes into the chat draft and the chat
  // comes up. The reference is a pointer the agent resolves with get_audit —
  // pasting the finding's text instead would put a stale copy in the
  // transcript forever, and the run is already on disk.
  function askAgent(token: string) {
    insertChat.current?.(token);
    setSideTab("chat");
  }

  function draftAdd(item: DraftItem) {
    if (draft.some((d) => d.oracle_id === item.oracle_id && d.action === item.action)) return;
    setDraft([...draft, item]);
  }

  type Group = {
    key: string;
    title: string;
    count: number;
    target: string | null;
    status: GroupStatus;
    delta: number;
    cards: typeof mainCards;
    /** Keep an empty slot visible so its unmet target still shows. */
    keepEmpty?: boolean;
  };

  // Counts are quantity-weighted in both modes, matching computeState() — a
  // group holding 6× Forest reads 6, not 1.
  const qty = (cs: typeof mainCards) => cs.reduce((n, c) => n + c.quantity, 0);

  const slotGroups: Group[] = [
    ...slots.map((s) => {
      const delta = computed.slot_deltas.find((d) => d.slot_id === s.id)!;
      return {
        key: `slot-${s.id}`,
        title: s.name,
        count: delta.count,
        target:
          s.target_min != null || s.target_max != null
            ? `${s.target_min ?? 0}–${s.target_max ?? "∞"}`
            : null,
        status: delta.status as GroupStatus,
        delta: delta.delta,
        cards: mainCards.filter((c) => c.slot_id === s.id),
        keepEmpty: true,
      };
    }),
    {
      key: "unslotted",
      title: "Unslotted",
      count: computed.unslotted_count,
      target: null,
      status: "untargeted" as GroupStatus,
      delta: 0,
      cards: mainCards.filter((c) => c.slot_id == null),
    },
  ];

  // No targets here: a target is a property of a slot, not of a card type.
  const typeGroups: Group[] = [...TYPE_GROUPS, "Other"]
    .map((t) => {
      const cards = mainCards.filter((c) => primaryType(c.type_line) === t);
      return {
        key: `type-${t}`,
        title: t,
        count: qty(cards),
        target: null,
        status: "untargeted" as GroupStatus,
        delta: 0,
        cards,
      };
    })
    .filter((g) => g.cards.length > 0);

  const groups = groupBy === "slot" ? slotGroups : typeGroups;

  return (
    <div className="deck-page">
      <header className="deck-header">
        <a className="back" href="#/" title="Back to decks" aria-label="Back to decks">
          <svg viewBox="0 0 14 14" width="14" height="14" aria-hidden="true" focusable="false">
            <path
              d="M8.5 2.5 4 7l4.5 4.5"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.6"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        </a>
        <h1 onClick={rename} title="Click to rename">
          {deck.name}
        </h1>
        {/* The commander is what the deck is, and every number to its right is
            downstream of it — the colour identity most of all. Peekable like
            any other card name: it's the one card you re-read constantly. */}
        {commanders.length > 0 && (
          <span className="commander">
            {commanders.map((c, i) => (
              <span key={c.oracle_id}>
                {i > 0 && <span className="muted"> + </span>}
                <span className="peekable" {...peekProps(c.name)}>
                  {c.name}
                </span>
              </span>
            ))}
          </span>
        )}
        {/* An empty identity means two different things — no commander yet, or
            a colorless one — and the letters conflated them. */}
        {commanders.length === 0 ? (
          <span className="mono identity">no commander</span>
        ) : (
          <ColorPips colors={deck.color_identity || "C"} />
        )}
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
        {/* The rail is `display: contents` on wide screens, so slots and the
            search/chat panel are independent columns there. Below 1400px it
            becomes a real element and the two share one sticky column. */}
        <div className="rail">
          <SlotPanel />

          <SidePanel
            search={<SearchPanel draftAdd={draftAdd} />}
            chat={<ChatPanel insertRef={insertChat} />}
          />
        </div>

        <section className="decklist">
          <div className="tool-bar">
            <button onClick={() => setTool("brief")}>Brief</button>
            <button onClick={() => openAudit.current?.()}>Audit</button>
            <button onClick={() => setTool("interop")}>Import / export</button>
            <button onClick={() => setTool("session")}>Context</button>
          </div>

          <ProposalSection draft={draft} setDraft={setDraft} />

          <div className="row gap group-by">
            <span className="muted">Group by</span>
            <div className="seg">
              <button
                className={groupBy === "slot" ? "active" : ""}
                onClick={() => setGroupBy("slot")}
                title="The deck's own roles, with their targets"
              >
                Slot
              </button>
              <button
                className={groupBy === "type" ? "active" : ""}
                onClick={() => setGroupBy("type")}
                title="Creature, instant, land… — counts what the cards are"
              >
                Card type
              </button>
            </div>
          </div>

          <div className="group">
            <GroupHead title="Command zone" count={commanders.length} target="2" />
            {commanders.map((c) => (
              <CardRow key={c.oracle_id} card={c} draftAdd={draftAdd} />
            ))}
            {!commanders.length && (
              <p className="muted rationale">
                No commander — search for a legendary creature and add it with role “Commander”.
              </p>
            )}
          </div>

          {groups.map(
            (g) =>
              (g.cards.length > 0 || g.keepEmpty) && (
                <div className="group" key={g.key}>
                  <GroupHead
                    title={g.title}
                    count={g.count}
                    target={g.target}
                    status={g.status}
                    delta={g.delta}
                  />
                  {g.cards.map((c) => (
                    <CardRow key={c.oracle_id} card={c} draftAdd={draftAdd} />
                  ))}
                </div>
              ),
          )}

          {companion.length > 0 && (
            <div className="group">
              <GroupHead title="Companion" count={companion.length} />
              {companion.map((c) => (
                <CardRow key={c.oracle_id} card={c} draftAdd={draftAdd} />
              ))}
            </div>
          )}

          <AuditSection askAgent={askAgent} openRef={openAudit} />
        </section>
      </div>

      {tool === "brief" && (
        <Modal title="Brief" onClose={() => setTool(null)}>
          <BriefPanel />
        </Modal>
      )}
      {tool === "interop" && (
        <Modal title="Archidekt & playtesting" onClose={() => setTool(null)} wide>
          <InteropPanel />
        </Modal>
      )}
      {tool === "session" && (
        <Modal title="Context & compaction" onClose={() => setTool(null)} wide>
          <SessionPanel />
        </Modal>
      )}
    </div>
  );
}
