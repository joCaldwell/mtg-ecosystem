import { GameLaunch } from "./GameLaunch.tsx";
import { api } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { href } from "../../app/routes.ts";
import { ColorIdentity, DeckArt } from "../../mtg/index.ts";
import { Button, ButtonLink, EmptyState, Loading } from "../../ui/index.ts";
import styles from "./PlayPage.module.css";

/** A later response for another deck cannot replace the keyed selection. */
export function SelectedDeck({ deckId }: { deckId: number }) {
  const query = useQuery(() => api.getDeck(deckId), [deckId], { resources: ["deck"], deckId });
  if (query.error) return <EmptyState title="Deck unavailable"><Button onClick={query.reload}>Try again</Button></EmptyState>;
  if (!query.data) return <div className={styles.lobbyArt}><Loading /></div>;
  const { state } = query.data;
  const commanders = state.cards.filter(card => card.role === "commander" && !card.maybe);
  const identity = state.deck.color_identity;
  return <>
    <DeckArt cards={commanders} identity={identity} variant="feature">
      <div className={styles.deckCaption}>
        <div className={styles.deckName}>
          <h2>{state.deck.name}</h2>
          <p>{commanders.length ? commanders.map(card => card.name).join(" + ") : "No commander"}</p>
        </div>
        <div className={styles.deckDetails}>
          <span className={styles.metadata}><ColorIdentity colors={identity} /> <span>{state.computed.card_count} cards</span></span>
          <ButtonLink variant="ghost" href={href({ page: "deck", deckId })}>Edit deck</ButtonLink>
        </div>
      </div>
    </DeckArt>
    <GameLaunch key={state.deck.revision} deckId={deckId} revision={state.deck.revision} />
  </>;
}
