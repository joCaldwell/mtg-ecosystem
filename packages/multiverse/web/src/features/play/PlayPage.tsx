import { api } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { href, navigate } from "../../app/routes.ts";
import { useSession } from "../../app/Session.tsx";
import { DeckArt } from "../../mtg/index.ts";
import { ButtonLink, EmptyState, ErrorNote, Loading, Page, PageHeader, Select } from "../../ui/index.ts";
import { SavedGames } from "./SavedGames.tsx";
import { SelectedDeck } from "./SelectedDeck.tsx";
import styles from "./PlayPage.module.css";

export function PlayPage({ deckId }: { deckId?: number }) {
  const decks = useQuery(() => api.listDecks(), [], "decks");
  const { session } = useSession();
  return <Page width="lg" header={<PageHeader title="New game" />}>
    <div className={styles.setup}>
      <div className={styles.controls}>
        {decks.error ? <ErrorNote>{decks.error}</ErrorNote> : decks.loading ? <Loading /> : decks.data && (decks.data.length === 0
          ? <EmptyState title="Your library is empty"><ButtonLink href={href({ page: "decks" })}>Go to My decks</ButtonLink></EmptyState>
          : <div className={styles.selector}>
            <label id="your-deck-label">Your deck</label>
            <Select aria-labelledby="your-deck-label" value={deckId === undefined ? "" : String(deckId)} placeholder="Choose a deck"
              options={decks.data.map(deck => ({ value: String(deck.id), label: deck.name }))}
              onChange={value => navigate({ page: "play", deckId: Number(value) })} />
          </div>)}
        <div className={styles.format}>
          <p><strong>Commander</strong><span>3 automated opponents</span></p>
          <ButtonLink href={href({ page: "game-preview", deckId })}>Preview table</ButtonLink>
        </div>
      </div>
      <section className={styles.deck} aria-label="Selected deck">
        {deckId !== undefined ? <SelectedDeck deckId={deckId} key={deckId} /> :
          <DeckArt cards={session.backdrop ? [session.backdrop] : null} variant="feature" priority="high" />}
      </section>
      <SavedGames decks={decks.data} loading={decks.loading} />
    </div>
  </Page>;
}
