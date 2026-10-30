// One decision (spec §8.1): a swap or a group is decided together, so it is one
// card with one pair of buttons, however many cards it names. Only the focused
// decision carries the reason field; its siblings in the open proposal keep
// Accept and Reject, and Reject asks for a reason there.

import { useEffect, useRef } from "react";
import type { ReviewUnit } from "../../../domain/review.ts";
import { cx } from "../../../ui/internal.ts";
import { DecisionControls } from "../../decisions/index.ts";
import { useDeckUi } from "../DeckUi.tsx";
import type { Review } from "../useReview.tsx";
import { revealReviewUnit } from "../reviewArrival.ts";
import { DecisionImpact } from "./DecisionImpact.tsx";
import { ReviewItemLine } from "./ReviewItemLine.tsx";
import styles from "./ReviewUnitCard.module.css";

const KIND: Record<ReviewUnit["kind"], string> = {
  swap: "Swap · decided together",
  add: "Add",
  cut: "Cut",
  group: "Group · decided together",
};

export function ReviewUnitCard({ unit, index, review, focused, reason, onReason }: {
  unit: ReviewUnit; index: number; review: Review;
  /** The decision under the cursor, in or out of review mode. */
  focused: boolean;
  /** The optional reason (§11.2), sent with Accept or Reject; the panel keeps the draft. */
  reason: string; onReason: (text: string) => void;
}) {
  const ui = useDeckUi();
  const here = ui.reviewing && focused;

  // Moving the cursor opens this decision's proposal; keep it in view.
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = root.current;
    if (!focused || !el) return;
    const reveal = () => revealReviewUnit(el);
    reveal();
    if (!ui.reviewing) return;
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
  }, [focused, ui.reviewing]);

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
        <DecisionImpact unit={unit} />
        <DecisionControls
          rejecting={ui.rejecting === unit.key}
          onAccept={(text) => void review.accept(unit, text, () => onReason(""))}
          onStartReject={() => {
            ui.setCursor(index);
            ui.setRejecting(unit.key);
          }}
          onReject={(text) => void review.reject(unit, text, () => onReason(""))}
          onCancelReject={() => ui.setRejecting(null)}
          showKeys={here}
          reason={focused ? {
            value: reason,
            onChange: onReason,
            label: `Reason for decision ${index + 1}, optional`,
          } : undefined}
        />
      </div>
    </div>
  );
}
