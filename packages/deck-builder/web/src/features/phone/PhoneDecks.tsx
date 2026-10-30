import { useInbox } from "../../app/Inbox.tsx";
import { deckWaitingState } from "../../domain/inbox.ts";
import { Fragment, useState } from "react";
import { DeckFilters } from "../decks/index.ts";
import { useSession } from "../../app/Session.tsx";
import { useVoice } from "../../app/Preferences.tsx";
// The deck list on a phone: one tappable row per deck, with what is waiting.

import { api, type DeckSummary } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { href, navigate, useScrollRestore } from "../../app/routes.ts";
import { familyOrder } from "../../domain/familyOrder.ts";
import { CardRef, DeckArt, DeckCard } from "../../mtg/index.ts";
import { Chip, EmptyState, ErrorNote, Icon, Loading, Text } from "../../ui/index.ts";
import styles from "./PhoneDecks.module.css";

function PhoneDeckRow({ deck, fork }: { deck: DeckSummary; fork: boolean }) {
  const { deckWaiting } = useInbox();
  const { count: waiting, pending } = deckWaitingState(deck, deckWaiting);
  const route = { page: "deck" as const, deckId: deck.id, review: pending && deck.category !== "archived" };
  return <DeckCard name={deck.name} href={href(route)} art={deck.commander_art}
    identity={deck.color_identity} category={deck.category} count={deck.card_count}
    commanders={deck.commanders.length ? deck.commanders.map((name, i) => <Fragment key={name}>
      {i > 0 && " + "}<CardRef name={name} onOpen={() => navigate(route)} />
    </Fragment>) : <Text tone="faint">no commander</Text>}
    lineage={fork && deck.parent_name ? `Fork of ${deck.parent_name}` : undefined}>
    {deck.visibility !== "private" && <span className={styles.shared} role="img" aria-label={deck.visibility === "public" ? "Public" : "Unlisted"}><Icon name={deck.visibility === "public" ? "globe" : "link"} size={12} /></span>}
    {pending && <Chip data-deck-waiting={deck.category !== "archived" || undefined} tone={deck.category === "archived" ? "neutral" : "agent"}>{waiting !== null && `${waiting} `}waiting</Chip>}
  </DeckCard>;
}

export function PhoneDecks() {
  const [search, setSearch] = useState("");
  const [category, setCategory] = useState("all");
  const voice = useVoice();
  const { session } = useSession();
  const decks = useQuery(() => api.listDecks(), [], "decks");
  useScrollRestore(decks.data !== null);
  if (decks.error && !decks.data) return <ErrorNote>{decks.error}</ErrorNote>;
  if (!decks.data) return <Loading />;
  if (!decks.data.length)
    return <div className={styles.page}><h1 className={styles.title}>My decks</h1><DeckArt cards={session.backdrop ? [session.backdrop] : null} variant="backdrop" priority="auto"><div className={styles.empty}><h2>No decks yet</h2><Text as="p" tone="muted" size="sm">{voice("Create or import your first deck on a larger screen, then read it here.", "Ask your agent to build one, or create one on a larger screen.")}</Text></div></DeckArt></div>;
  const query = search.trim().toLocaleLowerCase();
  const rows = familyOrder(decks.data);
  const shown = rows.filter(({ deck }) => (category === "all" || category === deck.category)
    && (!query || [deck.name, ...deck.commanders].some(name => name.toLocaleLowerCase().includes(query))));
  return (
    <div className={styles.page}>
      <h1 className={styles.title}>My decks</h1>
      <DeckFilters search={search} category={category} shown={shown.length} total={rows.length}
        onSearch={setSearch} onCategory={setCategory} />
      {shown.length ? <ul className={styles.list}>
        {shown.map(({ deck, fork }) => <PhoneDeckRow key={deck.id} deck={deck} fork={fork} />)}
      </ul> : <EmptyState title="No decks match">Try another name or lifecycle.</EmptyState>}
    </div>
  );
}
