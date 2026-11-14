import { api } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { useSession } from "../../app/Session.tsx";
import { href } from "../../app/routes.ts";
import { DeckThumbnail } from "../../mtg/index.ts";
import { ButtonLink, EmptyState, ErrorNote, Loading, Page, PageHeader, Text } from "../../ui/index.ts";
import styles from "./HomePage.module.css";

/** Product home uses the existing principal's library. This is not a second
 * account or a fictional game/activity feed. */
export function HomePage() {
  const decks = useQuery(() => api.listDecks(), [], "decks");
  const { session } = useSession();
  return <Page header={<PageHeader title="Home" meta={session.user ? `@${session.user.handle}` : "Local account"} />}>
    <section className={styles.play} aria-labelledby="home-play-title">
      <div><Text as="h2" size="xl" weight="bold" id="home-play-title">Play Commander</Text>
        <p>One seat for you. Three automated opponents.</p>
        <Text as="p" tone="muted">Choose a deck and explore the table while automated games are being connected.</Text></div>
      <ButtonLink variant="primary" href={href({ page: "play" })}>Start a game</ButtonLink>
    </section>
    <section className={styles.library} aria-labelledby="home-decks-title">
      <div className={styles.heading}><h2 id="home-decks-title">Your decks</h2><a href={href({ page: "decks" })}>All decks</a></div>
      {decks.error ? <ErrorNote>{decks.error}</ErrorNote> : decks.loading ? <Loading /> : decks.data && (decks.data.length === 0
        ? <EmptyState title="No decks yet">Create or import a deck in <a href={href({ page: "decks" })}>My decks</a>.</EmptyState>
        : <ul className={styles.decks}>{decks.data.slice(0, 6).map(deck => <li key={deck.id}>
          <DeckThumbnail cards={deck.commander_art} className={styles.art} />
          <div className={styles.deck}><a className={styles.name} href={href({ page: "deck", deckId: deck.id })}>{deck.name}</a>
            <p>{deck.commanders.length ? deck.commanders.join(" + ") : "No commander selected"}</p>
            <Text tone="muted" mono>{deck.card_count} cards · Revision {deck.revision}</Text></div>
          <div className={styles.actions}><ButtonLink href={href({ page: "deck", deckId: deck.id })} aria-label={`Edit ${deck.name}`}>Edit</ButtonLink>
            <ButtonLink href={href({ page: "play", deckId: deck.id })} aria-label={`Play with ${deck.name}`}>Play</ButtonLink></div>
        </li>)}</ul>)}
    </section>
  </Page>;
}
