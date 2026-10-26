// One decision (spec §8.1): a swap or a group is decided together, so it is one
// card with one pair of buttons, however many cards it names.

import { useEffect, useRef } from "react";
import type { ReviewUnit } from "../../../domain/review.ts";
import { cx } from "../../../ui/internal.ts";
import { DecisionControls } from "../../decisions/index.ts";
import { useDeckUi } from "../DeckUi.tsx";
import type { Review } from "../useReview.tsx";
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

  // Keyboard review moves the cursor through a list longer than the panel.
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (here) root.current?.scrollIntoView({ block: "nearest" });
  }, [here]);

  return (
    <div ref={root} className={cx(styles.unit, here && styles.current)}>
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
          onAccept={() => void review.accept(unit)}
          onStartReject={() => {
            ui.setCursor(index);
            ui.setRejecting(unit.key);
          }}
          onReject={(reason) => void review.reject(unit, reason)}
          onCancelReject={() => ui.setRejecting(null)}
          showKeys={here}
        />
      </div>
    </div>
  );
}
