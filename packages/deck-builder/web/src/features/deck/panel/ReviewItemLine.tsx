// One card inside a review unit: what is proposed, the agent's reason, and
// what the deck already knows about the card (spec §7.2, §8.3) so the decision
// is made with the same context an agent has.

import type { ProposalItem } from "../../../api/client.ts";
import { InlineText, ManaCost, CardRef } from "../../../mtg/index.ts";
import { Chip } from "../../../ui/index.ts";
import { cx } from "../../../ui/internal.ts";
import { useDeckState } from "../DeckStore.tsx";
import { ItemContext } from "./ItemContext.tsx";
import styles from "./ReviewItemLine.module.css";

export function ReviewItemLine({ item }: { item: ProposalItem }) {
  const state = useDeckState();
  const adding = item.action === "add";

  // An add names the slot the agent chose; a cut leaves the slot it is in now.
  const owned = state.cards.find((c) => c.oracle_id === item.oracle_id);
  const currentSlot = owned?.slot_id != null ? state.slots.find((s) => s.id === owned.slot_id)?.name : undefined;
  const slot = (adding ? item.slot_name : currentSlot) ?? "Unfiled";

  return (
    <div className={styles.item}>
      <div className={cx(styles.line, adding ? styles.add : styles.cut)}>
        <span className={styles.sign} aria-hidden="true">
          {adding ? "+" : "−"}
        </span>
        <span className={styles.name}>
          <span className={styles.srOnly}>{adding ? "Add " : "Cut "}</span>
          <CardRef name={item.card_name} card={{ ...item, name: item.card_name }} />
        </span>
        <ManaCost cost={item.mana_cost} />
        <span className={styles.slot}>
          {!!item.to_maybe && <Chip title="If accepted, the card moves to the maybe list instead of leaving the deck.">to maybe list</Chip>}
          <Chip>{slot}</Chip>
        </span>
      </div>
      {item.reason && (
        <p className={styles.why}>
          <InlineText text={item.reason} />
        </p>
      )}
      <ItemContext item={item} />
    </div>
  );
}
