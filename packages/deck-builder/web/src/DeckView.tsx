import { useEffect, useState, type FormEvent } from "react";
import { api, type DeckComparison, type DeckSummary, type PaperStatus } from "./api.ts";
import { DeckProvider, useDeck } from "./store.tsx";
import { useLive } from "./live.tsx";
import { useLocalStorage } from "./lib.ts";
import { SlotPanel } from "./SlotPanel.tsx";
import { SearchPanel } from "./SearchPanel.tsx";
import { CardRow, type PaperMode } from "./CardRow.tsx";
import { ProposalSection } from "./ProposalSection.tsx";
import { RulingsLog } from "./RulingsLog.tsx";
import { BriefPanel } from "./BriefPanel.tsx";
import { DeckMemories } from "./DeckMemories.tsx";
import { InteropPanel } from "./InteropPanel.tsx";
import { SidePanel } from "./SidePanel.tsx";
import { ColorPips } from "./Mana.tsx";
import { usePeekProps } from "./CardPeek.tsx";
import { Modal } from "./Modal.tsx";
import { ConfirmDialog } from "./ConfirmDialog.tsx";

type GroupStatus = "ok" | "under" | "over" | "untargeted";
type GroupBy = "slot" | "type";
// Tool panels open over the page instead of stacking above the decklist —
// their variable-height contents otherwise move the start of the deck.
type Tool = "proposals" | "brief" | "interop" | "checks" | "paper" | "compare" | "fork";

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
  const { deckId, state, error, apply, run, selected } = useDeck();
  const live = useLive();
  const [notFound, setNotFound] = useState(false);
  const [tool, setTool] = useState<Tool | null>(null);
  const [renameDraft, setRenameDraft] = useState<string | null>(null);
  const [renaming, setRenaming] = useState(false);
  const [confirmAllPaper, setConfirmAllPaper] = useState(false);
  const [groupBy, setGroupBy] = useLocalStorage<GroupBy>(
    "deck.groupBy",
    (raw) => (raw === "type" ? "type" : "slot"),
    (v) => v,
  );
  const [paperMode, setPaperMode] = useLocalStorage<PaperMode>(
    "deck.paperMode",
    (raw) => (raw === "missing" ? "missing" : "in_paper"),
    (v) => v,
  );
  const peekProps = usePeekProps();

  // Refetched whenever the server reports a change, so what an agent does in
  // another window shows up here without a reload.
  useEffect(() => {
    api
      .getDeck(deckId)
      .then(apply)
      .catch(() => setNotFound(true));
  }, [deckId, apply, live]);

  // Focus (spec §10.3): tell the app which deck is open and which cards are
  // expanded, so "this deck" in a conversation with an agent means this one.
  const selectedKey = selected.join(",");
  useEffect(() => {
    void api.setFocus(deckId, selectedKey ? selectedKey.split(",") : []).catch(() => {});
    const beat = window.setInterval(
      () => void api.setFocus(deckId, selectedKey ? selectedKey.split(",") : []).catch(() => {}),
      60_000,
    );
    return () => window.clearInterval(beat);
  }, [deckId, selectedKey]);
  useEffect(() => () => void api.setFocus(null).catch(() => {}), []);

  if (notFound)
    return (
      <div className="deck-list-page">
        <p>Deck not found.</p>
        <a href="#/">← back to decks</a>
      </div>
    );
  if (!state) return <div className="deck-list-page muted">Loading…</div>;

  const { deck, slots, cards, computed, proposals, document_edits, checks } = state;
  const inDeck = cards.filter((c) => !c.maybe);
  const maybe = cards.filter((c) => c.maybe);
  const commanders = inDeck.filter((c) => c.role === "commander");
  const companion = inDeck.filter((c) => c.role === "companion");
  const mainCards = inDeck.filter((c) => c.role === "card");
  const inPaperCount = inDeck.reduce((n, c) => n + (c.in_paper ? c.quantity : 0), 0);
  const missingCount = inDeck.reduce((n, c) => n + (c.in_paper ? 0 : c.quantity), 0);

  const problemCount = checks.length;
  const proposalCount =
    proposals.reduce((n, p) => n + p.items.filter((i) => i.status === "pending").length, 0) +
    document_edits.length;
  const isDraft = deck.status === "draft";

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
        {/* Spec §5.2: an agent edits a draft directly and changes an active
            deck only by proposal. Only the owner flips this. */}
        <div className="seg deck-status" role="group" aria-label="Deck status">
          <button
            className={isDraft ? "active" : ""}
            aria-pressed={isDraft}
            onClick={() => !isDraft && run(() => api.setDeckStatus(deck.id, "draft"))}
            title="Draft: an agent may add and remove cards directly"
          >
            Draft
          </button>
          <button
            className={!isDraft ? "active" : ""}
            aria-pressed={!isDraft}
            onClick={() => isDraft && run(() => api.setDeckStatus(deck.id, "active"))}
            title="Active: an agent's card changes come to you as proposals"
          >
            Active
          </button>
        </div>
        {deck.parent && (
          <span className="muted fork-of">
            fork of <a href={`#/deck/${deck.parent.id}`}>{deck.parent.name}</a>
          </span>
        )}
        <span className="mono muted" title="Bumped once per change to the list">
          rev {deck.revision}
        </span>
        <button
          className={`small ${problemCount ? "deck-problems-button" : ""}`}
          onClick={() => setTool("checks")}
          aria-haspopup="dialog"
        >
          {problemCount ? `${problemCount} issue${problemCount === 1 ? "" : "s"}` : "Deck checks clear"}
        </button>
      </header>

      {error && <div className="error-banner">{error}</div>}

      <div className="columns">
        {/* The rail is `display: contents` on wide screens, so slots and the
            search/memories panel are independent columns there. Below 1400px it
            becomes a real element and the two share one sticky column. */}
        <div className="rail">
          <SlotPanel />

          <SidePanel search={<SearchPanel />} memories={<DeckMemories />} />
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
              <button onClick={() => setTool("interop")}>Import / export</button>
              <button onClick={() => setTool("paper")}>Paper list</button>
              <button onClick={() => setTool("compare")}>Compare</button>
              <button onClick={() => setTool("fork")} title="Copy this deck into a new draft and keep this one">
                Fork
              </button>
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
                <span className="muted" title="Whether the physical card is in the physical deck. Not shown to agents unless they ask for the paper list.">
                  Paper deck
                </span>
                <div className="seg ownership-mode">
                  <button
                    className={paperMode === "in_paper" ? "active" : ""}
                    aria-pressed={paperMode === "in_paper"}
                    onClick={() => setPaperMode("in_paper")}
                    title="Checked cards are physically in the deck"
                  >
                    In paper <span className="seg-count">{inPaperCount}</span>
                  </button>
                  <button
                    className={paperMode === "missing" ? "active" : ""}
                    aria-pressed={paperMode === "missing"}
                    onClick={() => setPaperMode("missing")}
                    title="Checked cards are still missing from the physical deck"
                  >
                    Missing <span className="seg-count">{missingCount}</span>
                  </button>
                </div>
                {missingCount > 0 && (
                  <button
                    className="baseline-action"
                    onClick={() => setConfirmAllPaper(true)}
                    title="Start with the whole deck assembled, then mark the exceptions as missing"
                    aria-haspopup="dialog"
                  >
                    Mark all in paper
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
                  paperMode={paperMode}
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
                        paperMode={paperMode}
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
                    paperMode={paperMode}
                  />
                ))}
              </div>
            )}

            {maybe.length > 0 && (
              <div className="card-group maybe-group">
                <GroupHead title="Maybe list — outside the 100" count={maybe.length} />
                {maybe.map((c) => (
                  <CardRow key={c.oracle_id} card={c} paperMode={paperMode} />
                ))}
              </div>
            )}

            <RulingsLog />
          </div>
        </section>
      </div>

      {tool === "proposals" && (
        <Modal title={`Awaiting your ruling${proposalCount ? ` · ${proposalCount}` : ""}`} onClose={() => setTool(null)} wide>
          <ProposalSection />
        </Modal>
      )}
      {tool === "checks" && (
        <Modal title="Deck checks" onClose={() => setTool(null)} wide>
          {checks.length === 0 && <p className="muted">No problems found. These checks are computed from the current list.</p>}
          {checks.map((f) => (
            <div className="log-row" key={f.key}>
              <span className={`chip ${f.severity === "error" ? "over" : "under"}`}>
                {f.severity === "error" ? "problem" : "note"}
              </span>
              <span className="name">{f.title}</span>
              <span className="reason">{f.detail}</span>
            </div>
          ))}
        </Modal>
      )}
      {tool === "brief" && (
        <Modal title="Brief" onClose={() => setTool(null)}>
          <BriefPanel />
        </Modal>
      )}
      {tool === "interop" && (
        <Modal title="Import / export" onClose={() => setTool(null)} wide>
          <InteropPanel />
        </Modal>
      )}
      {tool === "paper" && (
        <Modal title="Paper list" onClose={() => setTool(null)} wide>
          <PaperList deckId={deck.id} />
        </Modal>
      )}
      {tool === "compare" && (
        <Modal title="Compare decks" onClose={() => setTool(null)} wide>
          <Compare />
        </Modal>
      )}
      {tool === "fork" && <ForkDialog deckId={deck.id} name={deck.name} onClose={() => setTool(null)} />}
      {confirmAllPaper && (
        <ConfirmDialog
          title="Mark all cards in paper"
          confirmLabel="Mark all in paper"
          onClose={() => setConfirmAllPaper(false)}
          onConfirm={() => {
            setConfirmAllPaper(false);
            void run(() => api.setAllInPaper(deckId, true));
          }}
        >
          <p>Mark every card in this deck as physically in the deck?</p>
          <p className="muted">You can then mark only the exceptions as missing.</p>
        </ConfirmDialog>
      )}
    </div>
  );
}

function ForkDialog({ deckId, name, onClose }: { deckId: number; name: string; onClose: () => void }) {
  const [forkName, setForkName] = useState(`${name} (fork)`);
  const [error, setError] = useState<string | null>(null);
  async function submit(e: FormEvent) {
    e.preventDefault();
    try {
      const { state } = await api.forkDeck(deckId, forkName);
      onClose();
      window.location.hash = `#/deck/${state.deck.id}`;
    } catch (err: any) {
      setError(err.message);
    }
  }
  return (
    <Modal title="Fork this deck" onClose={onClose}>
      <form onSubmit={submit} className="stack gap">
        <p className="muted">
          Copies the list, slots, tags, maybe list, brief and this deck's memories into a new draft, and
          keeps this deck as it is. The two then change independently.
        </p>
        {error && <div className="error-banner">{error}</div>}
        <input value={forkName} onChange={(e) => setForkName(e.target.value)} aria-label="Name for the fork" data-modal-autofocus />
        <div className="dialog-actions">
          <button type="button" onClick={onClose}>
            Cancel
          </button>
          <button className="primary" type="submit" disabled={!forkName.trim()}>
            Fork
          </button>
        </div>
      </form>
    </Modal>
  );
}

function PaperList({ deckId }: { deckId: number }) {
  const [status, setStatus] = useState<PaperStatus | null>(null);
  useEffect(() => {
    void api.paperStatus(deckId).then(setStatus);
  }, [deckId]);
  if (!status) return <p className="muted">Loading…</p>;
  return (
    <>
      <p className="muted">
        {status.in_paper_cards} of {status.total_cards} cards are marked as physically in this deck.
      </p>
      <h3 className="interop-head">Pull list · {status.pull.length}</h3>
      <p className="muted rationale">Missing here, and physically in another of your decks.</p>
      {status.pull.map((c) => (
        <div className="log-row" key={c.name}>
          <span className="name">
            {c.quantity > 1 && `${c.quantity}× `}
            {c.name}
          </span>
          <span className="reason">
            in {c.held_by.map((h) => h.deck_name).join(", ")} — the deck you take it from will then be missing it
          </span>
        </div>
      ))}
      {!status.pull.length && <p className="muted rationale">Nothing to pull.</p>}
      <h3 className="interop-head">Buy list · {status.buy.length}</h3>
      <p className="muted rationale">Missing here and in no other deck's paper.</p>
      {status.buy.map((c) => (
        <div className="log-row" key={c.name}>
          <span className="name">
            {c.quantity > 1 && `${c.quantity}× `}
            {c.name}
          </span>
        </div>
      ))}
      {!status.buy.length && <p className="muted rationale">Nothing to buy.</p>}
    </>
  );
}

// Compare this deck with any other, and record which deck it was forked
// from — a deck copied by hand has no such link until it is set here.
function Compare() {
  const { deckId, state, run } = useDeck();
  const parent = state!.deck.parent;
  const [decks, setDecks] = useState<DeckSummary[]>([]);
  const [other, setOther] = useState<number | null>(parent?.id ?? null);
  const [diff, setDiff] = useState<DeckComparison | null>(null);
  const [error, setError] = useState<string | null>(null);
  const peekProps = usePeekProps();

  useEffect(() => {
    void api.listDecks().then((all) => setDecks(all.filter((d) => d.id !== deckId)));
  }, [deckId]);
  useEffect(() => {
    setDiff(null);
    setError(null);
    if (other == null) return;
    api.compareDecks(deckId, other).then(setDiff).catch((e) => setError(e.message));
  }, [deckId, other, state!.deck.revision]);

  const list = (cards: DeckComparison["only_in_a"]) =>
    cards.length ? (
      cards.map((c) => (
        <div className="log-row" key={c.oracle_id}>
          <span className="name peekable" {...peekProps(c.name)}>
            {c.quantity > 1 && `${c.quantity}× `}
            {c.name}
          </span>
          <span className="reason">{c.type_line}</span>
        </div>
      ))
    ) : (
      <p className="muted rationale">None.</p>
    );

  return (
    <>
      <div className="row gap wrap">
        <label className="field">
          Compare with
          <select value={other ?? ""} onChange={(e) => setOther(e.target.value === "" ? null : Number(e.target.value))}>
            <option value="">choose a deck…</option>
            {decks.map((d) => (
              <option key={d.id} value={d.id}>
                {d.name}
                {d.id === parent?.id ? " (parent)" : ""}
              </option>
            ))}
          </select>
        </label>
        <span className="spacer" />
        {other != null && other !== parent?.id && (
          <button
            className="small"
            onClick={() => run(() => api.setDeckParent(deckId, other))}
            title="Record that this deck was forked from the one selected"
          >
            Mark this deck as a fork of it
          </button>
        )}
        {parent && (
          <button className="small" onClick={() => run(() => api.setDeckParent(deckId, null))}>
            Unlink from {parent.name}
          </button>
        )}
      </div>
      {error && <div className="error-banner">{error}</div>}
      {diff && (
        <>
          <p className="muted">{diff.shared_count} cards are in both. Maybe lists are not compared.</p>
          <h3 className="interop-head">
            Only in {diff.a.name} · {diff.only_in_a.length}
          </h3>
          {list(diff.only_in_a)}
          <h3 className="interop-head">
            Only in {diff.b.name} · {diff.only_in_b.length}
          </h3>
          {list(diff.only_in_b)}
        </>
      )}
    </>
  );
}
