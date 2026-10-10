// The owner's own edits to a card already in this deck (or on its maybe list).
// They apply at once and are logged; nothing here is a proposal.

import { useEffect, useState, type ReactNode } from "react";
import { api, type DeckCard } from "../../../api/client.ts";
import { Checkbox, Chip, KeyValue, Select, Text, TextField } from "../../../ui/index.ts";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import styles from "./CardControls.module.css";

const ROLES = ["card", "commander", "companion"] as const;

export function CardControls({ card }: { card: DeckCard }) {
  const { slots, tags } = useDeckState();
  const { deckId, run } = useDeck();
  const [quantity, setQuantity] = useState(String(card.quantity));
  useEffect(() => setQuantity(String(card.quantity)), [card.quantity]);

  const patch = (change: Parameters<typeof api.updateCard>[2]) => run(() => api.updateCard(deckId, card.oracle_id, change));

  function toggleTag(tagId: number) {
    const next = card.tag_ids.includes(tagId) ? card.tag_ids.filter((t) => t !== tagId) : [...card.tag_ids, tagId];
    void patch({ tag_ids: next });
  }

  async function commitQuantity() {
    const n = Number(quantity.trim());
    if (!/^\d+$/.test(quantity.trim()) || n < 1) return setQuantity(String(card.quantity));
    setQuantity(String(n));
    if (n === card.quantity) return;
    if (!(await patch({ quantity: n }))) setQuantity(String(card.quantity));
  }

  const inDeck = !card.maybe;
  // Singleton format: a second copy only makes sense of a basic land, so the
  // field appears for those and for any stack the deck already has.
  const showQuantity = inDeck && (card.quantity > 1 || /\bBasic\b/.test(card.type_line));

  const rows: Array<[string, ReactNode]> = [
    [
      "Slot",
      <Select
        key="slot"
        aria-label="Slot"
        value={card.slot_id ?? ""}
        onChange={(e) => void patch({ slot_id: e.target.value === "" ? null : Number(e.target.value) })}
      >
        <option value="">Unfiled</option>
        {slots.map((s) => (
          <option key={s.id} value={s.id}>
            {s.name}
          </option>
        ))}
      </Select>,
    ],
    [
      "Tags",
      tags.length ? (
        <div key="tags" className={styles.tags}>
          {tags.map((t) => {
            const on = card.tag_ids.includes(t.id);
            return (
              <Chip key={t.id} dashed={!on} aria-pressed={on} onClick={() => toggleTag(t.id)}>
                {on && "✓ "}
                {t.name}
              </Chip>
            );
          })}
        </div>
      ) : (
        <Text key="tags" tone="faint">
          none defined for this deck
        </Text>
      ),
    ],
  ];
  if (inDeck) {
    rows.push(
      [
        "Paper",
        <Checkbox
          key="paper"
          checked={!!card.in_paper}
          onChange={(in_paper) => void patch({ in_paper })}
          label="In the paper deck"
        />,
      ],
      [
        "Role",
        <Select key="role" aria-label="Role" value={card.role} onChange={(e) => void patch({ role: e.target.value })}>
          {ROLES.map((r) => (
            <option key={r} value={r}>
              {r}
            </option>
          ))}
        </Select>,
      ],
    );
  }
  if (showQuantity)
    rows.push([
      "Copies",
      <TextField
        key="quantity"
        className={styles.quantity}
        inputMode="numeric"
        value={quantity}
        aria-label={`Copies of ${card.name}`}
        onChange={(e) => setQuantity(e.target.value)}
        onFocus={(e) => e.currentTarget.select()}
        onBlur={() => void commitQuantity()}
        onKeyDown={(e) => {
          if (e.key === "Enter") e.currentTarget.blur();
          if (e.key === "Escape") {
            e.stopPropagation();
            setQuantity(String(card.quantity));
          }
        }}
      />,
    ]);

  return <KeyValue items={rows} className={styles.kv} />;
}
