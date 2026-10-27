// The maybe list (spec §5.4): cards considered for the deck but outside the
// 100. Kept apart from the columns so nothing in it can be mistaken for part
// of the count.

import type { DeckCard, Slot } from "../../../api/client.ts";
import { InlineText } from "../../../mtg/index.ts";
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
  // A shared import-origin note belongs to the list. Distinct card notes
  // remain beside their card, and nothing stored is shortened or rewritten.
  const sharedNote = cards.length > 1 && cards[0].maybe_note && cards.every(c => c.maybe_note === cards[0].maybe_note)
    ? cards[0].maybe_note : null;
  return (
    <section className={styles.maybe} aria-label="Maybe list">
      <div className={styles.head}>
        <h2 className={styles.title}>Maybe</h2>
        <Text tone="soft" size="sm">
          {cards.length} {cards.length === 1 ? "card" : "cards"} · outside the 100
        </Text>
      </div>
      {sharedNote && <p className={styles.sharedNote}><InlineText text={sharedNote} /></p>}
      {cards.map((c) => (
        <MaybeRow key={c.oracle_id} card={c} slotName={slotName(c.slot_id)} filterMatch={matchingCards?.has(c.oracle_id)} sharedNote={sharedNote != null} />
      ))}
    </section>
  );
}
