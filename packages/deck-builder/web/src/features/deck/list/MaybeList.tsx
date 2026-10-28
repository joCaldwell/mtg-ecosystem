// The maybe list (spec §5.4): cards considered for the deck but outside the
// 100. Kept apart from the columns so nothing in it can be mistaken for part
// of the count.

import type { DeckCard } from "../../../api/client.ts";
import { maybeNotes } from "../../../domain/maybeNotes.ts";
import { Text } from "../../../ui/index.ts";
import { MaybeRow } from "./MaybeRow.tsx";
import styles from "./MaybeList.module.css";

export function MaybeList({ cards, matchingCards }: {
  cards: DeckCard[];
  matchingCards?: ReadonlySet<string>;
}) {
  if (cards.length === 0) return null;
  const { origins } = maybeNotes(cards);
  return (
    <section className={styles.maybe} aria-label="Maybe list">
      <div className={styles.head}>
        <h2 className={styles.title}>Maybe</h2>
        <Text tone="soft" size="sm">
          {cards.length} {cards.length === 1 ? "card" : "cards"} · outside the 100
        </Text>
      </div>
      {origins.length > 0 && <p className={styles.sharedNote}>{origins.map(origin => `${origin.count} from ${origin.label}`).join(" · ")}</p>}
      <div className={styles.rows}>{cards.map((c) => (
        <MaybeRow key={c.oracle_id} card={c} filterMatch={matchingCards?.has(c.oracle_id)} importNote={origins.some(origin => origin.note === c.maybe_note)} />
      ))}</div>
    </section>
  );
}
