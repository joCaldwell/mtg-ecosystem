// The deck's identity and its vital signs (spec §5.1): name, who may edit it,
// and three numbers that each open the tab explaining them. Slot counts are
// not here; they live on the group headings, where they can be acted on.

import { useState, type FormEvent, type KeyboardEvent } from "react";
import { api } from "../../api/client.ts";
import { href } from "../../app/routes.ts";
import { DECK_CATEGORY_OPTIONS } from "../../domain/deckCategory.ts";
import { bracketLabel } from "../../domain/power.ts";
import { costCaption, formatUsd } from "../../domain/price.ts";
import { ColorIdentity, ManaCost, CardRef } from "../../mtg/index.ts";
import { Icon, Menu, Row, Segmented, Text, TextField, type MenuItem } from "../../ui/index.ts";
import { DeckStat } from "./DeckStat.tsx";
import { useDeck, useDeckState } from "./DeckStore.tsx";
import { useDeckUi } from "./DeckUi.tsx";
import styles from "./DeckHeader.module.css";

const STATUS_OPTIONS = [
  {
    value: "draft" as const,
    label: "Draft",
    title: "Draft: agents edit the list directly, and every edit lands in History. Only you can change this.",
  },
  {
    value: "active" as const,
    label: "Active",
    title: "Active: agents propose changes and you rule on them. Only you can change this.",
  },
];

export function DeckHeader() {
  const state = useDeckState();
  const { run } = useDeck();
  const ui = useDeckUi();
  const { deck, computed, checks, cards } = state;
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
  const notInPaper = cards.reduce((n, c) => n + (!c.maybe && !c.in_paper ? c.quantity : 0), 0);
  const commanders = cards.filter((c) => c.role === "commander" && !c.maybe);

  const moreItems: MenuItem[] = [
    { id: "fork", label: "Fork", onSelect: () => ui.openDialog("fork") },
    { id: "compare", label: "Compare", onSelect: () => ui.openDialog("compare") },
    { id: "interop", label: "Import / export", onSelect: () => ui.openDialog("interop") },
    { id: "slots", label: "Edit slots and tags", onSelect: () => ui.openDialog("slots") },
    { id: "delete", label: "Delete deck", tone: "cut", onSelect: () => ui.openDialog("delete") },
  ];

  return (
    <header className={styles.header}>
      <div className={styles.top}>
        {draft == null ? (
          <h1 className={styles.title}>
            <button type="button" className={styles.name} onClick={startRename} title="Click to rename">
              {deck.name}
            </button>
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

        <div className={styles.status} data-tone={deck.status === "draft" ? "warn" : "add"}>
          <Segmented
            label="Deck status"
            options={STATUS_OPTIONS}
            value={deck.status}
            onChange={(status) => {
              if (status !== deck.status) void run(() => api.setDeckStatus(deck.id, status));
            }}
          />
        </div>

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
          <DeckStat figure={notInPaper} tone="neutral" title="Open the paper list" onClick={() => ui.setTab("paper")}>
            not in paper
          </DeckStat>
          <Menu label="More" align="end" items={moreItems} />
        </div>
      </div>

      <Row className={styles.sub} gap={5} wrap>
        <Row gap={2}>
          <Text size="xs" tone="faint">Category</Text>
          <Segmented
            label="Deck category"
            options={DECK_CATEGORY_OPTIONS}
            value={deck.category}
            onChange={(category) => {
              if (category !== deck.category) void run(() => api.setDeckCategory(deck.id, category));
            }}
          />
        </Row>
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
        {computed.cost.as_of && (
          <Text mono tone="muted" title={costCaption(computed.cost)}>
            est. {formatUsd(computed.cost.usd_cents)}
          </Text>
        )}
        <button type="button" className={styles.name} onClick={() => ui.setTab("power")} title="Set the power target or assess this deck">
          Power: {state.power.assessment ? `${bracketLabel(state.power.assessment.mapping)}${state.power.assessment.stale ? " · needs review" : ""}` : "not assessed"}
          {state.power.target ? ` · target ${bracketLabel(state.power.target.mapping)}` : ""}
        </button>
        <Text mono tone="faint" title="The deck's revision">
          r{deck.revision}
        </Text>
      </Row>
    </header>
  );
}
