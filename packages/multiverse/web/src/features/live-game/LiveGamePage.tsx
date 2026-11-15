import { useEffect, useState } from "react";
import { GameSession } from "@multiverse/game-client/session";
import { api, ApiError, type GameRecord } from "../../api/client.ts";
import { href } from "../../app/routes.ts";
import { Button, ButtonLink, ErrorNote, Loading, Page, PageHeader } from "../../ui/index.ts";

type Grant = { gameId: string; attempt: number; file?: File; error?: string };
type Saved = { gameId: string; attempt: number; game?: GameRecord; error?: string; denied?: boolean };
function savedStatus(game: GameRecord): string {
  if (game.state === "completed") return game.outcome?.kind === "draw" ? "The game ended in a draw."
    : game.outcome?.kind === "win" ? game.outcome.winner === 0 ? "You won." : `Automated seat ${game.outcome.winner + 1} won.` : "The game is complete.";
  if (game.state === "provisioning") return "Your game is being prepared.";
  if (game.pause === "human_timeout") return "The game stopped after the human decision time limit. Its saved record is retained.";
  if (game.pause === "lifetime_exceeded") return "The game stopped after the game time limit. Its saved record is retained.";
  if (game.state === "interrupted") return "The game was interrupted. Its saved record is retained.";
  return "The game is paused. Return to saved games to resume.";
}
/** The route selects the match endpoint; imported bytes can never choose it.
 * Seat capabilities live only in this mounted grant/File and session memory. */
export function LiveGamePage({ gameId }: { gameId: string }) {
  const [attempt, setAttempt] = useState(0);
  const [grant, setGrant] = useState<Grant | null>(null);
  const [saved, setSaved] = useState<Saved | null>(null);
  useEffect(() => {
    let current = true, grantAllowed = true, revoked = false;
    let statusRequest: AbortController | undefined;
    let nextStatus: ReturnType<typeof setTimeout> | undefined;
    setGrant(null); setSaved(null);
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 10_000);
    // The owner record survives native shutdown. It is result/pause authority,
    // never evidence that an uncertain command was consumed or cancelled.
    async function refreshStatus() {
      const request = new AbortController(); statusRequest = request;
      const deadline = setTimeout(() => request.abort(), 10_000);
      let again = 5_000;
      try {
        const { game } = await api.getGame(gameId, request.signal);
        if (!current || request.signal.aborted) return;
        if (game.id !== gameId) throw new ApiError(403, "Game identity differs");
        grantAllowed = game.state === "active";
        setSaved({ gameId, attempt, game });
        if (game.state === "completed" || game.state === "interrupted") again = 0;
      } catch (error) {
        if (!current) return;
        const denied = error instanceof ApiError && [401, 403, 404].includes(error.status);
        if (denied) { revoked = true; grantAllowed = false; controller.abort(); setGrant(null); again = 0; }
        else again = 5_000;
        setSaved(previous => ({ ...(previous?.gameId === gameId && previous.attempt === attempt && !denied ? previous : {}), gameId, attempt, denied,
          error: denied ? "This game is no longer available from your account." : "Could not refresh game status. Retrying…" }));
      } finally { clearTimeout(deadline); }
      if (current && again) nextStatus = setTimeout(() => { void refreshStatus(); }, again);
    }
    void refreshStatus();
    void api.humanGameSeat(gameId, controller.signal).then(result => {
      if (!current || controller.signal.aborted || revoked) return;
      if (!grantAllowed) {
        setGrant({ gameId, attempt, error: "The game changed while its session was opening. Try again to reconnect." });
        return;
      }
      if (typeof result.bootstrap_file !== "string" || new TextEncoder().encode(result.bootstrap_file).length > 4096 || !result.bootstrap_file.length) throw new Error("Invalid human grant");
      // No blob URL, storage, broadcast or download: the public session performs
      // generated-schema and logical-origin/match/build checks on this File.
      const file = new File([result.bootstrap_file], "human-seat.json", { type: "application/json" });
      setGrant({ gameId, attempt, file });
    }).catch(() => {
      if (current && !revoked) setGrant({ gameId, attempt, error: "This game session is unavailable. Return to saved games to resume it, or try again." });
    }).finally(() => clearTimeout(timer));
    return () => { current = false; clearTimeout(timer); clearTimeout(nextStatus); controller.abort(); statusRequest?.abort(); };
  }, [gameId, attempt]);
  const current = grant?.gameId === gameId && grant.attempt === attempt ? grant : null;
  const record = saved?.gameId === gameId && saved.attempt === attempt ? saved : null;
  const game = record?.game;
  const inactive = !!game && game.state !== "active";
  return <Page width="full" header={<PageHeader title="Game" actions={<ButtonLink href={href({ page: "play" })}>Back to saved games</ButtonLink>} />}>
    {inactive && <section aria-labelledby="saved-game-status"><h2 id="saved-game-status">{game.state === "completed" ? "Game complete" : game.state === "provisioning" ? "Preparing game" : game.state === "paused" ? "Game paused" : "Game interrupted"}</h2><p role="status">{savedStatus(game)}</p></section>}
    {record?.error && <ErrorNote>{record.error}</ErrorNote>}
    {current?.file ? <GameSession gameId={gameId} bootstrapFile={current.file} headingLevel={2} interactive={!inactive && record?.denied !== true} />
      : inactive ? null
      : current?.error ? <><ErrorNote>{current.error}</ErrorNote><Button onClick={() => setAttempt(value => value + 1)}>Try again</Button></>
        : record?.denied ? <Button onClick={() => setAttempt(value => value + 1)}>Try again</Button>
        : <Loading />}
  </Page>;
}
