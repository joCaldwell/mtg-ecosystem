// What accepting this one decision changes: the card count and each slot's
// primary count. It applies the same rules the server uses for the aggregate
// (`computed.pending.by_slot` in deck/service.ts) to this unit's items only.

import type { ReviewUnit } from "../../../domain/review.ts";
import { Text } from "../../../ui/index.ts";
import { useDeckState } from "../DeckStore.tsx";
import styles from "./DecisionImpact.module.css";

const signed = (n: number) => (n > 0 ? `+${n}` : n < 0 ? `−${-n}` : "±0");

export function DecisionImpact({ unit }: { unit: ReviewUnit }) {
  const { cards, slots, computed } = useDeckState();
  const known = new Set(slots.map((s) => s.id));
  let delta = 0;
  const bySlot = new Map<number, number>();
  for (const item of unit.items) {
    const adding = item.action === "add";
    delta += adding ? 1 : -1;
    const owned = cards.find((c) => c.oracle_id === item.oracle_id);
    // Targets count primary assignments; the companion is outside them.
    if (owned?.role === "companion") continue;
    const proposed = item.slot_id != null && known.has(item.slot_id) ? item.slot_id : null;
    const current = owned?.slot_id ?? null;
    const primary = adding ? (owned && !owned.maybe ? current : proposed ?? (owned?.maybe ? current : null)) : current;
    if (primary != null && known.has(primary)) bySlot.set(primary, (bySlot.get(primary) ?? 0) + (adding ? 1 : -1));
  }
  const parts = [...bySlot].filter(([, n]) => n !== 0).map(([id, n]) => {
    const slot = slots.find((s) => s.id === id)!;
    const to = (computed.slot_deltas.find((d) => d.slot_id === id)?.count ?? 0) + n;
    const outside = (slot.target_min != null && to < slot.target_min) || (slot.target_max != null && to > slot.target_max);
    return { id, name: slot.name, n, outside };
  });
  return (
    <p className={styles.impact}>
      <span className={styles.label}>This decision</span>
      <span><Text mono>{signed(delta)}</Text> {Math.abs(delta) === 1 ? "card" : "cards"}</span>
      {parts.map((p) => (
        <span key={p.id}>
          {p.name} <Text mono tone={p.outside ? "warn" : "default"}>{signed(p.n)}</Text>
        </span>
      ))}
    </p>
  );
}
