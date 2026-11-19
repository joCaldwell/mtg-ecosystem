import { SavedGames as GameSavedGames } from "@multiverse/game/saved-games";
import type { DeckSummary } from "../../api/client.ts";
import { gameBrowserHost } from "../../app/game-host.ts";
import { DeckThumbnail } from "../../mtg/index.ts";
import styles from "./PlayPage.module.css";

export function SavedGames({ decks, loading }: { decks: readonly DeckSummary[] | null; loading: boolean }) {
  const display = decks?.map(deck => [deck.id, {
    name: deck.name,
    thumbnail: <DeckThumbnail cards={deck.commander_art} className={styles.savedArt} />,
  }] as const);
  return <GameSavedGames host={gameBrowserHost} decks={display ? new Map(display) : null} decksLoading={loading} />;
}
