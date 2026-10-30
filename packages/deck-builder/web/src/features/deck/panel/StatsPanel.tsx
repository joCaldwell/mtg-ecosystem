// Statistics are distinct from validation findings. Both readers and editors
// arrange the same server-computed values here.
import { TYPE_GROUPS } from "../../../domain/cards.ts";
import { lazy, Suspense, type CSSProperties } from "react";
import { Eyebrow, Loading, Text } from "../../../ui/index.ts";
import { useDeckList } from "../DeckListStore.tsx";
import styles from "./StatsPanel.module.css";

const CURVE_BUCKETS = ["0", "1", "2", "3", "4", "5", "6", "7+"];
// The comparison is secondary on public readers and only requested with Stats
// in the editor, so keep its detailed presentation out of the initial bundle.
const ManaComparison = lazy(() => import("./ManaComparison.tsx").then(module => ({ default: module.ManaComparison })));

export function StatsPanel() {
  const { state, viewer } = useDeckList();
  const { computed } = state;
  const curveTop = Math.max(1, ...CURVE_BUCKETS.map(bucket => computed.curve[bucket] ?? 0));
  const typeOrder = [...TYPE_GROUPS, "Other"].filter(type => (computed.type_counts[type] ?? 0) > 0);
  return <div className={styles.stats}>
      <section className={styles.section}>
        <Eyebrow className={styles.label}>Deck totals</Eyebrow>
        <dl className={styles.totals}>
          <div><dt>Cards</dt><dd>{computed.card_count}<span className={styles.maximum}> / 100</span></dd></div>
          <div><dt>Lands</dt><dd>{computed.land_count}</dd></div>
          <div><dt>Avg. nonland MV</dt><dd>{computed.average_mana_value == null ? "—" : computed.average_mana_value.toFixed(1)}</dd></div>
        </dl>
        {!viewer && <Text size="md" tone="soft">{computed.unslotted_count} unfiled · {computed.maybe_count} on the maybe list</Text>}
      </section>

      <section className={styles.section}>
        <Eyebrow className={styles.label}>Mana curve</Eyebrow>
        <div className={styles.curve} role="img" aria-label={`Mana curve: ${CURVE_BUCKETS.map((b) => `${computed.curve[b] ?? 0} at ${b}`).join(", ")}`}>
          {CURVE_BUCKETS.map((b) => {
            const n = computed.curve[b] ?? 0;
            return (
              <div key={b} className={styles.col}>
                <span className={styles.count}>{n}</span>
                <i className={styles.bar} style={{ "--curve-share": n / curveTop } as CSSProperties} />
                <span className={styles.bucket}>{b}</span>
              </div>
            );
          })}
        </div>
        {!Object.values(computed.curve).some(Boolean) && <Text tone="muted" size="sm">No cards in this curve.</Text>}
      </section>

        <section className={styles.section}>
          <Eyebrow className={styles.label}>Card types</Eyebrow>
          {typeOrder.length ? <ul className={styles.types} aria-label="Card-type groups and copy counts">
            {typeOrder.map((t) => (
              <li key={t}>
                <span>{t}</span>
                <span className={styles.typeTrack} aria-hidden="true"><i style={{ width: `${100 * computed.type_counts[t] / computed.card_count}%` }} /></span>
                <b className={styles.typeCount}>{computed.type_counts[t]}</b>
              </li>
            ))}
          </ul> : <Text tone="muted" size="sm">No cards to group.</Text>}
        </section>

      {/* Card types read fastest, so they come before the colour analysis. */}
      <Suspense fallback={<Loading />}><ManaComparison mana={computed.mana} /></Suspense>
  </div>;
}
