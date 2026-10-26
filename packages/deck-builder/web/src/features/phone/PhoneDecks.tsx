// The deck list on a phone: one tappable row per deck, with what is waiting.

import { api } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { href, useScrollRestore } from "../../app/routes.ts";
import { DECK_CATEGORY_LABELS } from "../../domain/deckCategory.ts";
import { familyOrder } from "../../domain/familyOrder.ts";
import { ColorIdentity } from "../../mtg/index.ts";
import { Chip, EmptyState, ErrorNote, Icon, Loading } from "../../ui/index.ts";
import styles from "./PhoneDecks.module.css";

export function PhoneDecks() {
  const decks = useQuery(() => api.listDecks(), []);
  useScrollRestore(decks.data !== null);
  if (decks.error && !decks.data) return <ErrorNote>{decks.error}</ErrorNote>;
  if (!decks.data) return <Loading />;
  if (!decks.data.length)
    return <div className={styles.page}><EmptyState title="No decks yet">Ask your agent to build one, or create one on a larger screen.</EmptyState></div>;
  return (
    <div className={styles.page}>
      <h1 className={styles.title}>Your decks</h1>
      <ul className={styles.list}>
        {familyOrder(decks.data).map(({ deck, fork }) => {
          const waiting = deck.pending_items + deck.pending_document_edits;
          return (
            <li key={deck.id}>
              <a className={fork ? `${styles.row} ${styles.fork}` : styles.row} href={href({ page: "deck", deckId: deck.id, review: waiting > 0 && deck.category !== "legacy" })}>
                <span className={styles.name}>
                  {deck.name}
                  {deck.visibility !== "private" && <span role="img" aria-label={deck.visibility === "public" ? "Public" : "Unlisted"}><Icon name={deck.visibility === "public" ? "globe" : "link"} /></span>}
                </span>
                <span className={styles.meta}>
                  <ColorIdentity colors={deck.color_identity} />
                  <span>{deck.commanders.join(" + ") || "no commander"}</span>
                </span>
                <span className={styles.chips}>
                  <Chip>{DECK_CATEGORY_LABELS[deck.category]}</Chip>
                  <span className={styles.count}>{deck.card_count}/100</span>
                  {waiting > 0 && <Chip tone={deck.category === "legacy" ? "neutral" : "agent"}>{waiting} waiting</Chip>}
                </span>
              </a>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
