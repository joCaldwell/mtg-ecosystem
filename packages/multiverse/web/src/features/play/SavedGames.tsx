import { useRef, useState, useEffect } from "react";
import { navigate } from "../../app/routes.ts";
import { api } from "../../api/client.ts";
import { useQuery } from "../../api/useQuery.ts";
import { admissionMessage } from "../../domain/gameLaunch.ts";
import { Button, ErrorNote, Loading, Text } from "../../ui/index.ts";
import styles from "./SavedGames.module.css";

export function SavedGames() {
  const query = useQuery(() => api.listGames(), []);
  const [pending, setPending] = useState<string | null>(null), [message, setMessage] = useState("");
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  async function resume(id: string) {
    if (pending) return;
    setPending(id); setMessage("");
    try {
      const result = await api.resumeGame(id);
      if (mounted.current) {
        if (result.admission.state === "supported" && result.admission.launch_ready && result.game?.state === "active") navigate({ page: "live-game", gameId: result.game.id });
        else { setMessage(admissionMessage(result.admission)); query.reload(); }
      }
    } catch { if (mounted.current) setMessage("Could not resume this game. Its saved record is retained."); }
    finally { if (mounted.current) setPending(null); }
  }
  return <section className={styles.saved} aria-labelledby="saved-games-title">
    <h2 id="saved-games-title">Saved games</h2>
    {query.error ? <><ErrorNote>{query.error}</ErrorNote><Button onClick={query.reload}>Try again</Button></> : query.loading ? <Loading /> : !query.data?.games.length ? <Text as="p" tone="muted">No games have been started from your account.</Text> : <ul className={styles.list} role="list">
      {query.data.games.map(game => <li key={game.id}>
        <div><Text mono>Deck {game.deck.deck_id} · r{game.deck.revision}</Text><Text as="p" tone="muted">{game.outcome?.kind === "draw" ? "Draw" : game.outcome?.kind === "win" ? game.outcome.winner === 0 ? "You won" : `Automated seat ${game.outcome.winner + 1} won` : game.pause === "human_timeout" ? "Interrupted after the human decision time limit" : game.pause === "lifetime_exceeded" ? "Interrupted after the game time limit" : game.state === "interrupted" ? "Interrupted" : game.state === "active" ? "In progress" : "Paused"}</Text></div>
        {game.state !== "completed" && game.state !== "interrupted" && <Button disabled={pending !== null} onClick={() => void resume(game.id)}>Resume game</Button>}
      </li>)}
    </ul>}
    <p role="status">{message}</p>
  </section>;
}
