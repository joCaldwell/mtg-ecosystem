import { OwnedDeckPreview } from "./OwnedDeckPreview.tsx";
import { GamePreview } from "./GamePreview.tsx";

export function GamePreviewPage({ deckId }: { deckId?: number }) {
  return deckId === undefined ? <GamePreview /> : <OwnedDeckPreview deckId={deckId} key={deckId} />;
}
