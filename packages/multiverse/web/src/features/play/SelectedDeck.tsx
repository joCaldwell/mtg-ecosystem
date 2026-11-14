import { GameLaunch } from "./GameLaunch.tsx";
import { api } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { href } from "../../app/routes.ts";
import { Button, ButtonLink, EmptyState, Loading, Text } from "../../ui/index.ts";
import styles from "./PlayPage.module.css";

/** Keyed by deck ID by the parent: changing the URL creates a fresh query state,
 * so a delayed prior read cannot label the new selection with another deck. */
export function SelectedDeck({ deckId }: { deckId: number }) {
  const query = useQuery(() => api.getDeck(deckId), [deckId], { resources: ["deck"], deckId });
  if (query.error) return <EmptyState title="Deck unavailable"><p>This deck is missing or cannot be opened from your account.</p><Button onClick={query.reload}>Try again</Button></EmptyState>;
  if (!query.data) return <Loading />;
  const { state } = query.data;
  const commanders = state.cards.filter(card => card.role === "commander" && !card.maybe);
  return <><div className={styles.selected} aria-label="Selected deck">
    <div className={styles.selectedHeading}><h3>{state.deck.name}</h3><ButtonLink href={href({ page: "deck", deckId })}>Edit deck</ButtonLink></div>
    <Text as="p" tone="soft">{commanders.length ? commanders.map(card => card.name).join(" + ") : "No commander selected"}</Text>
    <dl className={styles.facts}><div><dt>Cards</dt><dd>{state.computed.card_count}</dd></div><div><dt>Deck revision</dt><dd>{state.deck.revision}</dd></div></dl>
    <Text as="p" tone="muted">This is your current saved deck. A game will keep the exact deck version it starts with.</Text>
  </div><GameLaunch key={state.deck.revision} deckId={deckId} revision={state.deck.revision} /></>;
}
