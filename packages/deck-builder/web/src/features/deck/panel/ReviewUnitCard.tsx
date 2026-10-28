// One decision (spec §8.1): a swap or a group is decided together, so it is one
// card with one pair of buttons, however many cards it names.

import { useEffect, useRef, useState } from "react";
import type { ReviewUnit } from "../../../domain/review.ts";
import { cx } from "../../../ui/internal.ts";
import { DecisionControls } from "../../decisions/index.ts";
import { useDeckUi } from "../DeckUi.tsx";
import type { Review } from "../useReview.tsx";
import { revealReviewUnit } from "../reviewArrival.ts";
import { ReviewItemLine } from "./ReviewItemLine.tsx";
import styles from "./ReviewUnitCard.module.css";

const KIND: Record<ReviewUnit["kind"], string> = {
  swap: "Swap · decided together",
  add: "Add",
  cut: "Cut",
  group: "Group · decided together",
};

export function ReviewUnitCard({ unit, index, review }: { unit: ReviewUnit; index: number; review: Review }) {
  const ui = useDeckUi();
  const here = ui.reviewing && ui.cursor === index;
  // The phone's optional reason (§11.2), sent with Accept or Reject.
  const [reason, setReason] = useState("");

  // Keyboard review moves the cursor through a list longer than the panel.
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = root.current;
    if (!here || !el) return;
    const reveal = () => revealReviewUnit(el);
    reveal();
    // History/reason context arrives asynchronously. Keep this decision's
    // controls visible when it, a preceding proposal, or the pane resizes.
    const observer = new ResizeObserver(reveal);
    const pane = el.closest('[data-panel-body]');
    observer.observe(el);
    if (pane) {
      observer.observe(pane);
      pane.querySelectorAll('[data-proposal]').forEach(proposal => observer.observe(proposal));
    }
    return () => observer.disconnect();
  }, [here]);

  return (
    <div ref={root} data-review-unit={unit.key} className={cx(styles.unit, here && styles.current)}>
      <button type="button" className={styles.head} onClick={() => ui.setCursor(index)}>
        {KIND[unit.kind]}
      </button>
      <div className={styles.items}>
        {unit.items.map((item) => (
          <ReviewItemLine key={item.id} item={item} />
        ))}
      </div>
      <div className={styles.foot}>
        <DecisionControls
          rejecting={ui.rejecting === unit.key}
          onAccept={(text) => void review.accept(unit, text, () => setReason(""))}
          onStartReject={() => {
            ui.setCursor(index);
            ui.setRejecting(unit.key);
          }}
          onReject={(text) => void review.reject(unit, text, () => setReason(""))}
          onCancelReject={() => ui.setRejecting(null)}
          showKeys={here}
          reason={{
            value: reason,
            onChange: setReason,
            label: `Reason for decision ${index + 1}, optional`,
          }}
        />
      </div>
    </div>
  );
}
