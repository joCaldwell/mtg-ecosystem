import { useId, useState } from "react";
import { parseRange, rollInRange, type RandomRange } from "../../domain/random.ts";
import { Button, Dialog, ErrorNote, Field, TextField, useLocalStorage } from "../../ui/index.ts";
import styles from "./RandomNumberDialog.module.css";

export interface Roll extends RandomRange {
  value: number;
}

const DEFAULT_RANGE = { lower: "1", upper: "6" };

/** Rolls stay in the browser: they are table aids, not game actions, so they
 * are not saved with the session and game undo cannot reroll them. */
export function RandomNumberDialog({
  rolls,
  onRoll,
  onClose,
}: {
  rolls: Roll[];
  onRoll: (roll: Roll) => void;
  onClose: () => void;
}) {
  const formId = useId();
  const [saved, setSaved] = useLocalStorage(
    "playtest.randomRange",
    (raw) => {
      try {
        const v = raw ? JSON.parse(raw) : null;
        return typeof v?.lower === "string" && typeof v?.upper === "string"
          ? { lower: v.lower as string, upper: v.upper as string }
          : DEFAULT_RANGE;
      } catch {
        return DEFAULT_RANGE;
      }
    },
    (v) => JSON.stringify(v),
  );
  const [lower, setLower] = useState(saved.lower),
    [upper, setUpper] = useState(saved.upper);
  const range = parseRange(lower, upper);
  const [latest, ...earlier] = rolls;
  function roll() {
    if ("error" in range) return;
    setSaved({ lower, upper });
    onRoll({ ...range, value: rollInRange(range) });
  }
  return (
    <Dialog
      title="Random number"
      onClose={onClose}
      footer={
        <Button variant="primary" type="submit" form={formId} disabled={"error" in range}>
          {latest ? "Roll again" : "Roll"}
        </Button>
      }
    >
      <form
        id={formId}
        className={styles.form}
        onSubmit={(e) => {
          e.preventDefault();
          roll();
        }}
      >
        <div className={styles.bounds}>
          <Field label="Lower bound">
            <TextField data-autofocus type="number" step={1} inputMode="numeric"
              value={lower} onChange={(e) => setLower(e.target.value)} />
          </Field>
          <Field label="Upper bound">
            <TextField type="number" step={1} inputMode="numeric"
              value={upper} onChange={(e) => setUpper(e.target.value)} />
          </Field>
        </div>
        {"error" in range ? (
          <ErrorNote>{range.error}</ErrorNote>
        ) : (
          <p className={styles.hint}>Both bounds are included.</p>
        )}
      </form>
      <div className={styles.result} role="status" aria-live="polite">
        {latest ? (
          <>
            <span className={styles.value}>{latest.value}</span>
            <span className={styles.range}>
              from {latest.min} to {latest.max}
            </span>
          </>
        ) : (
          <span className={styles.range}>No roll yet</span>
        )}
      </div>
      {earlier.length > 0 && (
        <p className={styles.history}>
          Earlier:{" "}
          {earlier.map((r, i) => (
            <span key={i} title={`${r.min} to ${r.max}`}>
              {i > 0 && ", "}
              {r.value}
            </span>
          ))}
        </p>
      )}
    </Dialog>
  );
}
