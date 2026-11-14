import { api } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { href, navigate } from "../../app/routes.ts";
import { Button, ButtonLink, EmptyState, ErrorNote, Loading, Page, PageHeader, Select, Text } from "../../ui/index.ts";
import { SavedGames } from "./SavedGames.tsx";
import { SelectedDeck } from "./SelectedDeck.tsx";
import styles from "./PlayPage.module.css";

export function PlayPage({ deckId }: { deckId?: number }) {
  const decks = useQuery(() => api.listDecks(), [], "decks");
  return <Page width="lg" header={<PageHeader title="New game" meta="Commander · automated opponents" />}>
    <div className={styles.setup}>
      <section className={styles.choose} aria-labelledby="choose-deck-title">
        <h2 id="choose-deck-title">Choose your deck</h2>
        {decks.error ? <ErrorNote>{decks.error}</ErrorNote> : decks.loading ? <Loading /> : decks.data && (decks.data.length === 0
          ? <EmptyState title="Your library is empty">Create or import a deck in <a href={href({ page: "decks" })}>My decks</a>.</EmptyState>
          : <div className={styles.selector}><label id="your-deck-label">Your deck</label>
            <Select aria-labelledby="your-deck-label" value={deckId === undefined ? "" : String(deckId)} placeholder="Choose a deck"
              options={decks.data.map(deck => ({ value: String(deck.id), label: deck.name }))}
              onChange={value => navigate({ page: "play", deckId: Number(value) })} /></div>)}
        {deckId !== undefined ? <SelectedDeck deckId={deckId} key={deckId} /> : <Text as="p" tone="muted">Select a deck from your account to review it for this game.</Text>}
      </section>
      <section className={styles.opponentSection} aria-labelledby="opponents-title">
        <h2 id="opponents-title">Your opponents</h2>
        <Text as="p" tone="muted">Three predefined automated opponents. Each controls its own seat.</Text>
        <ol className={styles.opponents}>{[2, 3, 4].map(seat => <li key={seat}><Text mono>Seat {seat}</Text><span>Automated opponent</span></li>)}</ol>
      </section>
      <section className={styles.preview} aria-labelledby="table-preview-title">
        <div><h2 id="table-preview-title">Explore the table</h2><Text as="p" tone="muted">Try opening choices with a captured sample. The preview does not use your selected deck or start a game.</Text></div>
        <ButtonLink href={href({ page: "game-preview", deckId })}>Preview table</ButtonLink>
      </section>
      <SavedGames />
    </div>
  </Page>;
}
