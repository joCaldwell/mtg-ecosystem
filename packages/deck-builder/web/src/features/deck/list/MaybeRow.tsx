import type { DeckCard } from "../../../api/client.ts";
import { InlineText, ManaCost, useCardHover } from "../../../mtg/index.ts";
import { Chip } from "../../../ui/index.ts";
import { useDeckUi } from "../DeckUi.tsx";
import styles from "./MaybeRow.module.css";

export function MaybeRow({ card, slotName }: { card: DeckCard; slotName: string | null }) {
  const ui = useDeckUi();
  const hover = useCardHover();
  const open = ui.openCard?.oracleId === card.oracle_id;

  return (
    <div className={`${styles.row} ${open ? styles.open : ""}`}>
      <button
        type="button"
        className={styles.name}
        aria-current={open || undefined}
        onClick={() => ui.openCardPanel(open ? null : { oracleId: card.oracle_id, name: card.name })}
        // Enter opens the card here; it must not also start a review.
        onKeyDown={(e) => e.key === "Enter" && e.stopPropagation()}
        {...hover(card)}
      >
        {card.name}
      </button>
      <span className={styles.note}>{card.maybe_note && <InlineText text={card.maybe_note} />}</span>
      {slotName && <Chip>{slotName}</Chip>}
      <ManaCost cost={card.mana_cost} />
    </div>
  );
}
