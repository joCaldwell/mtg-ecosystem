import type { GroupStatus } from "../../../domain/grouping.ts";
import { type GroupCost, groupCostCaption } from "../../../domain/price.ts";
import { Price } from "../../../mtg/index.ts";
import styles from "./GroupHeader.module.css";

export interface GroupHeaderProps {
  title: string;
  /** Omitted for the commander and companion groups, which are not counted. */
  count?: number;
  primaryCount?: number;
  secondaryCount?: number;
  /** Formatted target, e.g. "8–12". */
  target?: string | null;
  status?: GroupStatus;
  /** How far outside the target; negative means under. */
  delta?: number;
  /** The count if everything pending were accepted. Only passed while reviewing. */
  projected?: number;
  /** What the group costs (spec §4.4). Drawn over the rows' price column. */
  cost?: GroupCost;
}

function describe(status: GroupStatus, delta: number, target: string | null | undefined): string | undefined {
  if (!target) return undefined;
  if (status === "under") return `${-delta} short of the target of ${target}`;
  if (status === "over") return `${delta} over the target of ${target}`;
  if (status === "ok") return `Within the target of ${target}`;
  return undefined;
}

export function GroupHeader({ title, count, primaryCount, secondaryCount, target, status = "untargeted", delta = 0, projected, cost }: GroupHeaderProps) {
  // A total of nothing priced would read as "free".
  const total = cost?.priced ? cost : null;
  return (
    <h3 className={styles.head}>
      <span className={styles.title}>{title}</span>
      <span className={styles.spacer} />
      {count != null && (
        <span className={`${styles.count} ${styles[status]}`} title={[describe(status, delta, target), primaryCount == null ? null : `${primaryCount} primary + ${secondaryCount ?? 0} secondary`].filter(Boolean).join("; ")}>
          {count}
          {projected != null && (
            <span className={styles.projected} title="After the pending changes">
              {" → "}
              {projected}
            </span>
          )}
          {target ? ` / ${target}` : ""}
        </span>
      )}
      {total && (
        <span className={styles.cost}>
          <Price currency={total.currency} cents={total.cents} column title={groupCostCaption(total)} />
        </span>
      )}
    </h3>
  );
}
