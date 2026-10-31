// The owner's own edits to a card already in this deck (or on its maybe list).
// They apply at once and are logged; nothing here is a proposal.

import type { ReactNode } from "react";
import { api, type DeckCard } from "../../../api/client.ts";
import { Button, KeyValue, MultiSelect, Segmented, Select, Text } from "../../../ui/index.ts";
import { CopiesControl } from "./CopiesControl.tsx";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import { useDeckUi } from "../DeckUi.tsx";
import styles from "./CardControls.module.css";

// The stored role says where the card is played from: the 99, the command
// zone, or outside the game as a companion. Named as boards, as Archidekt
// and Moxfield name them.
const BOARDS: Record<DeckCard["role"], string> = { card: "Main deck", commander: "Commander", companion: "Companion" };
const ROLES: Array<DeckCard["role"]> = ["card", "commander", "companion"];

/** `slots` and `rest` split the fields for a maybe card, whose slots apply when it is added. */
export function CardControls({ card, part = "all" }: { card: DeckCard; part?: "all" | "slots" | "rest" }) {
  const { slots, tags, deck } = useDeckState();
  const { deckId, run } = useDeck();
  const ui = useDeckUi();

  const patch = (change: Parameters<typeof api.updateCard>[2]) => run(() => api.updateCard(deckId, card.oracle_id, change));

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
        options={[{ value: "", label: "Unfiled" }, ...slots.map((s) => ({ value: String(s.id), label: s.name }))]}
        value={String(card.slot_id ?? "")}
        onChange={(value) => void patch({ slot_id: value === "" ? null : Number(value) })}
      />,
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
          {tags.length === 0 && <Button size="sm" onClick={() => ui.openDialog("slots")}>Create a tag</Button>}
        </div>
      ),
    ],
  ];
  if (inDeck) {
    rows.push(
      [
        "Board",
        <Segmented
          key="role"
          label="Board"
          className={styles.board}
          options={ROLES.map((role) => ({ value: role, label: BOARDS[role] }))}
          value={card.role}
          onChange={(role) => void patch({ role })}
        />,
      ],
    );
  }
  if (showQuantity)
    rows.push(["Copies", <CopiesControl key={card.oracle_id} card={card} />]);

  if (deck.category === "archived") return <KeyValue items={[
    ["Primary slot", slots.find(s => s.id === card.slot_id)?.name ?? "Unfiled"],
    ["Secondary slots", slots.filter(s => card.secondary_slot_ids.includes(s.id)).map(s => s.name).join(", ") || "None"],
    ["Tags", tags.filter(t => card.tag_ids.includes(t.id)).map(t => t.name).join(", ") || "None"],
    ["Board", BOARDS[card.role]], ["Copies", card.quantity],
  ]} className={styles.kv} />;
  const shown = part === "slots" ? rows.slice(0, 2) : part === "rest" ? rows.slice(2) : rows;
  return <KeyValue items={shown} className={styles.kv} />;
}
