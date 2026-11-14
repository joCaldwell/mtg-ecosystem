import { useEffect, useState } from "react";
import { GameSession } from "@multiverse/game-client/session";
import { api } from "../../api/client.ts";
import { href } from "../../app/routes.ts";
import { Button, ButtonLink, ErrorNote, Loading, Page, PageHeader } from "../../ui/index.ts";

type Grant = { gameId: string; attempt: number; file?: File; error?: string };
/** The route selects the match endpoint; imported bytes can never choose it.
 * Seat capabilities live only in this mounted grant/File and session memory. */
export function LiveGamePage({ gameId }: { gameId: string }) {
  const [attempt, setAttempt] = useState(0);
  const [grant, setGrant] = useState<Grant | null>(null);
  useEffect(() => {
    let current = true;
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 10_000);
    void api.humanGameSeat(gameId, controller.signal).then(result => {
      if (!current || controller.signal.aborted) return;
      if (typeof result.bootstrap_file !== "string" || new TextEncoder().encode(result.bootstrap_file).length > 4096 || !result.bootstrap_file.length) throw new Error("Invalid human grant");
      // No blob URL, storage, broadcast or download: the public session performs
      // generated-schema and logical-origin/match/build checks on this File.
      const file = new File([result.bootstrap_file], "human-seat.json", { type: "application/json" });
      setGrant({ gameId, attempt, file });
    }).catch(() => {
      if (current) setGrant({ gameId, attempt, error: "This game session is unavailable. Return to saved games to resume it, or try again." });
    }).finally(() => clearTimeout(timer));
    return () => { current = false; clearTimeout(timer); controller.abort(); };
  }, [gameId, attempt]);
  const current = grant?.gameId === gameId && grant.attempt === attempt ? grant : null;
  return <Page width="full" header={<PageHeader title="Game" actions={<ButtonLink href={href({ page: "play" })}>Back to saved games</ButtonLink>} />}>
    {current?.file ? <GameSession gameId={gameId} bootstrapFile={current.file} headingLevel={2} />
      : current?.error ? <><ErrorNote>{current.error}</ErrorNote><Button onClick={() => setAttempt(value => value + 1)}>Try again</Button></>
        : <Loading />}
  </Page>;
}
