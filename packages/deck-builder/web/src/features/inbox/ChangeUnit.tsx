import type { ReviewUnit } from "../../domain/review.ts";
import type { DeckState } from "../../api/client.ts";
import { CardRef, ManaCost } from "../../mtg/index.ts";
import { Box, Chip, Text } from "../../ui/index.ts";
import styles from "./ChangeUnit.module.css";

const LABEL: Record<ReviewUnit["kind"], string> = {
  swap: "Swap · decided together",
  group: "Group · decided together",
  add: "Add",
  cut: "Cut",
};

/** The name of the slot a card sits in now, for a cut; the deck state is the only place that says. */
function slotOf(state: DeckState | null, oracleId: string): string | null {
  const card = state?.cards.find((c) => c.oracle_id === oracleId);
  if (!card || card.slot_id == null) return null;
  return state?.slots.find((s) => s.id === card.slot_id)?.name ?? null;
}

/** One decision the owner will make in the deck: its items, read-only. */
export function ChangeUnit({ unit, state }: { unit: ReviewUnit; state: DeckState | null }) {
  return (
    <Box padding="sm" className={styles.unit}>
      {unit.items.map((item) => {
        const slot = item.action === "add" ? (item.slot_name ?? "Unfiled") : slotOf(state, item.oracle_id);
        return (
          <div key={item.id} className={styles.item}>
            <span className={item.action === "add" ? styles.add : styles.cut} aria-label={item.action}>
              {item.action === "add" ? "+" : "−"}
            </span>
            <span className={styles.name}>
              <CardRef name={item.card_name} card={{ ...item, name: item.card_name }} />
            </span>
            <ManaCost cost={item.mana_cost} />
            {!!item.to_maybe && <Chip>to maybe list</Chip>}
            {slot && <Chip>{slot}</Chip>}
          </div>
        );
      })}
      <Text tone="faint" size="xs" className={styles.kind}>
        {LABEL[unit.kind]}
      </Text>
    </Box>
  );
}
