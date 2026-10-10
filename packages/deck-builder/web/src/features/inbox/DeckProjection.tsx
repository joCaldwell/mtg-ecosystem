import type { DeckState } from "../../api/client.ts";
import { formatTarget } from "../../domain/grouping.ts";
import { Box, Eyebrow, Text } from "../../ui/index.ts";
import styles from "./DeckProjection.module.css";

interface Line {
  key: string;
  label: string;
  from: number;
  to: number;
  target: string | null;
  outside: boolean;
}

/** Slots the pending items touch, with the server's numbers; nothing is recounted here. */
function lines(state: DeckState): Line[] {
  const { computed } = state;
  const out: Line[] = [
    {
      key: "cards",
      label: "Cards",
      from: computed.card_count,
      to: computed.pending.projected_count,
      target: "100",
      outside: computed.pending.projected_count !== 100,
    },
  ];
  for (const slot of computed.slot_deltas) {
    const change = computed.pending.by_slot[slot.slot_id] ?? 0;
    if (change === 0) continue;
    const to = slot.count + change;
    out.push({
      key: `slot-${slot.slot_id}`,
      label: slot.name,
      from: slot.count,
      to,
      target: formatTarget(slot.target_min, slot.target_max),
      outside: (slot.target_min != null && to < slot.target_min) || (slot.target_max != null && to > slot.target_max),
    });
  }
  return out;
}

/** What the list becomes if everything pending on the deck were accepted (spec §8.4). */
export function DeckProjection({ state }: { state: DeckState }) {
  const rows = lines(state);
  const openProposals = state.proposals.length;
  return (
    <Box>
      <Eyebrow>What the deck becomes</Eyebrow>
      <div className={styles.grid}>
        {rows.map((row) => (
          <div key={row.key} className={styles.line}>
            <span>{row.label}</span>
            <Text mono tone="muted" className={styles.from}>
              {row.from} →
            </Text>
            <Text mono tone={row.outside ? "warn" : "default"}>
              {row.to}
              {row.target && <span className={styles.target}> / {row.target}</span>}
            </Text>
          </div>
        ))}
      </div>
      <Text tone="faint" size="xs">
        {openProposals > 1
          ? `Counts everything pending on this deck: ${openProposals} open proposals, not only this one.`
          : "Counts everything pending on this deck."}{" "}
        Amber means outside the target.
      </Text>
    </Box>
  );
}
