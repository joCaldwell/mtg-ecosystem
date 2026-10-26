import { useId, useState, type FormEvent } from "react";
import { api } from "../../../api/client.ts";
import { Button, EmptyState, ErrorNote, Table, TextField } from "../../../ui/index.ts";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import { SlotRow, type SlotPatch } from "./SlotRow.tsx";
import { parseTarget, TARGET_ERROR } from "./targets.ts";
import { useAttempt } from "./useAttempt.ts";
import styles from "./SlotsTable.module.css";

/** The deck's slots, each editable in place, and a row to add another. */
export function SlotsTable() {
  const { deckId } = useDeck();
  const { computed } = useDeckState();
  const { error, setError, mutate } = useAttempt();
  const formId = useId();
  const [name, setName] = useState("");
  const [min, setMin] = useState("");
  const [max, setMax] = useState("");

  async function save(slotId: number, patch: SlotPatch): Promise<boolean> {
    const result = await mutate(() =>
      api.updateSlot(deckId, slotId, patch),
    );
    return result != null;
  }

  async function remove(slotId: number) {
    await mutate(() => api.deleteSlot(deckId, slotId));
  }

  async function add(e: FormEvent) {
    e.preventDefault();
    const lo = parseTarget(min);
    const hi = parseTarget(max);
    if (!lo.ok || !hi.ok) return setError(TARGET_ERROR);
    const result = await mutate(() => api.createSlot(deckId, name.trim(), lo.value, hi.value));
    if (!result) return;
    setName("");
    setMin("");
    setMax("");
  }

  return (
    <>
      {error ? <ErrorNote>{error}</ErrorNote> : null}
      <Table className={styles.table}>
        <thead>
          <tr>
            <th>Name</th>
            <th className={styles.numHead}>Min</th>
            <th className={styles.numHead}>Max</th>
            <th>Cards</th>
            <th>
              <span className="visually-hidden">Actions</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {computed.slot_deltas.map((slot) => (
            <SlotRow key={slot.slot_id} slot={slot} onSave={save} onInvalid={setError} onRevert={() => setError(null)} onDelete={remove} />
          ))}
          <tr className={styles.addRow}>
            <td>
              <TextField
                form={formId}
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Add a slot"
                aria-label="Name of the new slot"
              />
            </td>
            <td>
              <TextField
                form={formId}
                className={styles.num}
                inputMode="numeric"
                value={min}
                placeholder="none"
                onChange={(e) => setMin(e.target.value)}
                aria-label="Minimum for the new slot"
              />
            </td>
            <td>
              <TextField
                form={formId}
                className={styles.num}
                inputMode="numeric"
                value={max}
                placeholder="none"
                onChange={(e) => setMax(e.target.value)}
                aria-label="Maximum for the new slot"
              />
            </td>
            <td />
            <td className={styles.actions}>
              <Button size="sm" variant="primary" type="submit" form={formId} disabled={!name.trim()}>
                Add
              </Button>
            </td>
          </tr>
        </tbody>
      </Table>
      <form id={formId} onSubmit={add} />
      {computed.slot_deltas.length === 0 ? (
        <EmptyState title="No slots yet">Name one above, such as ramp or removal, to start filing cards under it.</EmptyState>
      ) : null}
    </>
  );
}
