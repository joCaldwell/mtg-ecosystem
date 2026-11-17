import { LiveGamePage as GameLiveGamePage } from "@multiverse/game/live-page";
import { gameBrowserHost } from "../../app/game-host.ts";
export function LiveGamePage({ gameId }: { gameId: string }) {
  return <GameLiveGamePage host={gameBrowserHost} gameId={gameId} />;
}
