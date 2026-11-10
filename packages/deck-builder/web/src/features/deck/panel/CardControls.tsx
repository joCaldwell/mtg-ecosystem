// The owner's own edits to a card already in this deck (or on its maybe list).
// They apply at once and are logged; nothing here is a proposal.

import type { ReactNode } from "react";
import { api, type DeckCard } from "../../../api/client.ts";
import { Button, KeyValue, MultiSelect, Segmented, Select } from "../../../ui/index.ts";
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
  // Singleton format: the field appears for a card that allows more than one
  // copy, and for any stack the deck already has, so it can be lowered.
  const showQuantity = inDeck && (card.quantity > 1 || card.copy_limit !== 1);
  // Keep an invalid current role available so it can always be moved back.
  const roles = ROLES.filter(role => role === "card" || role === card.role ||
    (role === "commander" ? card.can_be_commander : card.can_be_companion));
  const showBoard = inDeck && roles.length > 1;

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
  // Where the card is played from, apart from what it is there to do.
  const place = <div className={styles.place}>
    {showBoard && <Segmented
      label="Board"
      className={styles.board}
      options={roles.map(role => ({ value: role, label: BOARDS[role] }))}
      value={card.role}
      onChange={role => void patch({ role })}
    />}
    {showQuantity && <div className={styles.copies}><span>Copies</span><CopiesControl key={card.oracle_id} card={card} /></div>}
  </div>;

  if (deck.category === "archived") return (
    <div className={`${styles.form} ${styles.readOnly}`}>
      <KeyValue className={styles.kv} items={[
        ["Primary slot", slots.find(s => s.id === card.slot_id)?.name ?? "Unfiled"],
        ["Secondary slots", slots.filter(s => card.secondary_slot_ids.includes(s.id)).map(s => s.name).join(", ") || "None"],
        ["Tags", tags.filter(t => card.tag_ids.includes(t.id)).map(t => t.name).join(", ") || "None"],
      ]} />
      <div className={styles.place}>
        {showBoard && <div role="group" aria-label="Board">{BOARDS[card.role]}</div>}
        <div className={styles.copies}><span>Copies</span><span>{card.quantity}</span></div>
      </div>
    </div>
  );
  const shown = part === "slots" ? rows.slice(0, 2) : part === "rest" ? rows.slice(2) : rows;
  return (
    <div className={styles.form}>
      <KeyValue items={shown} className={styles.kv} />
      {part !== "slots" && (showBoard || showQuantity) && place}
    </div>
  );
}
