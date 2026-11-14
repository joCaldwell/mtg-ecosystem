import type { KeyboardEvent } from "react";
import { tokenSummary, type DeckToken } from "../domain/tokens.ts";
import { useCardHover, useCardInspect } from "./CardHover.tsx";
import { ColorIdentity } from "./ColorIdentity.tsx";
import { tokenCardInfo } from "./tokenCard.ts";
import styles from "./TokenRow.module.css";

/**
 * One token or emblem a deck makes, on one dense line: name, "P/T · type"
 * (or "Emblem"), its colours and how many of the deck's cards make it. The
 * whole line is one button; hovering shows the token's own picture, ? on a
 * focused row inspects it, as on a card row.
 */
export function TokenRow({ token, open = false, filterMatch, onOpen }: {
  token: DeckToken;
  /** Its details are showing. */
  open?: boolean;
  /** Drawn because a matching card makes it. */
  filterMatch?: boolean;
  onOpen: () => void;
}) {
  const hover = useCardHover();
  const inspect = useCardInspect();
  const info = tokenCardInfo(token);

  // Enter opens the token; it must not also reach the screen's Enter shortcut.
  function onKeyDown(e: KeyboardEvent) {
    if (e.key === "?") { e.preventDefault(); e.stopPropagation(); inspect(info); }
    if (e.key === "Enter") e.stopPropagation();
  }

  return (
    <button
      type="button"
      className={open ? `${styles.row} ${styles.open}` : styles.row}
      data-token-id={token.oracle_id}
      data-filter-match={filterMatch || undefined}
      aria-current={open || undefined}
      onClick={onOpen}
      onKeyDown={onKeyDown}
      {...hover(info)}
    >
      <span className={styles.name}>
        {token.name} <span className={styles.summary}>{tokenSummary(token)}</span>
      </span>
      {token.colors && <ColorIdentity colors={token.colors} label="Colour" />}
      <span className={styles.from}>from {token.producer_count}<span className="visually-hidden"> {token.producer_count === 1 ? "card" : "cards"}</span></span>
    </button>
  );
}
