import { Explainer } from "../../ui/index.ts";
import { useVoice } from "../../app/Preferences.tsx";
// The deck's identity and its vital signs (spec §5.1): name, who may edit it,
// and three numbers that each open the tab explaining them. Slot counts are
// not here; they live on the group headings, where they can be acted on.

import { useState, type FormEvent, type KeyboardEvent } from "react";
import { api } from "../../api/client.ts";
import { href } from "../../app/routes.ts";
import { DECK_CATEGORY_OPTIONS } from "../../domain/deckCategory.ts";
import { bracketLabel } from "../../domain/power.ts";
import { costCaption, formatPrice } from "../../domain/price.ts";
import { ColorIdentity, ManaCost, CardRef, DeckArt } from "../../mtg/index.ts";
import { Button, Icon, Menu, Row, Segmented, Text, TextField, type MenuItem } from "../../ui/index.ts";
import { DeckStat } from "./DeckStat.tsx";
import { useDeck, useDeckState } from "./DeckStore.tsx";
import { useDeckUi } from "./DeckUi.tsx";
import styles from "./DeckHeader.module.css";

export function DeckHeader({ collapsed = false }: { collapsed?: boolean }) {
  const voice = useVoice();
  const state = useDeckState();
  const { run } = useDeck();
  const ui = useDeckUi();
  const { deck, computed, checks, cards } = state;
  const readOnly = deck.category === "archived";
  const [draft, setDraft] = useState<string | null>(null);

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

  const moreItems: MenuItem[] = [
    { id: "fork", label: "Fork", onSelect: () => ui.openDialog("fork") },
    { id: "compare", label: "Compare", onSelect: () => ui.openDialog("compare") },
    ...(!readOnly ? [{ id: "import", label: "Import", onSelect: () => ui.openDialog("import") }] : []),
    { id: "export", label: "Export", onSelect: () => ui.openDialog("export") },
    { id: "slots", label: "Edit slots and tags", disabled: readOnly, onSelect: () => ui.openDialog("slots") },
    { id: "delete", label: "Delete deck", disabled: readOnly, tone: "cut", onSelect: () => ui.openDialog("delete") },
  ];

  return (
    <DeckArt cards={commanders} identity={deck.color_identity} variant="banner" collapsed={collapsed} priority="auto"><header className={styles.header}>
      <div className={styles.top}>
        {draft == null ? (
          <h1 className={styles.title}>
            {readOnly ? <span className={styles.name}>{deck.name}</span> : <button type="button" className={styles.name} onClick={startRename} title="Click to rename">{deck.name}</button>}
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

        <Segmented
          label="Deck lifecycle"
          options={DECK_CATEGORY_OPTIONS}
          value={deck.category}
          onChange={(category) => {
            if (category !== deck.category) void run(() => api.setDeckCategory(deck.id, category));
          }}
        />

        <div className={styles.stats}>
          <DeckStat
            figure={count}
            tone={count === 100 ? "add" : "cut"}
            title="Open the checks"
            onClick={() => ui.setTab("checks")}
          >
            / 100 cards
          </DeckStat>
          <DeckStat
            figure={checks.length || <Icon name="check" size={12} />}
            tone={hasError ? "cut" : checks.length ? "warn" : "add"}
            title="Open the checks"
            onClick={() => ui.setTab("checks")}
          >
            {/* The count is in the figure; the words say how bad. */}
            {hasError ? (checks.length === 1 ? "problem" : "problems") : checks.length ? "off target" : "Checks clear"}
          </DeckStat>
          {/* Peers, so one secondary style, set apart from the status chips. */}
          <div className={styles.actions}>
            {/* A link, so it opens in a new tab too; drawn as Button's default. */}
            <a className={styles.action} href={href({ page: "playtest", deckId: deck.id })}>Playtest</a>
            <Button onClick={() => ui.openDialog("share")} aria-haspopup="dialog"
              title={deck.visibility === "private" ? "Private: only you can see this deck" : deck.visibility === "public" ? "Public: anyone can read it" : "Unlisted: anyone with the link can read it"}>
              {deck.visibility !== "private" && <Icon name={deck.visibility === "public" ? "globe" : "link"} />}
              Share{deck.visibility !== "private" ? ` · ${deck.visibility === "public" ? "Public" : "Unlisted"}` : ""}
            </Button>
            <Menu label="More" align="end" items={moreItems} />
          </div>
        </div>
      </div>

      <Explainer title="Deck lifecycle" summary={voice("Theorycrafted is a work in progress; Paper is a deck you play; Archived keeps it read-only.", "Lifecycle controls how your agent can change the deck.")}>Only you change lifecycle. Agents edit Theorycrafted cards directly and propose Paper card changes. Brief and power-target changes always need your approval. Archived decks can be shared, exported or forked.</Explainer>
      <Row className={styles.sub} gap={5} wrap>
        {/* An archived deck says what it means once, in the banner under the toolbar. */}
        {!readOnly && <Text size="xs" tone="muted">{deck.category === "paper" ? voice("A deck you play", "Agents propose card and brief changes") : voice("A work in progress", "Agents edit cards directly · brief changes need approval")}</Text>}
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
        {computed.cost.as_of && (
          <Text mono tone="muted" title={costCaption(computed.cost)}>
            est. {formatPrice(computed.cost.cents, computed.cost.currency)} · {computed.cost.store === "tcgplayer" ? "TCGplayer" : "Cardmarket"} · {computed.cost.as_of.slice(0, 10)}
          </Text>
        )}
        <button type="button" className={styles.name} onClick={() => ui.setTab("power")} title="Set the power target or assess this deck">
          Power: {state.power.assessment ? `${bracketLabel(state.power.assessment.mapping)}${state.power.assessment.stale ? " · needs review" : ""}` : "not assessed"}
          {state.power.target ? ` · target ${bracketLabel(state.power.target.mapping)}` : ""}
        </button>
      </Row>
    </header></DeckArt>
  );
}
