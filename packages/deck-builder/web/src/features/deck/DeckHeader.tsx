// The deck's identity and its vital signs (spec §5.1): name, who may edit it,
// and three numbers that each open the tab explaining them. Slot counts are
// not here; they live on the group headings, where they can be acted on.

import { useState, type FormEvent, type KeyboardEvent } from "react";
import { api, type DeckCategory } from "../../api/client.ts";
import { href } from "../../app/routes.ts";
import { DECK_CATEGORY_LABELS, DECK_CATEGORY_OPTIONS } from "../../domain/deckCategory.ts";
import { bracketLabel } from "../../domain/power.ts";
import { formatPrice } from "../../domain/price.ts";
import { ColorIdentity, ManaCost, CardRef, DeckArt } from "../../mtg/index.ts";
import { Button, ConfirmDialog, Explainer, Icon, Menu, Row, Text, TextField, type MenuItem } from "../../ui/index.ts";
import { DeckStat } from "./DeckStat.tsx";
import { useDeck, useDeckState } from "./DeckStore.tsx";
import { useDeckUi } from "./DeckUi.tsx";
import styles from "./DeckHeader.module.css";

export function DeckHeader({ compact = false }: { compact?: boolean }) {
  const state = useDeckState();
  const { run } = useDeck();
  const ui = useDeckUi();
  const { deck, computed, checks, cards } = state;
  const readOnly = deck.category === "archived";
  const [draft, setDraft] = useState<string | null>(null);
  const [confirmArchive, setConfirmArchive] = useState(false);
  const [lifecyclePill, setLifecyclePill] = useState<HTMLButtonElement | null>(null);
  const [explaining, setExplaining] = useState(false);

  function startRename() {
    setDraft(deck.name);
  }

  async function saveName(e: FormEvent) {
    e.preventDefault();
    const name = draft?.trim();
    if (!name || name === deck.name) return setDraft(null);
    // Keep the field open on failure: the error banner says why.
    if (await run(() => api.renameDeck(deck.id, name))) setDraft(null);
  }

  function onNameKey(e: KeyboardEvent) {
    if (e.key !== "Escape") return;
    // The deck screen's own Escape would also clear the selection.
    e.stopPropagation();
    setDraft(null);
  }

  const count = computed.card_count;
  const hasError = checks.some((c) => c.severity === "error");
  const commanders = cards.filter((c) => c.role === "commander" && !c.maybe);

  function setCategory(category: DeckCategory) {
    void run(() => api.setDeckCategory(deck.id, category));
  }

  // Lifecycle is a state shown as a pill; its menu changes it. Archiving makes
  // the deck read-only, so that one choice asks first. What each lifecycle
  // allows is the menu's last item, set apart from the choices.
  const lifecycleItems: MenuItem[] = [
    ...DECK_CATEGORY_OPTIONS.map((option): MenuItem => ({
      id: option.value,
      label: option.label,
      checked: option.value === deck.category,
      onSelect: () => {
        if (option.value === deck.category) return;
        if (option.value === "archived") setConfirmArchive(true);
        else setCategory(option.value);
      },
    })),
    { id: "who-can-edit", label: "Who can edit?", separated: true, onSelect: () => setExplaining(true) },
  ];

  const moreItems: MenuItem[] = [
    { id: "fork", label: "Fork", onSelect: () => ui.openDialog("fork") },
    { id: "compare", label: "Compare", onSelect: () => ui.openDialog("compare") },
    ...(!readOnly ? [{ id: "import", label: "Import", onSelect: () => ui.openDialog("import") }] : []),
    { id: "export", label: "Export", onSelect: () => ui.openDialog("export") },
    { id: "slots", label: "Edit slots and tags", disabled: readOnly, onSelect: () => ui.openDialog("slots") },
    { id: "delete", label: "Delete deck", disabled: readOnly, tone: "cut", onSelect: () => ui.openDialog("delete") },
  ];

  const content = (
    <header className={`${styles.header} ${compact ? styles.compact : ""} ${readOnly ? styles.archived : ""}`}>
      {readOnly && !compact && <p className={styles.archiveNotice}>Archived decks are read-only. To make changes, switch this deck to Paper or Theorycrafted, or fork it.</p>}
      <div className={styles.top}>
        {draft == null ? (
          <h1 className={styles.title}>
            {readOnly ? <span className={styles.name}>{deck.name}</span> : <button type="button" className={styles.name} onClick={startRename}>{deck.name}</button>}
          </h1>
        ) : (
          <form className={styles.rename} onSubmit={saveName}>
            <TextField
              fieldSize="lg"
              aria-label="Deck name"
              value={draft}
              autoFocus
              onFocus={(e) => e.currentTarget.select()}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={onNameKey}
              onBlur={() => setDraft(null)}
            />
          </form>
        )}

        <div className={styles.lifecycle}>
          <Menu
            pill
            label={DECK_CATEGORY_LABELS[deck.category]}
            ariaLabel={`Lifecycle: ${DECK_CATEGORY_LABELS[deck.category]}`}
            items={lifecycleItems}
            triggerRef={setLifecyclePill}
          />
          <Explainer title="Deck lifecycle" control={{ anchor: lifecyclePill, open: explaining, onClose: () => setExplaining(false) }}>
            <p>Paper requires agent card proposals. Theorycrafted lets agents edit cards directly. Archived is read-only.</p>
            <p>Only you can change lifecycle. Agent edits to the brief and power target always need approval.</p>
          </Explainer>
        </div>

      </div>

      <Row className={styles.sub} gap={5} wrap>
        {commanders.length > 0 ? (
          <>
            {commanders.map((c) => (
              <span key={c.oracle_id} className={styles.commander}>
                <CardRef name={c.name} card={c} />
                <ManaCost cost={c.mana_cost} />
              </span>
            ))}
            <ColorIdentity colors={deck.color_identity} />
          </>
        ) : (
          <Text tone="warn">No commander yet</Text>
        )}
        {deck.parent && (
          <span>
            fork of <a className={styles.link} href={href({ page: "deck", deckId: deck.parent.id })}>{deck.parent.name}</a>
          </span>
        )}
        {deck.upstream && (
          <span>
            forked from <a className={styles.link} href={href({ page: "shared", slug: deck.upstream.slug })}>{deck.upstream.owner_handle}/{deck.upstream.name}</a>
          </span>
        )}
      </Row>
      <Row className={styles.metadata} gap={5} wrap>
        {computed.cost.as_of && (
          <Text mono tone="soft">
            est. {formatPrice(computed.cost.cents, computed.cost.currency)} · {computed.cost.store === "tcgplayer" ? "TCGplayer" : "Cardmarket"} · {computed.cost.as_of.slice(0, 10)}
          </Text>
        )}
        <button type="button" className={styles.powerLink} onClick={() => ui.setTab("power")}>
          Power: {state.power.assessment ? `${bracketLabel(state.power.assessment.mapping)}${state.power.assessment.stale ? " · needs review" : ""}` : "not assessed"}
          {state.power.target ? ` · target ${bracketLabel(state.power.target.mapping)}` : ""}
        </button>
      </Row>
      <div className={styles.stats}>
        {!compact && <DeckStat
          figure={count}
          tone={count === 100 ? "add" : "warn"}
          title="Open stats"
          onClick={() => ui.setTab("stats")}
        >
          / 100 cards
        </DeckStat>}
        {!compact && <DeckStat
          figure={checks.length || <Icon name="check" size={12} />}
          tone={hasError ? "cut" : checks.length ? "warn" : "add"}
          title="Open the checks"
          onClick={() => ui.setTab("checks")}
        >
          {/* The count is in the figure; the words say how bad. */}
          {hasError ? (checks.length === 1 ? "problem" : "problems") : checks.length ? "off target" : "Checks clear"}
        </DeckStat>}
        {/* Peers, so one secondary style, set apart from the status chips. */}
        <div className={styles.actions}>
          {readOnly && <Button onClick={() => ui.openDialog("fork")} aria-haspopup="dialog">Fork</Button>}
          {/* A link, so it opens in a new tab too; drawn as Button's default. */}
          <a className={styles.action} href={href({ page: "playtest", deckId: deck.id })}>Playtest</a>
          {/* One word wide in every state, so publishing never wraps the
              header; the icon shows the state and the name says it. */}
          <Button onClick={() => ui.openDialog("share")} aria-haspopup="dialog"
            aria-label={deck.visibility === "private" ? "Share" : `Share · ${deck.visibility === "public" ? "Public" : "Unlisted"}`}>
            {deck.visibility !== "private" && <Icon name={deck.visibility === "public" ? "globe" : "link"} />}
            Share
          </Button>
          <Menu label="More" align="end" items={moreItems} />
        </div>
      </div>
      {confirmArchive && (
        <ConfirmDialog
          title="Archive this deck?"
          confirmLabel="Archive deck"
          onClose={() => setConfirmArchive(false)}
          onConfirm={() => {
            setConfirmArchive(false);
            setCategory("archived");
          }}
        >
          <Text>It becomes read-only for you and your agents until you change its lifecycle again.</Text>
        </ConfirmDialog>
      )}
    </header>
  );
  return compact ? content : <DeckArt cards={commanders} identity={deck.color_identity} variant="editor" priority="auto">{content}</DeckArt>;
}
