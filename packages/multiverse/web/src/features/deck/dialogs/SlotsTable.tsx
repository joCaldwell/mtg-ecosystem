import { useId, useRef, useState, type FormEvent } from "react";
import { api } from "../../../api/client.ts";
import { Button, EmptyState, ErrorNote, Table, Text, TextField } from "../../../ui/index.ts";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import { restoreFocusAfterRemoval } from "./ConfirmStrip.tsx";
import { SlotRow, type SlotPatch } from "./SlotRow.tsx";
import { SlotTargetField } from "./SlotTargetField.tsx";
import { parseTargets } from "./targets.ts";
import { useAttempt } from "./useAttempt.ts";
import styles from "./SlotsTable.module.css";

/** The deck's slots, each editable in place, and a row to add another. */
export function SlotsTable() {
  const { deckId } = useDeck();
  const { computed } = useDeckState();
  const { error, setError, busy, mutate } = useAttempt();
  const formId = useId();
  const [name, setName] = useState("");
  const [min, setMin] = useState("");
  const [max, setMax] = useState("");
  const [checked, setChecked] = useState(false);
  const [validationStatus, setValidationStatus] = useState("");
  const addField = useRef<HTMLInputElement>(null);
  const minField = useRef<HTMLInputElement>(null);
  const maxField = useRef<HTMLInputElement>(null);
  const nameId = (id: number) => `${formId}-slot-${id}`;
  const targets = parseTargets(min, max);

  async function save(slotId: number, patch: SlotPatch): Promise<boolean> {
    const result = await mutate(() =>
      api.updateSlot(deckId, slotId, patch),
    );
    return result != null;
  }

  async function remove(slotId: number) {
    if (busy) return;
    const from = document.activeElement;
    const slots = computed.slot_deltas;
    const index = slots.findIndex(slot => slot.slot_id === slotId);
    const next = slots[index + 1] ?? slots[index - 1];
    if (await mutate(() => api.deleteSlot(deckId, slotId))) {
      restoreFocusAfterRemoval(from, () => (next && document.getElementById(nameId(next.slot_id))) || addField.current);
    }
  }

  async function add(e: FormEvent) {
    e.preventDefault();
    if (busy || !name.trim()) return;
    if (!targets.ok) {
      setChecked(true);
      requestAnimationFrame(() => (targets.minError ? minField : maxField).current?.focus());
      return;
    }
    const result = await mutate(() => api.createSlot(deckId, name.trim(), targets.min, targets.max));
    if (!result) return;
    setName("");
    setMin("");
    setMax("");
    setChecked(false);
  }

  return (
    <>
      <Text role="status" className="visually-hidden">{validationStatus}</Text>
      {error ? <ErrorNote>{error}</ErrorNote> : null}
      <Table className={styles.table}>
        <thead>
          <tr>
            <th rowSpan={2} scope="col">Name</th>
            <th colSpan={2} scope="colgroup" className={styles.targetHint}>Primary only · blank = no limit</th>
            <th rowSpan={2} scope="col">Cards</th>
            <th rowSpan={2} scope="col">
              <span className="visually-hidden">Actions</span>
            </th>
          </tr>
          <tr>
            <th scope="col" className={styles.numHead}>Min</th>
            <th scope="col" className={styles.numHead}>Max</th>
          </tr>
        </thead>
        <tbody>
          {computed.slot_deltas.map((slot) => (
            <SlotRow
              key={slot.slot_id}
              slot={slot}
              nameId={nameId(slot.slot_id)}
              busy={busy}
              onSave={save}
              onValidation={setValidationStatus}
              onRevert={() => setError(null)}
              onDelete={remove}
            />
          ))}
          <tr className={styles.addRow}>
            <td>
              <TextField
                ref={addField}
                form={formId}
                name="name"
                required
                pattern={".*\\S.*"}
                disabled={busy}
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Add a slot"
                aria-label="Name of the new slot"
              />
            </td>
            <td>
              <SlotTargetField
                form={formId}
                inputRef={minField}
                error={checked && !targets.ok ? targets.minError : undefined}
                readOnly={busy}
                value={min}
                onChange={(e) => setMin(e.target.value)}
                aria-label="Minimum for the new slot"
              />
            </td>
            <td>
              <SlotTargetField
                form={formId}
                inputRef={maxField}
                error={checked && !targets.ok ? targets.maxError : undefined}
                readOnly={busy}
                value={max}
                onChange={(e) => setMax(e.target.value)}
                aria-label="Maximum for the new slot"
              />
            </td>
            <td />
            <td className={styles.actions}>
              <Button size="sm" variant="primary" type="submit" form={formId} disabled={busy}>
                Add
              </Button>
            </td>
          </tr>
        </tbody>
      </Table>
      <form id={formId} method="post" onSubmit={add} />
      {computed.slot_deltas.length === 0 ? (
        <EmptyState title="No slots yet">Name one above, such as ramp or removal, to start filing cards under it.</EmptyState>
      ) : null}
    </>
  );
}
