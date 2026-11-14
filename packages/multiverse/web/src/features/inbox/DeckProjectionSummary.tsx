import type { DeckState } from "../../api/client.ts";
import { formatPrice } from "../../domain/price.ts";
import { Text } from "../../ui/index.ts";
import styles from "./DeckProjectionSummary.module.css";

/** The same deck-wide projection, never the selected proposal alone.
 * Unknown prices stay visible beside an incomplete estimate. */
export function DeckProjectionSummary({ state }: { state: DeckState }) {
  const { computed } = state;
  const { pending } = computed;
  return (
    <span className={styles.summary}>
      <span>Cards <Text mono tone="soft">{computed.card_count} → {pending.projected_count}</Text></span>
      {computed.cost.as_of && <span>Est. <Text mono tone="soft">{formatPrice(computed.cost.cents, computed.cost.currency)} → {formatPrice(pending.projected_cost.cents, computed.cost.currency)}</Text></span>}
      {(computed.cost.unpriced > 0 || pending.projected_cost.unpriced > 0) && <span>Unpriced <Text mono tone="soft">{computed.cost.unpriced} → {pending.projected_cost.unpriced}</Text></span>}
    </span>
  );
}
