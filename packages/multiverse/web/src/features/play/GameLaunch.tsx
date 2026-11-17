import { GameLaunch as GameGameLaunch } from "@multiverse/game/launch";
import { gameBrowserHost } from "../../app/game-host.ts";
export function GameLaunch({ deckId, revision }: { deckId: number; revision: number }) {
  return <GameGameLaunch host={gameBrowserHost} deckId={deckId} revision={revision} />;
}
