import { useMemo } from "react";
import { LiveGamePage as GameLiveGamePage } from "@multiverse/game/live-page";
import type { GameBrowserHost } from "@multiverse/game/browser-host";
import { beginGameRequestBudget, productFetch, waitForGameRequestCooldown } from "../../api/request-budget.ts";
import { gameBrowserHost } from "../../app/game-host.ts";
import { useSession } from "../../app/Session.tsx";

export function LiveGamePage({ gameId }: { gameId: string }) {
  const { session } = useSession();
  const policy = session.mode === "hosted" && session.user ? session.ui_request_budget : undefined;
  const count = policy?.count;
  const windowMs = policy?.window_ms;
  const host = useMemo<GameBrowserHost>(() => {
    if (count === undefined || windowMs === undefined) return gameBrowserHost;
    return {
      ...gameBrowserHost,
      requestFetch: productFetch,
      startRequestBudget: () => beginGameRequestBudget({ count, window_ms: windowMs }),
      api: {
        ...gameBrowserHost.api,
        async getGame(id, signal) {
          await waitForGameRequestCooldown(signal);
          return gameBrowserHost.api.getGame(id, signal);
        },
      },
    };
  }, [count, windowMs]);
  return <GameLiveGamePage key={session.user?.id} host={host} gameId={gameId} />;
}
