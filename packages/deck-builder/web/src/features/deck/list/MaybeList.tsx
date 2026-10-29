// The maybe list (spec §5.4): cards considered for the deck but outside the
// 100. Kept apart from the columns so nothing in it can be mistaken for part
// of the count.

import type { DeckCard } from "../../../api/client.ts";
import type { DeckFilterMatches } from "../../../domain/deckSearch.ts";
import { maybeNotes } from "../../../domain/maybeNotes.ts";
import { Text } from "../../../ui/index.ts";
import { MaybeRow } from "./MaybeRow.tsx";
import styles from "./MaybeList.module.css";

export function MaybeList({ cards, filter }: {
  cards: DeckCard[];
  /** The active filter, if any: only matching cards are drawn, and the list
   *  is left out when none match. */
  filter?: DeckFilterMatches;
}) {
  const filtering = !!filter?.active;
  const shown = filter?.active ? cards.filter((c) => filter.cardIds.has(c.oracle_id)) : cards;
  if (shown.length === 0) return null;
  // Origins describe the whole maybe list; each row keeps its own note.
  const { origins } = maybeNotes(cards);
  return (
    <section className={styles.maybe} aria-label="Maybe list">
      <div className={styles.head}>
        <h2 className={styles.title}>Maybe</h2>
        <Text tone="soft" size="sm">
          {filtering ? `${shown.length} of ${cards.length}` : `${cards.length} ${cards.length === 1 ? "card" : "cards"}`} · outside the 100
        </Text>
      </div>
      {!filtering && origins.length > 0 && <p className={styles.sharedNote}>{origins.map(origin => `${origin.count} from ${origin.label}`).join(" · ")}</p>}
      <div className={styles.rows}>{shown.map((c) => (
        <MaybeRow key={c.oracle_id} card={c} filterMatch={filtering} importNote={origins.some(origin => origin.note === c.maybe_note)} />
      ))}</div>
    </section>
  );
}
