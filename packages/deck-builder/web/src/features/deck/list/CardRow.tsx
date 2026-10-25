// One card in the list: a single focusable line (spec §5). Click opens the
// card in the side panel; Ctrl, Cmd or Shift click selects instead, so a
// bulk action never needs a mode.

import type { KeyboardEvent, MouseEvent } from "react";
import type { CardMark } from "../../../domain/marks.ts";
import { ManaCost, Price, useCardHover, type CardInfo } from "../../../mtg/index.ts";
import { useDeckList } from "../DeckStore.tsx";
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
  /** Undefined without filters; false keeps the row visible with muted styling. */
  filterMatch?: boolean;
  /** Purpose metadata, distinct from sourcing/status tags. */
  purpose?: string;
}

export function CardRow({ card, quantity = 1, change, mark, bold, current, filterMatch, purpose }: CardRowProps) {
  const ui = useDeckUi();
  const { readOnly } = useDeckList();
  const hover = useCardHover();
  const selected = ui.selected.includes(card.oracle_id);
  const open = ui.openCard?.oracleId === card.oracle_id;
  // Proposed adds are not in the deck, and a read-only list has no selection.
  const selectable = change !== "add" && !readOnly;

  function onClick(e: MouseEvent) {
    if (selectable && (e.ctrlKey || e.metaKey || e.shiftKey)) ui.toggleSelected(card.oracle_id);
    else ui.openCardPanel(open ? null : { oracleId: card.oracle_id, name: card.name });
  }

  // Enter on a focused row opens the card; it must not also reach the screen's
  // own Enter shortcut, which starts a review.
  function onKeyDown(e: KeyboardEvent) {
    if (e.key === "Enter") e.stopPropagation();
  }

  const sign = change === "cut" ? "−" : change === "add" ? "+" : quantity > 1 ? quantity : "";
  const cls = [
    styles.row,
    filterMatch === true && styles.match,
    filterMatch === false && styles.muted,
    change === "cut" && styles.cut,
    change === "add" && styles.add,
    selected && styles.selected,
    open && styles.open,
    current && styles.current,
    bold && styles.bold,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <button
      type="button"
      className={cls}
      data-current={current || undefined}
      data-filter-match={filterMatch}
      aria-current={open || undefined}
      onClick={onClick}
      onKeyDown={onKeyDown}
      {...hover(card)}
    >
      <span className={styles.q}>{sign}</span>
      <span className={styles.name}>
        <span>{card.name}</span>
        {purpose && <span className={styles.purpose}>{purpose}</span>}
      </span>
      {(mark?.memories || mark?.rulings) && (
        <span className={styles.marks}>
          {!!mark?.memories && <Mark kind="memory" />}
          {!!mark?.rulings && <Mark kind="ruling" />}
        </span>
      )}
      <span className={styles.cost}>
        <ManaCost cost={card.mana_cost} />
      </span>
      <Price currency={card.currency} cents={card.cents} column />
    </button>
  );
}
