import { formatPrice } from "../../../domain/price.ts";
// What the deck would be if everything still open were accepted (spec §8).
// The numbers are the server's (`computed.pending`), so they stay right after
// every ruling without the client recounting anything.

import { formatTarget } from "../../../domain/grouping.ts";
import { Box, Eyebrow, Text } from "../../../ui/index.ts";
import { useDeckState } from "../DeckStore.tsx";
import styles from "./ReviewDelta.module.css";

type Tone = "ok" | "warn" | "plain";

interface DeltaRow {
  key: string;
  label: string;
  from: number;
  to: number;
  target: string;
  tone: Tone;
}

export function ReviewDelta() {
  const { computed, slots } = useDeckState();
  const { pending } = computed;

  const rows: DeltaRow[] = [
    {
      key: "cards",
      label: "Cards",
      from: computed.card_count,
      to: pending.projected_count,
      target: "100",
      tone: pending.projected_count === 100 ? "ok" : "warn",
    },
  ];
  for (const slot of slots) {
    const change = pending.by_slot[slot.id] ?? 0;
    if (change === 0) continue;
    const from = computed.slot_deltas.find((d) => d.slot_id === slot.id)?.count ?? 0;
    const to = from + change;
    const target = formatTarget(slot.target_min, slot.target_max);
    const outside = (slot.target_min != null && to < slot.target_min) || (slot.target_max != null && to > slot.target_max);
    rows.push({
      key: `slot-${slot.id}`,
      label: slot.name,
      from,
      to,
      target: target ?? "",
      tone: target == null ? "plain" : outside ? "warn" : "ok",
    });
  }

  return (
    <Box>
      <Eyebrow>If you accept everything still open</Eyebrow>
      <div className={styles.delta}>
        {rows.map((r) => (
          <div key={r.key} className={styles.row}>
            <span className={styles.label}>{r.label}</span>
            <Text tone="muted" mono>
              {r.from} →
            </Text>
            <span className={styles.to}>
              <Text mono tone={r.tone === "ok" ? "add" : r.tone === "warn" ? "warn" : "default"}>
                {r.to}
              </Text>
              {r.target && (
                <Text mono tone="faint">
                  {" "}
                  / {r.target}
                </Text>
              )}
            </span>
          </div>
        ))}
      </div>
      {computed.cost.as_of && <Text size="sm" tone="muted">Estimated cost: {formatPrice(computed.cost.cents, computed.cost.currency)} → {formatPrice(computed.pending.projected_cost.cents, computed.cost.currency)} · {computed.pending.projected_cost.unpriced} unpriced cards</Text>}
      <Text tone="faint" size="xs" as="p">
        Computed by the app from the current list, so it stays right as you rule.
      </Text>
    </Box>
  );
}
