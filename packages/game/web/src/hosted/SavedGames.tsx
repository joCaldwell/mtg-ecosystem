import type { GameRecord } from "../../../shared/contracts.ts";
import type { GameBrowserHost } from "./host.ts";
import { useRef, useState, useEffect, type ReactNode } from "react";
import { admissionMessage } from "./admission.ts";
import styles from "./SavedGames.module.css";

const updatedDate = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

function statusLabel(game: GameRecord): string {
  if (game.outcome?.kind === "draw") return "Draw";
  if (game.outcome?.kind === "win") return game.outcome.winner === 0 ? "You won" : `Automated seat ${game.outcome.winner + 1} won`;
  if (game.pause === "human_timeout") return "Interrupted · decision timed out";
  if (game.pause === "lifetime_exceeded") return "Interrupted · game timed out";
  return {
    active: "In progress", completed: "Complete", interrupted: "Interrupted", paused: "Paused", provisioning: "Preparing game",
  }[game.state];
}

export function SavedGames({ host, decks, decksLoading }: {
  host: GameBrowserHost;
  decks?: ReadonlyMap<number, { name: string; thumbnail: ReactNode }> | null;
  decksLoading?: boolean;
}) {
  const { api } = host;
  const { Button, ButtonLink, ErrorNote, Loading } = host.ui;
  const query = host.useSavedGames();
  const [pending, setPending] = useState<string | null>(null), [message, setMessage] = useState("");
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  async function resume(id: string) {
    if (pending) return;
    setPending(id); setMessage("");
    try {
      const result = await api.resumeGame(id);
      if (mounted.current) {
        if (result.admission.state === "supported" && result.admission.launch_ready && result.game?.state === "active") host.navigateGame(result.game.id);
        else { setMessage(admissionMessage(result.admission)); query.reload(); }
      }
    } catch { if (mounted.current) setMessage("Could not resume this game. Its saved record is retained."); }
    finally { if (mounted.current) setPending(null); }
  }
  return <section className={styles.saved} aria-labelledby="saved-games-title">
    <h2 id="saved-games-title">Saved games</h2>
    {query.error ? <><ErrorNote>Could not load saved games.</ErrorNote><Button onClick={query.reload}>Try again</Button></> : query.loading || decksLoading ? <Loading /> : !query.data?.games.length ? <p className={styles.empty}>No saved games yet.</p> : <ul className={styles.list} role="list">
      {query.data.games.map(game => {
        const deck = decks?.get(game.deck.deck_id);
        return <li key={game.id}>
          <div className={styles.deck}>
            {deck?.thumbnail}
            <div>
              <strong>{deck?.name ?? (decks ? "Deleted deck" : "Deck unavailable")}</strong>
              <div className={styles.meta}>
                <span>{statusLabel(game)}</span>
                <time dateTime={new Date(game.updated_at).toISOString()}>Updated {updatedDate.format(game.updated_at)}</time>
              </div>
            </div>
          </div>
          {game.state === "completed" || game.state === "interrupted"
            ? <ButtonLink href={host.gameHref(game.id)}>{game.state === "completed" ? "View result" : "View game"}</ButtonLink>
            : <Button disabled={pending !== null} onClick={() => void resume(game.id)}>{pending === game.id ? "Resuming…" : "Resume game"}</Button>}
        </li>;
      })}
    </ul>}
    <p role="status" className={styles.message}>{message}</p>
  </section>;
}
