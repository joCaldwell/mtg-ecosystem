import type { PlaytestAction, PlaytestView } from "../../api/client.ts";
import { Button, Menu } from "../../ui/index.ts";
import styles from "./SessionControls.module.css";

export function SessionControls({
  view, busy, status, onAction, onDrawSeveral, onCreateToken, onRestart, onObservation,
}: {
  view: PlaytestView;
  busy: boolean;
  status: "saved" | "saving" | "failed";
  onAction: (action: PlaytestAction) => void;
  onDrawSeveral: () => void;
  onCreateToken: () => void;
  onRestart: () => void;
  onObservation: () => void;
}) {
  const playing = view.session.frame.opening.stage === "playing";
  return (
    <section className={styles.controls} aria-label="Session controls">
      <div className={styles.heading}>
        <span>Session</span>
        <span role="status">{busy || status === "saving" ? "Saving…" : status === "failed" ? "Save failed" : "Saved"}</span>
      </div>
      <div className={styles.primary}>
        <Button variant="primary" kbd="D" disabled={!playing || busy || !view.computed.counts.library}
          onClick={() => onAction({ type: "draw", count: 1 })}>Draw</Button>
        <Button kbd="N" disabled={!playing || busy} title="Untap and advance the turn; draw separately"
          onClick={() => onAction({ type: "next-turn" })}>Next turn</Button>
      </div>
      <div className={styles.secondary}>
        <Button size="sm" variant="ghost" disabled={busy || !view.computed.can_undo}
          title="Undo (Z)" onClick={() => onAction({ type: "undo" })}>Undo</Button>
        <Button size="sm" variant="ghost" disabled={busy || !view.computed.can_redo}
          title="Redo (Shift+Z)" onClick={() => onAction({ type: "redo" })}>Redo</Button>
        <Menu label="More" size="sm" variant="ghost" align="end" items={[
          { id: "draw", label: "Draw several…", disabled: !playing || busy || !view.computed.counts.library, onSelect: onDrawSeveral },
          { id: "shuffle", label: "Shuffle library", disabled: !playing || busy, onSelect: () => onAction({ type: "shuffle" }) },
          { id: "untap", label: "Untap all", disabled: !playing || busy, onSelect: () => onAction({ type: "untap-all" }) },
          { id: "token", label: "Create token", disabled: !playing || busy, onSelect: onCreateToken },
          { id: "observation", label: "Deck observation", onSelect: onObservation },
          { id: "restart", label: "New opening test", disabled: busy, onSelect: onRestart },
        ]} />
      </div>
    </section>
  );
}
