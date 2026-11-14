import { api } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { href } from "../../app/routes.ts";
import { EmptyState, Loading, Page } from "../../ui/index.ts";
import { GamePreview } from "./GamePreview.tsx";

export function OwnedDeckPreview({ deckId }: { deckId: number }) {
  const deck = useQuery(() => api.getDeck(deckId), [deckId], { resources: ["deck"], deckId });
  if (deck.error) return <Page><EmptyState heading="h1" title="Deck unavailable">This deck is missing or cannot be opened from your account. <a href={href({ page: "play" })}>Back to setup</a>.</EmptyState></Page>;
  return deck.data ? <GamePreview deckId={deckId} deckName={deck.data.state.deck.name} /> : <Loading />;
}
