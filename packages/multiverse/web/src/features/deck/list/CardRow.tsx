// Primary activation inspects a card. Right-click, modifier-click and
// Shift+Space toggle bulk selection.

import { useRef, type KeyboardEvent, type MouseEvent } from "react";
import type { CardMark } from "../../../domain/marks.ts";
import { ManaCost, Price, useCardHover, useCardInspect, type CardInfo } from "../../../mtg/index.ts";
import { useDeckList } from "../DeckListStore.tsx";
import { useDeckUi } from "../DeckUi.tsx";
import { Mark } from "./Mark.tsx";
import styles from "./CardRow.module.css";

export interface CardRowProps {
  card: CardInfo & { oracle_id: string };
  /** Shown when more than one copy. Ignored for cut and add rows, which show a sign. */
  quantity?: number;
  /** In review mode a pending cut or add is drawn in place. */
  change?: "cut" | "add";
  mark?: CardMark;
  /** Commanders read as the head of the list. */
  bold?: boolean;
  /** The row belongs to the decision under the review cursor. */
  current?: boolean;
  /** Drawn because it matches the active filter (nonmatches are not drawn). */
  filterMatch?: boolean;
  /** Purpose metadata, distinct from sourcing/status tags. */
  purpose?: string;
}

export function CardRow({ card, quantity = 1, change, mark, bold, current, filterMatch, purpose }: CardRowProps) {
  const ui = useDeckUi();
  const { readOnly, state } = useDeckList();
  const rowRef = useRef<HTMLButtonElement>(null);
  const hover = useCardHover();
  const inspect = useCardInspect();
  const selectable = change !== "add" && !readOnly && state.deck.category !== "archived";
  const selected = selectable && ui.selected.includes(card.oracle_id);
  const open = ui.openCard?.oracleId === card.oracle_id;

  function onClick(e: MouseEvent) {
    if (selectable && (e.ctrlKey || e.metaKey || e.shiftKey)) ui.toggleSelected(card.oracle_id);
    else ui.openCardPanel(open ? null : { oracleId: card.oracle_id, name: card.name });
  }

  // Enter on a focused row opens the card; it must not also reach the screen's
  // own Enter shortcut, which starts a review.
  function onKeyDown(e: KeyboardEvent) {
    if (e.nativeEvent.isComposing || e.nativeEvent.keyCode === 229) return;
    if (e.key === "?") { e.preventDefault(); e.stopPropagation(); inspect(card); }
    if (selectable && ((e.key === " " && e.shiftKey) || (e.key === "Enter" && (e.ctrlKey || e.metaKey || e.shiftKey)))) {
      e.preventDefault();
      e.stopPropagation();
      if (!e.repeat) ui.toggleSelected(card.oracle_id);
      return;
    }
    if (e.key === "Enter") e.stopPropagation();
  }

  const sign = change === "cut" ? "−" : change === "add" ? "+" : quantity > 1 ? quantity : "";
  const cls = [
    styles.row,
    change === "cut" && styles.cut,
    change === "add" && styles.add,
    selected && styles.selected,
    open && styles.open,
    current && styles.current,
    bold && styles.bold,
  ]
    .filter(Boolean)
    .join(" ");

  const row = (
    <button
      ref={rowRef}
      type="button"
      className={cls}
      data-current={current || undefined}
      data-oracle-id={card.oracle_id}
      data-filter-match={filterMatch || undefined}
      aria-current={open || undefined}
      aria-keyshortcuts={selectable ? "Shift+Space" : undefined}
      onClick={onClick}
      onContextMenu={selectable ? (e) => {
        e.preventDefault();
        ui.toggleSelected(card.oracle_id);
      } : undefined}
      onKeyDown={onKeyDown}
      {...hover(card)}
    >
      <span className={styles.q}>{sign}</span>
      <span className={styles.name}>
        <span>{card.name}</span>
        {purpose && <span className={styles.purpose}>{purpose}</span>}
      </span>
      {(mark?.memories || mark?.decisions) && (
        <span className={styles.marks}>
          {!!mark?.memories && <Mark kind="memory" />}
          {!!mark?.decisions && <Mark kind="decision" />}
        </span>
      )}
      <span className={styles.cost} title="Inspect card · ? on a focused row" onClick={(e) => { e.stopPropagation(); inspect(card); }}>
        <ManaCost cost={card.mana_cost} />
      </span>
      <span className={styles.price}><Price currency={card.currency} cents={card.cents} column /></span>
    </button>
  );
  if (!selectable) return row;
  return <div className={styles.item}>
    <label className={styles.selector} data-selection-active={ui.selected.length > 0}>
      <input
        className={styles.selectCheck}
        type="checkbox"
        data-deck-selection
        aria-label={`Select ${card.name}`}
        checked={selected}
        onChange={() => {
          if (selected && ui.selected.length === 1) rowRef.current?.focus({ preventScroll: true });
          ui.toggleSelected(card.oracle_id);
        }}
      />
    </label>
    {row}
  </div>;
}
