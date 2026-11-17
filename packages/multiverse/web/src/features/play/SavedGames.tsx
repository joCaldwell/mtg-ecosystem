import { SavedGames as GameSavedGames } from "@multiverse/game/saved-games";
import { gameBrowserHost } from "../../app/game-host.ts";
export function SavedGames() {
  return <GameSavedGames host={gameBrowserHost} />;
}
