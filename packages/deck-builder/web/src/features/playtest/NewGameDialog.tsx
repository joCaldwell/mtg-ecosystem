import { useState } from "react";
import type { Mode } from "../../api/client.ts";
import { ManaCost } from "../../mtg/index.ts";
import { Button, Dialog, ErrorNote, Text } from "../../ui/index.ts";
import styles from "./NewGameDialog.module.css";

const MODES: Array<{ value: Mode; label: string; description: string }> = [
  {
    value: "commander",
    label: "Commander mulligans",
    description: "First mulligan free, then bottom one card for each after it.",
  },
  {
    value: "practice",
    label: "Practice redraws",
    description: "Redraw seven as often as you like.",
  },
];

/** Starts a game over the table, the first time or as a replacement. What a
 * replacement discards is stated beside the button that discards it. */
export function NewGameDialog({
  deckName,
  revision,
  commanders,
  initialMode = "commander",
  replaces,
  keepsDraft = false,
  error,
  busy,
  onRetry,
  onStart,
  onClose,
}: {
  deckName: string;
  revision: number;
  commanders: Array<{ oracle_id: string; name: string; mana_cost: string | null }>;
  initialMode?: Mode;
  /** What starting would replace: the current table, or a saved one that could not be read. */
  replaces?: "table" | "unreadable";
  keepsDraft?: boolean;
  error?: string | null;
  busy: boolean;
  onRetry?: () => void;
  onStart: (mode: Mode) => void;
  onClose: () => void;
}) {
  const [mode, setMode] = useState<Mode>(initialMode);
  return (
    <Dialog
      title="New game"
      onClose={onClose}
      footer={
        <>
          {replaces && (
            <span className={styles.consequence}>
              {replaces === "unreadable"
                ? "Replaces the saved table. Memories and tags stay."
                : `Replaces this table and its undo history. Memories and tags stay${keepsDraft ? ", and so does your unfinished observation" : ""}.`}
            </span>
          )}
          {replaces === "table" && (
            <Button variant="ghost" onClick={onClose}>
              Cancel
            </Button>
          )}
          <Button
            variant={replaces ? "reject" : "primary"}
            disabled={busy}
            data-autofocus={replaces ? undefined : ""}
            onClick={() => onStart(mode)}
          >
            {busy ? "Shuffling…" : "Shuffle and draw seven"}
          </Button>
        </>
      }
    >
      <div className={styles.deck}>
        <strong>{deckName}</strong>
        <span className={styles.identity}>
          {commanders.map((c) => (
            <span key={c.oracle_id} className={styles.commander}>
              {c.name}
              <ManaCost cost={c.mana_cost} />
            </span>
          ))}
          <Text mono tone="muted">r{revision}</Text>
        </span>
      </div>
      {error && (
        <ErrorNote>
          {error}
          {onRetry && (
            <Button size="sm" className={styles.retry} onClick={onRetry} disabled={busy}>
              Retry loading it
            </Button>
          )}
        </ErrorNote>
      )}
      <fieldset className={styles.modes} disabled={busy}>
        <legend className={styles.legend}>Opening hands</legend>
        {MODES.map((option) => (
          <label className={styles.option} key={option.value}>
            <input
              className={styles.radio}
              type="radio"
              name="opening-mode"
              value={option.value}
              checked={mode === option.value}
              onChange={() => setMode(option.value)}
              data-autofocus={replaces && mode === option.value ? "" : undefined}
              aria-describedby={`mode-${option.value}-description`}
            />
            <span className={styles.optionText}>
              <span className={styles.optionTitle}>{option.label}</span>
              <span id={`mode-${option.value}-description`} className={styles.optionDescription}>
                {option.description}
              </span>
            </span>
          </label>
        ))}
      </fieldset>
    </Dialog>
  );
}
