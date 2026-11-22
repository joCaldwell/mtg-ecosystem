import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import type { SlotDelta } from "../../../api/client.ts";
import { Icon, IconButton, Text, TextField } from "../../../ui/index.ts";
import { ConfirmStrip } from "./ConfirmStrip.tsx";
import { SlotTargetField } from "./SlotTargetField.tsx";
import { parseTargets, targetText } from "./targets.ts";
import styles from "./SlotsTable.module.css";

export interface SlotPatch {
  name?: string;
  target_min?: number | null;
  target_max?: number | null;
}

interface Props {
  slot: SlotDelta;
  nameId: string;
  busy: boolean;
  /** Resolves false when the server refused, so the typed text stays for fixing. */
  onSave: (slotId: number, patch: SlotPatch) => Promise<boolean>;
  onValidation: (message: string) => void;
  /** An edit that was refused and then taken back leaves no error behind. */
  onRevert: () => void;
  onDelete: (slotId: number) => void;
}

export function SlotRow({ slot, nameId, busy, onSave, onValidation, onRevert, onDelete }: Props) {
  const [name, setName] = useState(slot.name);
  const [min, setMin] = useState(targetText(slot.target_min));
  const [max, setMax] = useState(targetText(slot.target_max));
  const [confirming, setConfirming] = useState(false);
  const [checked, setChecked] = useState(false);
  const minField = useRef<HTMLInputElement>(null);
  const maxField = useRef<HTMLInputElement>(null);
  const deleteButton = useRef<HTMLButtonElement>(null);
  const targets = parseTargets(min, max);

  // Follow the server: after a save, or when an agent changes the slot.
  useEffect(() => setName(slot.name), [slot.name]);
  useEffect(() => setMin(targetText(slot.target_min)), [slot.target_min]);
  useEffect(() => setMax(targetText(slot.target_max)), [slot.target_max]);

  // Enter and then blur both commit; the second must not repeat the request.
  const saving = useRef(false);

  async function commit(focusError = false) {
    if (saving.current) return;
    const patch: SlotPatch = {};
    if (name.trim() !== slot.name) patch.name = name.trim();
    if (!targets.ok) {
      setChecked(true);
      if (focusError) requestAnimationFrame(() => (targets.minError ? minField : maxField).current?.focus());
      else if (!checked) {
        onValidation(targets.minError
          ? `Minimum for ${slot.name}. ${targets.minError}`
          : `Maximum for ${slot.name}. ${targets.maxError}`);
      }
      return;
    }
    if (targets.min !== slot.target_min) patch.target_min = targets.min;
    if (targets.max !== slot.target_max) patch.target_max = targets.max;
    if (Object.keys(patch).length === 0) return revert();
    saving.current = true;
    try {
      await onSave(slot.slot_id, patch);
    } finally {
      saving.current = false;
    }
  }

  function revert() {
    setChecked(false);
    onValidation("");
    setName(slot.name);
    setMin(targetText(slot.target_min));
    setMax(targetText(slot.target_max));
  }

  const dirty =
    name !== slot.name || min !== targetText(slot.target_min) || max !== targetText(slot.target_max);

  function editTargets(nextMin: string, nextMax: string) {
    setMin(nextMin);
    setMax(nextMax);
    if (checked && parseTargets(nextMin, nextMax).ok) {
      setChecked(false);
      onValidation("");
    }
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.nativeEvent.isComposing || e.nativeEvent.keyCode === 229) return;
    if (e.key === "Enter") {
      e.preventDefault();
      void commit(true);
    } else if (e.key === "Escape" && dirty) {
      // Undo the edit first; a second Escape closes the dialog.
      e.preventDefault();
      e.stopPropagation();
      if (saving.current) return;
      revert();
      onRevert();
    }
  }

  return (
    <>
      <tr>
        <td>
          <TextField
            id={nameId}
            value={name}
            onChange={(e) => setName(e.target.value)}
            onBlur={() => void commit()}
            onKeyDown={onKeyDown}
            aria-label={`Name of the slot ${slot.name}`}
          />
        </td>
        <td>
          <SlotTargetField
            inputRef={minField}
            error={checked && !targets.ok ? targets.minError : undefined}
            value={min}
            onChange={(e) => editTargets(e.target.value, max)}
            onBlur={() => void commit()}
            onKeyDown={onKeyDown}
            aria-label={`Minimum for ${slot.name}`}
          />
        </td>
        <td>
          <SlotTargetField
            inputRef={maxField}
            error={checked && !targets.ok ? targets.maxError : undefined}
            value={max}
            onChange={(e) => editTargets(min, e.target.value)}
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
            <Text tone="add" size="sm" className={`${styles.status} ${styles.onTarget}`}>
              <Icon name="check" size={12} /> on target
            </Text>
          ) : null}
        </td>
        <td className={styles.actions}>
          <IconButton ref={deleteButton} tone="danger" aria-disabled={busy || undefined} onClick={() => { if (!busy) setConfirming(true); }} label={`Delete ${slot.name}`}>
            <Icon name="trash" />
          </IconButton>
        </td>
      </tr>
      {confirming ? (
        <tr>
          <td colSpan={5} className={styles.confirm}>
            <ConfirmStrip
              confirmLabel="Delete slot"
              busy={busy}
              onCancel={() => { setConfirming(false); deleteButton.current?.focus(); }}
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

