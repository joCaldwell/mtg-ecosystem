import { useId, type KeyboardEvent } from "react";
import type { DeckCard } from "../../../api/client.ts";
import { InlineText, ManaCost, Price, useCardHover, useCardInspect } from "../../../mtg/index.ts";
import { useDeckUi } from "../DeckUi.tsx";
import styles from "./MaybeRow.module.css";

export function MaybeRow({ card, filterMatch, sharedNote = false, importNote = false }: { card: DeckCard; filterMatch?: boolean; sharedNote?: boolean; importNote?: boolean }) {
  const noteId = useId();
  const ui = useDeckUi();
  const hover = useCardHover();
  const inspect = useCardInspect();
  const open = ui.openCard?.oracleId === card.oracle_id;

  // Maybe cards open details without joining bulk actions that assume deck
  // membership. Main-list modifier selection keeps its existing contract.
  function onClick() {
    ui.openCardPanel(open ? null : { oracleId: card.oracle_id, name: card.name });
  }

  function onKeyDown(event: KeyboardEvent) {
    if (event.key === "?") { event.preventDefault(); event.stopPropagation(); inspect(card); }
    if (event.key === "Enter") event.stopPropagation();
  }

  return (
    <div
      className={[styles.row, filterMatch === true && styles.match, open && styles.open].filter(Boolean).join(" ")}
      data-oracle-id={card.oracle_id}
      data-filter-match={filterMatch}
    >
      {/* A native full-row button stays a sibling of note links: no nested
          controls, and mana/price/blank space all open this card's details. */}
      <button
        type="button"
        className={styles.hit}
        aria-label={card.name}
        aria-describedby={importNote ? noteId : undefined}
        aria-current={open || undefined}
        onClick={onClick}
        onKeyDown={onKeyDown}
        {...hover(card)}
      />
      <span className={styles.name}>{card.name}</span>
      <span className={styles.note}>
        {!sharedNote && !importNote && card.maybe_note && <InlineText text={card.maybe_note} />}
      </span>
      <span className={styles.mana}><ManaCost cost={card.mana_cost} /></span>
      <span className={styles.price}><Price currency={card.currency} cents={card.cents} column /></span>
      {importNote && <span id={noteId} className="visually-hidden">{card.maybe_note}</span>}
    </div>
  );
}
