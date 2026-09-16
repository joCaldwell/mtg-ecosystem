import { useEffect, useRef, useState, type FormEvent } from "react";
import { api, type DraftItem } from "./api.ts";
import { DeckProvider, useDeck } from "./store.tsx";
import { useLocalStorage } from "./lib.ts";
import { SlotPanel } from "./SlotPanel.tsx";
import { SearchPanel } from "./SearchPanel.tsx";
import { CardRow, type OwnershipMode } from "./CardRow.tsx";
import { ProposalSection } from "./ProposalSection.tsx";
import { DecisionLog } from "./DecisionLog.tsx";
import { AuditSection, DeckChecks } from "./AuditSection.tsx";
import { BriefPanel } from "./BriefPanel.tsx";
import { ChatPanel } from "./ChatPanel.tsx";
import { InteropPanel } from "./InteropPanel.tsx";
import { SessionPanel } from "./SessionPanel.tsx";
import { SidePanel } from "./SidePanel.tsx";
import { ColorPips } from "./Mana.tsx";
import { usePeekProps } from "./CardPeek.tsx";
import { Modal } from "./Modal.tsx";
import { ConfirmDialog } from "./ConfirmDialog.tsx";

type GroupStatus = "ok" | "under" | "over" | "untargeted";
type GroupBy = "slot" | "type";
// Tool panels open over the page instead of stacking above the decklist —
// their variable-height contents otherwise move the start of the deck. Audit
// stays below the list because its recorded output is part of the deck record.
type Tool = "proposals" | "brief" | "interop" | "session" | "checks";

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
  const { deckId, state, audit, error, apply, run, setSideTab } = useDeck();
  const [notFound, setNotFound] = useState(false);
  const [draft, setDraft] = useState<DraftItem[]>([]);
  const [tool, setTool] = useState<Tool | null>(null);
  const [renameDraft, setRenameDraft] = useState<string | null>(null);
  const [renaming, setRenaming] = useState(false);
  const [confirmAllOwned, setConfirmAllOwned] = useState(false);
  const insertChat = useRef<((token: string) => void) | null>(null);
  const openAudit = useRef<(() => void) | null>(null);
  const [groupBy, setGroupBy] = useLocalStorage<GroupBy>(
    "deck.groupBy",
    (raw) => (raw === "type" ? "type" : "slot"),
    (v) => v,
  );
  const [ownershipMode, setOwnershipMode] = useLocalStorage<OwnershipMode>(
    "deck.ownershipMode",
    (raw) => (raw === "missing" ? "missing" : "owned"),
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

  const { deck, slots, tags, cards, computed, proposals, brief_edits } = state;
  const commanders = cards.filter((c) => c.role === "commander");
  const companion = cards.filter((c) => c.role === "companion");
  const mainCards = cards.filter((c) => c.role === "card");
  const ownedCount = cards.reduce((n, c) => n + (c.owned ? c.quantity : 0), 0);
  const missingCount = cards.reduce((n, c) => n + (c.owned ? 0 : c.quantity), 0);

  const problemCount = audit ? audit.findings.length + audit.dismissed.length : null;
  // A draft is one proposal-in-progress even when it contains several cards.
  const proposalCount = proposals.length + brief_edits.length + (draft.length ? 1 : 0);

  async function rename(e: FormEvent) {
    e.preventDefault();
    const name = renameDraft?.trim();
    if (!name) return;
    if (name === deck.name) {
      setRenameDraft(null);
      return;
    }
    setRenaming(true);
    const result = await run(() => api.renameDeck(deck.id, name));
    setRenaming(false);
    if (result) setRenameDraft(null);
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
    setTool("proposals");
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
        {renameDraft == null ? (
          <h1>
            <button
              className="link deck-name-trigger"
              onClick={() => setRenameDraft(deck.name)}
              title="Click to rename"
              aria-label={`Rename ${deck.name}`}
            >
              {deck.name}
            </button>
          </h1>
        ) : (
          <form className="deck-name-edit" onSubmit={rename}>
            <input
              value={renameDraft}
              onChange={(e) => setRenameDraft(e.target.value)}
              onFocus={(e) => e.currentTarget.select()}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  e.preventDefault();
                  setRenameDraft(null);
                }
              }}
              aria-label="Deck name"
              autoFocus
              disabled={renaming}
            />
            <button
              className="icon primary"
              type="submit"
              title="Save name"
              aria-label="Save name"
              disabled={renaming || !renameDraft.trim()}
            >
              ✓
            </button>
            <button
              className="icon"
              type="button"
              title="Cancel rename"
              aria-label="Cancel rename"
              onClick={() => setRenameDraft(null)}
              disabled={renaming}
            >
              ✕
            </button>
          </form>
        )}
        {/* Keep commander and colour identity beside the deck name. Counts
            and pending changes live in the sidebar overview. */}
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
        <button
          className={`small ${problemCount ? "deck-problems-button" : ""}`}
          onClick={() => setTool("checks")}
          aria-haspopup="dialog"
        >
          {problemCount == null ? "Deck checks" : problemCount ? `${problemCount} issue${problemCount === 1 ? "" : "s"}` : "Deck checks clear"}
        </button>
      </header>

      {error && <div className="error-banner">{error}</div>}

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
          <div className="deck-content">
            <div className="tool-bar">
              <button
                className={proposalCount ? "has-pending" : ""}
                onClick={() => setTool("proposals")}
                aria-haspopup="dialog"
              >
                Proposals
                {proposalCount > 0 && <span className="tool-count">{proposalCount}</span>}
              </button>
              <button onClick={() => setTool("brief")}>Brief</button>
              <button onClick={() => openAudit.current?.()}>Audit</button>
              <button onClick={() => setTool("interop")}>Import / export</button>
              <button onClick={() => setTool("session")}>Context</button>
            </div>

            <div className="group-by">
              <div className="control-set">
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
              <div className="control-set ownership-controls">
                <span className="muted">Mark</span>
                <div className="seg ownership-mode">
                  <button
                    className={ownershipMode === "owned" ? "active" : ""}
                    aria-pressed={ownershipMode === "owned"}
                    onClick={() => setOwnershipMode("owned")}
                    title="Checked cards are cards you own"
                  >
                    Owned <span className="seg-count">{ownedCount}</span>
                  </button>
                  <button
                    className={ownershipMode === "missing" ? "active" : ""}
                    aria-pressed={ownershipMode === "missing"}
                    onClick={() => setOwnershipMode("missing")}
                    title="Checked cards are cards you still need"
                  >
                    Missing <span className="seg-count">{missingCount}</span>
                  </button>
                </div>
                {ownedCount < ownedCount + missingCount && (
                  <button
                    className="baseline-action"
                    onClick={() => setConfirmAllOwned(true)}
                    title="Start with the whole deck owned, then mark missing exceptions"
                    aria-haspopup="dialog"
                  >
                    Mark all owned
                  </button>
                )}
              </div>
            </div>

            <div className="card-group">
              <GroupHead title="Command zone" count={commanders.length} target="2" />
              {commanders.map((c) => (
                <CardRow
                  key={c.oracle_id}
                  card={c}
                  ownershipMode={ownershipMode}
                />
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
                  <div className="card-group" key={g.key}>
                    <GroupHead
                      title={g.title}
                      count={g.count}
                      target={g.target}
                      status={g.status}
                      delta={g.delta}
                    />
                    {g.cards.map((c) => (
                      <CardRow
                        key={c.oracle_id}
                        card={c}
                        ownershipMode={ownershipMode}
                      />
                    ))}
                  </div>
                ),
            )}

            {companion.length > 0 && (
              <div className="card-group">
                <GroupHead title="Companion" count={companion.length} />
                {companion.map((c) => (
                  <CardRow
                    key={c.oracle_id}
                    card={c}
                    ownershipMode={ownershipMode}
                  />
                ))}
              </div>
            )}

            <AuditSection askAgent={askAgent} openRef={openAudit} />
            <DecisionLog />
          </div>
        </section>
      </div>

      {tool === "proposals" && (
        <Modal title={`Proposals${proposalCount ? ` · ${proposalCount}` : ""}`} onClose={() => setTool(null)} wide>
          <ProposalSection draft={draft} setDraft={setDraft} />
        </Modal>
      )}
      {tool === "checks" && (
        <Modal title="Deck problems" onClose={() => setTool(null)} wide>
          <DeckChecks askAgent={(token) => { askAgent(token); setTool(null); }} />
        </Modal>
      )}
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
      {confirmAllOwned && (
        <ConfirmDialog
          title="Mark all cards owned"
          confirmLabel="Mark all owned"
          onClose={() => setConfirmAllOwned(false)}
          onConfirm={() => {
            setConfirmAllOwned(false);
            void run(() => api.setAllCardsOwned(deckId, true));
          }}
        >
          <p>Mark every card in this deck as owned?</p>
          <p className="muted">You can then mark only the exceptions as missing.</p>
        </ConfirmDialog>
      )}
    </div>
  );
}
