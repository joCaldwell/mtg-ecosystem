// The Checks tab (spec §9). Computed on request from the current list and
// never stored, so there is no report to go stale. Everything numeric is the
// server's (`state.checks`, `state.computed`); this arranges it.

import { primaryType, TYPE_GROUPS } from "../../../domain/cards.ts";
import { CardRef, ColorIdentity } from "../../../mtg/index.ts";
import { Button, Chip, Eyebrow, KeyValue, Text } from "../../../ui/index.ts";
import { cx } from "../../../ui/internal.ts";
import type { Finding } from "../../../api/client.ts";
import { useDeckList } from "../DeckStore.tsx";
import { useDeckUi } from "../DeckUi.tsx";
import styles from "./ChecksPanel.module.css";

const CURVE_BUCKETS = ["0", "1", "2", "3", "4", "5", "6", "7+"];
const COLORS = ["W", "U", "B", "R", "G"] as const;
/** Tallest curve bar, in pixels; the bars are scaled to the biggest bucket. */
const CURVE_HEIGHT = 52;

/** Reads the list through useDeckList, so a shared deck and the phone view
 *  draw the same panel; `checks` come with whichever composite was loaded. */
export function ChecksPanel({ checks }: { checks: Finding[] }) {
  const { state, readOnly } = useDeckList();
  const ui = useDeckUi();
  const { computed, cards, slots } = state;

  const offTarget = computed.slot_deltas.filter((d) => d.status === "under" || d.status === "over");
  const hasTargets = computed.slot_deltas.some((d) => d.status !== "untargeted");

  const curveTop = Math.max(1, ...CURVE_BUCKETS.map((b) => computed.curve[b] ?? 0));
  const pipTotal = COLORS.reduce((n, c) => n + (computed.pips[c] ?? 0), 0);
  const pipColors = COLORS.filter((c) => (computed.pips[c] ?? 0) > 0);

  // An arrangement of the list, not a recount of a server number: the server
  // keeps no per-type totals.
  const types = new Map<string, number>();
  for (const c of cards) if (!c.maybe) types.set(primaryType(c.type_line), (types.get(primaryType(c.type_line)) ?? 0) + c.quantity);
  const typeOrder = [...TYPE_GROUPS, "Other"].filter((t) => types.has(t));

  return (
    <>
      <section className={styles.section}>
        <Eyebrow>Findings · {checks.length}</Eyebrow>
        {checks.length === 0 ? (
          <Text tone="muted" size="sm" as="p">
            No problems found.
          </Text>
        ) : (
          checks.map((f) => (
            <div key={f.key} className={cx(styles.finding, f.severity === "error" ? styles.error : styles.warn)}>
              <span className={styles.glyph} aria-hidden="true">
                {f.severity === "error" ? "✗" : "!"}
              </span>
              <div>
                <div className={styles.title}>
                  <span className={styles.srOnly}>{f.severity === "error" ? "Error: " : "Warning: "}</span>
                  {f.title}
                </div>
                <div className={styles.detail}>
                  {f.detail}
                  {f.card_name && (
                    <>
                      {" "}
                      <CardRef name={f.card_name} />
                    </>
                  )}
                </div>
              </div>
            </div>
          ))
        )}
      </section>

      <section className={styles.section}>
        <Eyebrow>Slot targets</Eyebrow>
        {offTarget.length > 0 && (
          <Text tone="muted" size="sm" as="p">
            {offTarget.length === 1 ? "One slot is" : `${offTarget.length} slots are`} outside target; each is listed above.
          </Text>
        )}
        {offTarget.length === 0 &&
          (hasTargets ? (
            <Text tone="muted" size="sm" as="p">
              Every slot with a target is inside it.
            </Text>
          ) : (
            <div className={styles.none}>
              <Text tone="muted" size="sm" as="p">
                {slots.length
                  ? "No slot has a target. Set one, such as 8 to 10 for ramp, and the list shows when a slot drifts."
                  : "This deck has no slots. Slots group cards by job and can carry a target."}
              </Text>
              {!readOnly && (
                <Button size="sm" onClick={() => ui.openDialog("slots")}>
                  Edit slots
                </Button>
              )}
            </div>
          ))}
        <Text tone="faint" size="xs" as="p">
          {readOnly ? "Targets are the owner's and soft." : "Targets are yours and soft. Nothing here blocks an edit."}
        </Text>
      </section>

      <section className={styles.section}>
        <Eyebrow>Numbers</Eyebrow>
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

      <section className={styles.section}>
        <Eyebrow>Colour · share of coloured symbols in costs</Eyebrow>
        {pipColors.length === 0 ? (
          <Text tone="muted" size="sm" as="p">
            No coloured symbols in the list.
          </Text>
        ) : (
          <div className={styles.pips}>
            {pipColors.map((c) => {
              const n = computed.pips[c] ?? 0;
              const share = Math.round((n / pipTotal) * 100);
              return (
                <div key={c} className={styles.pip}>
                  <ColorIdentity colors={c} />
                  <div className={styles.track}>
                    <i className={cx(styles.fill, styles[c])} style={{ width: `${share}%` }} />
                  </div>
                  <span className={styles.share}>
                    {share}% · {n}
                  </span>
                </div>
              );
            })}
          </div>
        )}
      </section>

      {typeOrder.length > 0 && (
        <section className={styles.section}>
          <Eyebrow>Types</Eyebrow>
          <div className={styles.types}>
            {typeOrder.map((t) => (
              <Chip key={t}>
                {t} <b className={styles.typeCount}>{types.get(t)}</b>
              </Chip>
            ))}
          </div>
        </section>
      )}
    </>
  );
}
