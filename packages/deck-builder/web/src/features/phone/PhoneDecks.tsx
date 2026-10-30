import { useSession } from "../../app/Session.tsx";
import { useVoice } from "../../app/Preferences.tsx";
// The deck list on a phone: one tappable row per deck, with what is waiting.

import { api, type DeckSummary } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { href, useScrollRestore } from "../../app/routes.ts";
import { DECK_CATEGORY_LABELS } from "../../domain/deckCategory.ts";
import { familyOrder } from "../../domain/familyOrder.ts";
import { ColorIdentity, DeckArt } from "../../mtg/index.ts";
import { Chip, ErrorNote, Icon, Loading, Text, useNearView } from "../../ui/index.ts";
import styles from "./PhoneDecks.module.css";

function PhoneDeckRow({ deck, fork }: { deck: DeckSummary; fork: boolean }) {
  const [ref, near] = useNearView<HTMLLIElement>();
  const waiting = deck.pending_items + deck.pending_document_edits;
  return (
    <li ref={ref} className={fork ? styles.fork : undefined}>
      <DeckArt cards={deck.commander_art} identity={deck.color_identity} load={near}><a className={styles.row} href={href({ page: "deck", deckId: deck.id, review: waiting > 0 && deck.category !== "archived" })}>
        <span className={styles.name}>
          {deck.name}
          {deck.visibility !== "private" && <span role="img" aria-label={deck.visibility === "public" ? "Public" : "Unlisted"} title={deck.visibility === "public" ? "Public" : "Unlisted"}><Icon name={deck.visibility === "public" ? "globe" : "link"} /></span>}
        </span>
        <span className={styles.meta}>
          <ColorIdentity colors={deck.color_identity} />
          <span>{deck.commanders.join(" + ") || "no commander"}</span>
        </span>
        <span className={styles.chips}>
          <Chip>{DECK_CATEGORY_LABELS[deck.category]}</Chip>
          <span className={styles.count}>{deck.card_count}/100</span>
          {waiting > 0 && <Chip tone={deck.category === "archived" ? "neutral" : "agent"}>{waiting} waiting</Chip>}
        </span>
      </a></DeckArt>
    </li>
  );
}

export function PhoneDecks() {
  const voice = useVoice();
  const { session } = useSession();
  const decks = useQuery(() => api.listDecks(), []);
  useScrollRestore(decks.data !== null);
  if (decks.error && !decks.data) return <ErrorNote>{decks.error}</ErrorNote>;
  if (!decks.data) return <Loading />;
  if (!decks.data.length)
    return <div className={styles.page}><h1 className={styles.title}>Your decks</h1><DeckArt cards={session.backdrop ? [session.backdrop] : null} variant="backdrop" priority="auto"><div className={styles.empty}><h2>No decks yet</h2><Text as="p" tone="muted" size="sm">{voice("Create or import your first deck on a larger screen, then read it here.", "Ask your agent to build one, or create one on a larger screen.")}</Text></div></DeckArt></div>;
  return (
    <div className={styles.page}>
      <h1 className={styles.title}>Your decks</h1>
      <ul className={styles.list}>
        {familyOrder(decks.data).map(({ deck, fork }) => <PhoneDeckRow key={deck.id} deck={deck} fork={fork} />)}
      </ul>
    </div>
  );
}
