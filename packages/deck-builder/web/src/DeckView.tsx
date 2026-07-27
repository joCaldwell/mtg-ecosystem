import { useEffect, useRef, useState } from "react";
import { api, type DeckState, type DraftItem } from "./api.ts";
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
  const [state, setState] = useState<DeckState | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notFound, setNotFound] = useState(false);
  const [draft, setDraft] = useState<DraftItem[]>([]);
  const [tool, setTool] = useState<Tool | null>(null);
  // The chat draft outlives the chat panel, which unmounts on every tab flip.
  const [chatDraft, setChatDraft] = useState("");
  const showSideTab = useRef<((tab: "search" | "chat") => void) | null>(null);
  const openAudit = useRef<(() => void) | null>(null);
  const [groupBy, setGroupBy] = useState<GroupBy>(
    () => (localStorage.getItem("deck.groupBy") === "type" ? "type" : "slot"),
  );

  useEffect(() => {
    localStorage.setItem("deck.groupBy", groupBy);
  }, [groupBy]);

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
    if (name && name !== deck.name) await mutate(() => api.renameDeck(deck.id, name));
  }

  // "ask agent" on a finding: the token goes into the chat draft and the chat
  // comes up. The reference is a pointer the agent resolves with get_audit —
  // pasting the finding's text instead would put a stale copy in the
  // transcript forever, and the run is already on disk.
  function askAgent(token: string) {
    setChatDraft((d) => (d.trim() ? `${d.trimEnd()} ${token} ` : `${token} `));
    showSideTab.current?.("chat");
  }

  function draftAdd(item: DraftItem) {
    if (draft.some((d) => d.oracle_id === item.oracle_id && d.action === item.action)) return;
    setDraft([...draft, item]);
  }

  const rowProps = { deckId: deck.id, slots, tags, mutate, draftAdd };

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
        <a className="back" href="#/">
          ← decks
        </a>
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
        {/* The rail is `display: contents` on wide screens, so slots and the
            search/chat panel are independent columns there. Below 1400px it
            becomes a real element and the two share one sticky column. */}
        <div className="rail">
          <SlotPanel state={state} mutate={mutate} />

          <SidePanel
            search={
              <SearchPanel
                deckId={deck.id}
                slots={slots}
                hasCommander={commanders.length > 0}
                mutate={mutate}
                draftAdd={draftAdd}
              />
            }
            chat={
              <ChatPanel
                deckId={deck.id}
                setState={setState}
                input={chatDraft}
                setInput={setChatDraft}
                slots={slots}
                mutate={mutate}
              />
            }
            showRef={showSideTab}
          />
        </div>

        <section className="decklist">
          <div className="tool-bar">
            <button onClick={() => setTool("brief")}>Brief</button>
            <button onClick={() => openAudit.current?.()}>Audit</button>
            <button onClick={() => setTool("interop")}>Import / export</button>
            <button onClick={() => setTool("session")}>Context</button>
          </div>

          <ProposalSection state={state} draft={draft} setDraft={setDraft} mutate={mutate} />

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
              <CardRow key={c.oracle_id} card={c} {...rowProps} />
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
                    <CardRow key={c.oracle_id} card={c} {...rowProps} />
                  ))}
                </div>
              ),
          )}

          {companion.length > 0 && (
            <div className="group">
              <GroupHead title="Companion" count={companion.length} />
              {companion.map((c) => (
                <CardRow key={c.oracle_id} card={c} {...rowProps} />
              ))}
            </div>
          )}

          <AuditSection
            deckId={deck.id}
            revision={deck.revision}
            setState={setState}
            askAgent={askAgent}
            openRef={openAudit}
          />
        </section>
      </div>

      {tool === "brief" && (
        <Modal title="Brief" onClose={() => setTool(null)}>
          <BriefPanel deckId={deck.id} />
        </Modal>
      )}
      {tool === "interop" && (
        <Modal title="Archidekt & playtesting" onClose={() => setTool(null)} wide>
          <InteropPanel state={state} mutate={mutate} />
        </Modal>
      )}
      {tool === "session" && (
        <Modal title="Context & compaction" onClose={() => setTool(null)} wide>
          <SessionPanel state={state} setState={setState} />
        </Modal>
      )}
    </div>
  );
}
