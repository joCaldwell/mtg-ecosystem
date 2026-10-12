// The maybe list (spec §5.4): cards considered for the deck but outside the
// 100. Kept apart from the columns so nothing in it can be mistaken for part
// of the count.

import type { DeckCard, Slot } from "../../../api/client.ts";
import { Text } from "../../../ui/index.ts";
import { MaybeRow } from "./MaybeRow.tsx";
import styles from "./MaybeList.module.css";

export function MaybeList({ cards, slots, matchingCards }: {
  cards: DeckCard[];
  slots: Slot[];
  matchingCards?: ReadonlySet<string>;
}) {
  if (cards.length === 0) return null;
  const slotName = (id: number | null) => slots.find((s) => s.id === id)?.name ?? null;
  return (
    <section className={styles.maybe} aria-label="Maybe list">
      <div className={styles.head}>
        <h3 className={styles.title}>Maybe list</h3>
        <Text tone="faint" size="sm">
          {cards.length} {cards.length === 1 ? "card" : "cards"} · outside the 100 · agents can shortlist here without asking
        </Text>
      </div>
      {cards.map((c) => (
        <MaybeRow key={c.oracle_id} card={c} slotName={slotName(c.slot_id)} filterMatch={matchingCards?.has(c.oracle_id)} />
      ))}
    </section>
  );
}
