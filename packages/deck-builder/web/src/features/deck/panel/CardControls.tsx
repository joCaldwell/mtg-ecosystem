// The owner's own edits to a card already in this deck (or on its maybe list).
// They apply at once and are logged; nothing here is a proposal.

import { useEffect, useState, type ReactNode } from "react";
import { api, type DeckCard } from "../../../api/client.ts";
import { Button, KeyValue, MultiSelect, Select, Text, TextField } from "../../../ui/index.ts";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import { useDeckUi } from "../DeckUi.tsx";
import styles from "./CardControls.module.css";

// The stored role says where the card is played from: the 99, the command
// zone, or outside the game as a companion. Named as boards, as Archidekt
// and Moxfield name them.
const BOARDS: Record<DeckCard["role"], string> = { card: "Main deck", commander: "Commander", companion: "Companion" };

/** `slots` and `rest` split the fields for a maybe card, whose slots apply when it is added. */
export function CardControls({ card, part = "all" }: { card: DeckCard; part?: "all" | "slots" | "rest" }) {
  const { slots, tags, deck } = useDeckState();
  const { deckId, run } = useDeck();
  const ui = useDeckUi();
  const [quantity, setQuantity] = useState(String(card.quantity));
  useEffect(() => setQuantity(String(card.quantity)), [card.quantity]);

  const patch = (change: Parameters<typeof api.updateCard>[2]) => run(() => api.updateCard(deckId, card.oracle_id, change));

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
      "Primary slot",
      <Select
        key="slot"
        aria-label="Primary slot"
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
      "Secondary slots",
      <MultiSelect
        key={`purposes-${card.oracle_id}`}
        label="Secondary slots"
        compact
        placeholder="Select purposes…"
        options={slots.filter(s => s.id !== card.slot_id).map(s => ({ value: String(s.id), label: s.name }))}
        value={card.secondary_slot_ids.map(String)}
        onChange={ids => patch({ secondary_slot_ids: ids.map(Number) })}
        emptyText="No other slots. Create purposes using Edit slots and tags."
      />,
    ],
    [
      "Tags",
      (
        <div key="tags" className={styles.tags}>
          <MultiSelect
            key={card.oracle_id}
            label="Card tags"
            placeholder="Select tags…"
            options={tags.map((tag) => ({ value: String(tag.id), label: tag.name }))}
            value={card.tag_ids.map(String)}
            onChange={(ids) => patch({ tag_ids: ids.map(Number) })}
            emptyText="No tags yet. Create one using Edit slots and tags."
          />
          {card.tag_ids.length === 0 && <Text tone="muted" size="sm">No tags assigned</Text>}
          {tags.length === 0 && <Button size="sm" onClick={() => ui.openDialog("slots")}>Create a tag</Button>}
        </div>
      ),
    ],
  ];
  if (inDeck) {
    rows.push(
      [
        "Board",
        <Select key="role" aria-label="Board" value={card.role} onChange={(e) => void patch({ role: e.target.value })}>
          {Object.entries(BOARDS).map(([role, label]) => (
            <option key={role} value={role}>
              {label}
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

  if (deck.category === "archived") return <KeyValue items={[
    ["Primary slot", slots.find(s => s.id === card.slot_id)?.name ?? "Unfiled"],
    ["Secondary slots", slots.filter(s => card.secondary_slot_ids.includes(s.id)).map(s => s.name).join(", ") || "None"],
    ["Tags", tags.filter(t => card.tag_ids.includes(t.id)).map(t => t.name).join(", ") || "None"],
    ["Board", BOARDS[card.role]], ["Copies", card.quantity],
  ]} className={styles.kv} />;
  const shown = part === "slots" ? rows.slice(0, 2) : part === "rest" ? rows.slice(2) : rows;
  return <KeyValue items={shown} className={styles.kv} />;
}
