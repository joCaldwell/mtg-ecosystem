// The Checks tab (spec §9). Computed on request from the current list and
// never stored, so there is no report to go stale. Everything numeric is the
// server's (`state.checks`, `state.computed`); this arranges it.

import { CardRef } from "../../../mtg/index.ts";
import { Button, Eyebrow, Text } from "../../../ui/index.ts";
import { cx } from "../../../ui/internal.ts";
import type { Finding } from "../../../api/client.ts";
import { useDeckList } from "../DeckListStore.tsx";
import { useDeckUi } from "../DeckUi.tsx";
import styles from "./ChecksPanel.module.css";

/** Reads the list through useDeckList, so a shared deck and the phone view
 *  draw the same panel; `checks` come with whichever composite was loaded. */
export function ChecksPanel({ checks }: { checks: Finding[] }) {
  const { state, readOnly, viewer } = useDeckList();
  const ui = useDeckUi();
  const { computed, slots } = state;

  const offTarget = computed.slot_deltas.filter((d) => d.status === "under" || d.status === "over");
  const hasTargets = computed.slot_deltas.some((d) => d.status !== "untargeted");

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
                  <span className="visually-hidden">{f.severity === "error" ? "Error: " : "Warning: "}</span>
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

      {!(viewer && slots.length === 0) && (
      <section className={styles.section}>
          <Eyebrow>Slot targets</Eyebrow>
          {offTarget.length > 0 && (
            <Text tone="muted" size="sm" as="p">
              {offTarget.length === 1 ? "One slot is" : `${offTarget.length} slots are`} outside target.
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
        </section>
      )}

    </>
  );
}
