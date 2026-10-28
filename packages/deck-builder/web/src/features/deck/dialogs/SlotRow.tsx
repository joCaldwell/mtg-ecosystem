import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import type { SlotDelta } from "../../../api/client.ts";
import { Button, Text, TextField } from "../../../ui/index.ts";
import { ConfirmStrip } from "./ConfirmStrip.tsx";
import { parseTarget, TARGET_ERROR, targetText } from "./targets.ts";
import styles from "./SlotsTable.module.css";

export interface SlotPatch {
  name?: string;
  target_min?: number | null;
  target_max?: number | null;
}

interface Props {
  slot: SlotDelta;
  /** Resolves false when the server refused, so the typed text stays for fixing. */
  onSave: (slotId: number, patch: SlotPatch) => Promise<boolean>;
  onInvalid: (message: string) => void;
  /** An edit that was refused and then taken back leaves no error behind. */
  onRevert: () => void;
  onDelete: (slotId: number) => void;
}

export function SlotRow({ slot, onSave, onInvalid, onRevert, onDelete }: Props) {
  const [name, setName] = useState(slot.name);
  const [min, setMin] = useState(targetText(slot.target_min));
  const [max, setMax] = useState(targetText(slot.target_max));
  const [confirming, setConfirming] = useState(false);

  // Follow the server: after a save, or when an agent changes the slot.
  useEffect(() => setName(slot.name), [slot.name]);
  useEffect(() => setMin(targetText(slot.target_min)), [slot.target_min]);
  useEffect(() => setMax(targetText(slot.target_max)), [slot.target_max]);

  // Enter and then blur both commit; the second must not repeat the request.
  const saving = useRef(false);

  async function commit() {
    if (saving.current) return;
    const patch: SlotPatch = {};
    if (name.trim() !== slot.name) patch.name = name.trim();
    const lo = parseTarget(min);
    const hi = parseTarget(max);
    if (!lo.ok || !hi.ok) return onInvalid(TARGET_ERROR);
    if (lo.value !== slot.target_min) patch.target_min = lo.value;
    if (hi.value !== slot.target_max) patch.target_max = hi.value;
    if (Object.keys(patch).length === 0) return revert();
    saving.current = true;
    try {
      await onSave(slot.slot_id, patch);
    } finally {
      saving.current = false;
    }
  }

  function revert() {
    setName(slot.name);
    setMin(targetText(slot.target_min));
    setMax(targetText(slot.target_max));
  }

  const dirty =
    name !== slot.name || min !== targetText(slot.target_min) || max !== targetText(slot.target_max);

  function onKeyDown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      void commit();
    } else if (e.key === "Escape" && dirty) {
      // Undo the edit first; a second Escape closes the dialog.
      e.stopPropagation();
      revert();
      onRevert();
    }
  }

  return (
    <>
      <tr>
        <td>
          <TextField
            value={name}
            onChange={(e) => setName(e.target.value)}
            onBlur={() => void commit()}
            onKeyDown={onKeyDown}
            aria-label={`Name of the slot ${slot.name}`}
          />
        </td>
        <td>
          <TextField
            className={styles.num}
            inputMode="numeric"
            value={min}
            placeholder="none"
            onChange={(e) => setMin(e.target.value)}
            onBlur={() => void commit()}
            onKeyDown={onKeyDown}
            aria-label={`Minimum for ${slot.name}`}
          />
        </td>
        <td>
          <TextField
            className={styles.num}
            inputMode="numeric"
            value={max}
            placeholder="none"
            onChange={(e) => setMax(e.target.value)}
            onBlur={() => void commit()}
            onKeyDown={onKeyDown}
            aria-label={`Maximum for ${slot.name}`}
          />
        </td>
        <td className={styles.count}>
          <Text mono>{slot.count}</Text>
          <Text tone="muted" size="sm" className={styles.status}>
            primary · {slot.secondary_count} secondary
          </Text>
          {slot.status === "under" || slot.status === "over" ? (
            <Text tone="warn" size="sm" className={styles.status}>
              {Math.abs(slot.delta)} {slot.status}
            </Text>
          ) : slot.status === "ok" ? (
            <Text tone="muted" size="sm" className={styles.status}>
              on target
            </Text>
          ) : null}
        </td>
        <td className={styles.actions}>
          <Button size="sm" variant="reject" onClick={() => setConfirming(true)} aria-label={`Delete the slot ${slot.name}`}>
            Delete
          </Button>
        </td>
      </tr>
      {confirming ? (
        <tr>
          <td colSpan={5} className={styles.confirm}>
            <ConfirmStrip
              confirmLabel="Delete slot"
              onCancel={() => setConfirming(false)}
              onConfirm={() => onDelete(slot.slot_id)}
            >
              Delete the slot <strong>{slot.name}</strong>?{" "}
              Cards stay in the deck and on the maybe list. Only this purpose is removed; their other assignments stay.
            </ConfirmStrip>
          </td>
        </tr>
      ) : null}
    </>
  );
}

