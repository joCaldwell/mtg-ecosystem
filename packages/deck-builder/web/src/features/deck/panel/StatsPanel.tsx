// Statistics are distinct from validation findings. Both readers and editors
// arrange the same server-computed values here.
import { TYPE_GROUPS } from "../../../domain/cards.ts";
import { lazy, Suspense } from "react";
import { Chip, Eyebrow, KeyValue, Loading } from "../../../ui/index.ts";
import { useDeckList } from "../DeckListStore.tsx";
import styles from "./StatsPanel.module.css";

const CURVE_BUCKETS = ["0", "1", "2", "3", "4", "5", "6", "7+"];
// The comparison is secondary on public readers and only requested with Stats
// in the editor, so keep its detailed presentation out of the initial bundle.
const ManaComparison = lazy(() => import("./ManaComparison.tsx").then(module => ({ default: module.ManaComparison })));
const CURVE_HEIGHT = 52;

export function StatsPanel() {
  const { state } = useDeckList();
  const { computed } = state;
  const curveTop = Math.max(1, ...CURVE_BUCKETS.map(bucket => computed.curve[bucket] ?? 0));
  const typeOrder = [...TYPE_GROUPS, "Other"].filter(type => (computed.type_counts[type] ?? 0) > 0);
  return <>
      <section className={styles.section}>
        <Eyebrow>Deck totals</Eyebrow>
        <KeyValue
          items={[
            ["Cards", <span className={styles.num}>{computed.card_count} / 100</span>],
            ["Lands", <span className={styles.num}>{computed.land_count}</span>],
            [
              "Average mana value",
              <span className={styles.num}>
                {computed.average_mana_value == null ? "—" : computed.average_mana_value.toFixed(1)}
              </span>,
            ],
            ["Unfiled", <span className={styles.num}>{computed.unslotted_count}</span>],
            ["Maybe list", <span className={styles.num}>{computed.maybe_count}</span>],
          ]}
        />
      </section>

      <Suspense fallback={<Loading />}><ManaComparison mana={computed.mana} /></Suspense>

      <section className={styles.section}>
        <Eyebrow>Mana curve · nonland</Eyebrow>
        <div className={styles.curve} role="img" aria-label={`Mana curve: ${CURVE_BUCKETS.map((b) => `${computed.curve[b] ?? 0} at ${b}`).join(", ")}`}>
          {CURVE_BUCKETS.map((b) => {
            const n = computed.curve[b] ?? 0;
            return (
              <div key={b} className={styles.col}>
                <span className={styles.count}>{n}</span>
                <i className={styles.bar} style={{ height: Math.max(2, Math.round((n / curveTop) * CURVE_HEIGHT)) }} />
                <span className={styles.bucket}>{b}</span>
              </div>
            );
          })}
        </div>
      </section>

      {typeOrder.length > 0 && (
        <section className={styles.section}>
          <Eyebrow>Types</Eyebrow>
          <div className={styles.types}>
            {typeOrder.map((t) => (
              <Chip key={t}>
                {t} <b className={styles.typeCount}>{computed.type_counts[t]}</b>
              </Chip>
            ))}
          </div>
        </section>
      )}
  </>;
}
