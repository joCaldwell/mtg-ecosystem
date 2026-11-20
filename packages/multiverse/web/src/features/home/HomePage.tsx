import { api } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { useSession } from "../../app/Session.tsx";
import { href } from "../../app/routes.ts";
import { ColorIdentity, DeckArt, DeckThumbnail } from "../../mtg/index.ts";
import { ButtonLink, EmptyState, ErrorNote, Loading, Page, PageHeader } from "../../ui/index.ts";
import styles from "./HomePage.module.css";

export function HomePage() {
  const decks = useQuery(() => api.listDecks(), [], "decks");
  const { session } = useSession();
  return <Page header={<PageHeader title="Home" />}>
    <section className={styles.play} aria-labelledby="home-play-title">
      <DeckArt cards={session.backdrop ? [session.backdrop] : null} variant="feature" priority="high">
        <div className={styles.playContent}>
          <h2 id="home-play-title">Play Commander</h2>
          <div className={styles.actions}>
            <ButtonLink variant="primary" href={href({ page: "play" })}>Start a game</ButtonLink>
            <ButtonLink href={href({ page: "game-preview" })}>Preview table</ButtonLink>
          </div>
        </div>
      </DeckArt>
    </section>
    <section className={styles.library} aria-labelledby="home-decks-title">
      <div className={styles.heading}><h2 id="home-decks-title">Your decks</h2><ButtonLink variant="ghost" href={href({ page: "decks" })}>My decks</ButtonLink></div>
      {decks.error ? <ErrorNote>{decks.error}</ErrorNote> : decks.loading ? <Loading /> : decks.data && (decks.data.length === 0
        ? <EmptyState title="No decks yet">Create or import a deck in <a href={href({ page: "decks" })}>My decks</a>.</EmptyState>
        : <ul className={styles.decks} role="list">{decks.data.slice(0, 6).map(deck => <li key={deck.id}>
          <DeckThumbnail cards={deck.commander_art} className={styles.art} />
          <div className={styles.deck}>
            <a className={styles.name} href={href({ page: "deck", deckId: deck.id })}>{deck.name}</a>
            <p>{deck.commanders.length ? deck.commanders.join(" + ") : "No commander"}</p>
            <div className={styles.details}><ColorIdentity colors={deck.color_identity} /> <span>{deck.card_count} cards</span></div>
          </div>
          <ButtonLink variant="ghost" href={href({ page: "play", deckId: deck.id })} aria-label={`Play with ${deck.name}`}>Play <span aria-hidden="true">→</span></ButtonLink>
        </li>)}</ul>)}
    </section>
  </Page>;
}
