// Bulk actions on the cards the owner has selected (spec §5.3). Each call
// returns the whole deck, so they run one after another and the last one's
// state is what the list shows; one failure stops the rest and leaves the
// selection in place for a retry.

import { useState } from "react";
import { api } from "../../../api/client.ts";
import { Button, ConfirmDialog, Menu, Text, useToast, type MenuItem } from "../../../ui/index.ts";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import { useDeckUi } from "../DeckUi.tsx";
import styles from "./SelectionBar.module.css";

type Patch = Parameters<typeof api.updateCard>[2];

const plural = (n: number) => (n === 1 ? "1 card" : `${n} cards`);

export function SelectionBar() {
  const state = useDeckState();
  const { deckId, run } = useDeck();
  const ui = useDeckUi();
  const { toast } = useToast();
  const [confirmCut, setConfirmCut] = useState(false);
  const [busy, setBusy] = useState(false);

  // A card an agent removed since it was selected is simply skipped.
  const cards = state.cards.filter((c) => ui.selected.includes(c.oracle_id));
  if (ui.selected.length === 0) return null;

  /** Apply `step` to each selected card in turn; resolves to how many succeeded. */
  async function each(step: (oracleId: string) => ReturnType<typeof run>, skip?: (oracleId: string) => boolean) {
    setBusy(true);
    let done = 0;
    for (const c of cards) {
      if (skip?.(c.oracle_id)) continue;
      if (!(await step(c.oracle_id))) {
        setBusy(false);
        toast(`Stopped after ${done} of ${cards.length}. The error is above the list.`);
        return null;
      }
      done++;
    }
    setBusy(false);
    ui.clearSelection();
    return done;
  }

  async function patchAll(patch: (oracleId: string) => Patch, summary: (n: number) => string) {
    const n = await each((id) => run(() => api.updateCard(deckId, id, patch(id))));
    if (n != null) toast(summary(n));
  }

  async function tagAll(tagId: number, tagName: string, remove = false) {
    const has = (id: string) => !!cards.find((c) => c.oracle_id === id)?.tag_ids.includes(tagId);
    const n = await each(
      (id) => {
        const tags = cards.find((c) => c.oracle_id === id)?.tag_ids ?? [];
        return run(() => api.updateCard(deckId, id, { tag_ids: remove ? tags.filter((tag) => tag !== tagId) : [...tags, tagId] }));
      },
      (id) => remove ? !has(id) : has(id),
    );
    if (n != null) toast(n === 0 ? `No tags changed.` : remove ? `Removed ${tagName} from ${plural(n)}.` : `Tagged ${plural(n)} ${tagName}.`);
  }

  async function cutAll() {
    setConfirmCut(false);
    const n = await each((id) => run(() => api.removeCard(deckId, id)));
    if (n != null) toast(`Cut ${plural(n)}. Each one is in History and can be undone.`);
  }

  const slotItems: MenuItem[] = [
    ...state.slots.map((s) => ({
      id: `slot-${s.id}`,
      label: s.name,
      onSelect: () => void patchAll(() => ({ slot_id: s.id }), (n) => `Filed ${plural(n)} into ${s.name}.`),
    })),
    {
      id: "unfiled",
      label: "Unfiled",
      onSelect: () => void patchAll(() => ({ slot_id: null }), (n) => `Moved ${plural(n)} out of their slots.`),
    },
  ];

  const tagItems: MenuItem[] = state.tags.length
    ? state.tags.flatMap((t) => [
        { id: `tag-${t.id}`, label: `Add ${t.name}`, onSelect: () => void tagAll(t.id, t.name), disabled: busy },
        { id: `untag-${t.id}`, label: `Remove ${t.name}`, onSelect: () => void tagAll(t.id, t.name, true), disabled: busy },
      ])
    : [{ id: "none", label: "No tags yet. Add them under Edit slots and tags.", onSelect: () => {}, disabled: true }];

  const paperItems: MenuItem[] = [
    {
      id: "in",
      label: "Mark in paper",
      onSelect: () => void patchAll(() => ({ in_paper: true }), (n) => `Marked ${plural(n)} in paper.`),
    },
    {
      id: "out",
      label: "Mark missing",
      onSelect: () => void patchAll(() => ({ in_paper: false }), (n) => `Marked ${plural(n)} not in paper.`),
    },
  ];

  return (
    <div className={styles.bar} role="toolbar" aria-label="Selected cards" aria-busy={busy}>
      <Text weight="bold" size="sm">
        {ui.selected.length} selected
      </Text>
      <Menu size="sm" label="File into slot" items={slotItems} />
      <Menu size="sm" label="Tag" items={tagItems} />
      <Button
        size="sm"
        disabled={busy}
        onClick={() => void patchAll(() => ({ maybe: true }), (n) => `Moved ${plural(n)} to the maybe list.`)}
      >
        To maybe
      </Button>
      <Menu size="sm" label="Paper" items={paperItems} />
      <Button
        size="sm"
        variant="reject"
        disabled={busy}
        onClick={() => (cards.length > 1 ? setConfirmCut(true) : void cutAll())}
      >
        Cut
      </Button>
      <Button size="sm" variant="ghost" kbd="Esc" onClick={ui.clearSelection}>
        Clear
      </Button>
      {confirmCut && (
        <ConfirmDialog
          title={`Cut ${plural(cards.length)}?`}
          confirmLabel={`Cut ${plural(cards.length)}`}
          tone="danger"
          onConfirm={() => void cutAll()}
          onClose={() => setConfirmCut(false)}
        >
          They leave the deck now and each one is logged, so any of them can be undone from History.
        </ConfirmDialog>
      )}
    </div>
  );
}
