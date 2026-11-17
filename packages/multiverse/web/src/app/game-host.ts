import type { GameBrowserHost } from "@multiverse/game/browser-host";
import { api, ApiError } from "../api/client.ts";
import { useQuery } from "../api/useQuery.ts";
import { href, navigate } from "./routes.ts";
import { Button, ButtonLink, ErrorNote, Loading, Page, PageHeader, Text } from "../ui/index.ts";
export const gameBrowserHost: GameBrowserHost = {
  api, ApiError, backHref: href({ page: "play" }),
  navigateGame: gameId => navigate({ page: "live-game", gameId }),
  useSavedGames: () => useQuery(() => api.listGames(), []),
  ui: { Button, ButtonLink, ErrorNote, Loading, Page, PageHeader, Text },
};
