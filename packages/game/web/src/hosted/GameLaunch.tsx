import type { GameAdmission, GameRecord } from "../../../shared/contracts.ts";
import type { GameBrowserHost } from "./host.ts";
import { useEffect, useRef, useState } from "react";
import styles from "./GameLaunch.module.css";

import { admissionMessage } from "./admission.ts";

/** Keyed by deck/revision; obsolete responses cannot authorize a new selection.
 * Retry keeps the same key after an uncertain launch, never mints another game. */
export function GameLaunch({ deckId, revision, host }: { deckId: number; revision: number; host: GameBrowserHost }) {
  const { api } = host;
  const { Button, ErrorNote } = host.ui;
  const [admission, setAdmission] = useState<GameAdmission>({ state: "not_checked" });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [game, setGame] = useState<GameRecord | null>(null);
  const mounted = useRef(true), requestId = useRef<string | null>(null);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  async function perform(launch: boolean) {
    if (busy) return;
    setBusy(true); setError("");
    if (!launch) setAdmission({ state: "checking" });
    try {
      if (launch) requestId.current ??= crypto.randomUUID();
      const result: { admission: GameAdmission; game?: GameRecord } = launch ? await api.launchGame(deckId, revision, requestId.current!) : await api.gameAdmission(deckId, revision);
      if (!mounted.current) return;
      setAdmission(result.admission);
      if ("game" in result && result.game) {
        setGame(result.game);
        if (result.admission.state === "supported" && result.admission.launch_ready && result.game.state === "active") host.navigateGame(result.game.id);
      }
    } catch { if (mounted.current) { setError(launch ? "The launch outcome is unknown. Retry uses the same game request." : "Could not check compatibility. Try again."); if (!launch) setAdmission({ state: "unavailable", reason: "native_unavailable" }); } }
    finally { if (mounted.current) setBusy(false); }
  }
  const ready = admission.state === "supported" && admission.launch_ready && !game;
  return <section className={styles.launch} aria-label="Game launch" aria-busy={busy}>
    <p id="game-admission" aria-live="polite">{admissionMessage(admission)}</p>
    {error && <ErrorNote>{error}</ErrorNote>}
    <div className={styles.actions}>
      <Button {...(ready ? {} : { variant: "primary" as const })} disabled={busy} onClick={() => void perform(false)}>Check compatibility</Button>
      <Button variant="primary" disabled={busy || !ready} aria-describedby="game-admission" onClick={() => void perform(true)}>Start game</Button>
    </div>
    {game && <p role="status">{game.state === "active" ? "Your game is ready." : game.state === "completed" ? "Your game is complete." : "Your game is saved. Its session is currently paused."}</p>}
  </section>;
}
