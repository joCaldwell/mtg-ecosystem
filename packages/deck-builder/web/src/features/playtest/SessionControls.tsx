import type { PlaytestAction, PlaytestView } from "../../api/client.ts";
import { Button, Dot, Icon, IconButton, Menu } from "../../ui/index.ts";
import styles from "./SessionControls.module.css";

export function SessionControls({
  view, busy, status, onAction, onDrawSeveral, onCreateToken, onRandomNumber, onRestart, onObservation,
}: {
  view: PlaytestView;
  busy: boolean;
  status: "saved" | "saving" | "failed";
  onAction: (action: PlaytestAction) => void;
  onDrawSeveral: () => void;
  onCreateToken: () => void;
  onRandomNumber: () => void;
  onRestart: () => void;
  onObservation: () => void;
}) {
  const playing = view.session.frame.opening.stage === "playing";
  const saving = busy || status === "saving";
  return (
    <section className={styles.controls} aria-label="Session controls">
      {/* Turn actions appear once a hand is kept; until then the opening prompt leads. */}
      {playing && (
        <>
          <div className={styles.group}>
            <Button variant="primary" kbd="D" disabled={busy || !view.computed.counts.library}
              onClick={() => onAction({ type: "draw", count: 1 })}>Draw</Button>
            <Button kbd="N" disabled={busy} title="Untap and advance the turn; draw separately"
              onClick={() => onAction({ type: "next-turn" })}>Next turn</Button>
          </div>
          <div className={styles.group}>
            <Button kbd="U" disabled={busy}
              onClick={() => onAction({ type: "untap-all" })}>Untap all</Button>
            <Button kbd="S" disabled={busy}
              onClick={() => onAction({ type: "shuffle" })}>Shuffle</Button>
          </div>
        </>
      )}
      <div className={styles.group}>
        <IconButton label="Undo (Z)" disabled={busy || !view.computed.can_undo}
          onClick={() => onAction({ type: "undo" })}><Icon name="undo" /></IconButton>
        <IconButton label="Redo (Shift+Z)" disabled={busy || !view.computed.can_redo}
          onClick={() => onAction({ type: "redo" })}><Icon name="redo" /></IconButton>
        <Menu label="More" variant="ghost" align="end" items={[
          { id: "draw", label: "Draw several…", disabled: !playing || busy || !view.computed.counts.library, onSelect: onDrawSeveral },
          { id: "token", label: "Create token", disabled: !playing || busy, onSelect: onCreateToken },
          { id: "random", label: "Random number…", onSelect: onRandomNumber },
          { id: "observation", label: "Deck observation", onSelect: onObservation },
          { id: "restart", label: "New game…", disabled: busy, onSelect: onRestart },
        ]} />
      </div>
      <span className={styles.status} role="status" data-state={saving ? "saving" : status}>
        <Dot tone={status === "failed" && !saving ? "cut" : "neutral"} pulse={saving} />
        {saving ? "Saving…" : status === "failed" ? "Save failed" : "Saved"}
      </span>
    </section>
  );
}
