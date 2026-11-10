import type { GroupStatus } from "../../../domain/grouping.ts";
import { type GroupCost, groupCostCaption } from "../../../domain/price.ts";
import { Price } from "../../../mtg/index.ts";
import { Icon } from "../../../ui/index.ts";
import styles from "./GroupHeader.module.css";

export interface GroupHeaderProps {
  title: string;
  /** Board-level headings span the list above its card columns. */
  board?: boolean;
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
  /** While a filter hides rows: the copies still shown. The header then reads
   *  "2 of 14" (of the projected count while reviewing) and leaves out the
   *  target and cost, which describe the rows that are hidden. */
  matched?: number;
}

function describe(status: GroupStatus, delta: number, target: string | null | undefined): string | undefined {
  if (!target) return undefined;
  if (status === "under") return `${-delta} short of the target of ${target}`;
  if (status === "over") return `${delta} over the target of ${target}`;
  if (status === "ok") return `Within the target of ${target}`;
  return undefined;
}

/** The same signs as the slots dialog, so status never rests on colour alone:
 *  a tick on target, "!" outside it. The words are for assistive technology. */
function StatusSign({ status, delta, target }: { status: GroupStatus; delta: number; target: string | null | undefined }) {
  const said = describe(status, delta, target);
  if (!said) return null;
  return (
    <>
      <span className={styles.sign} aria-hidden="true">{status === "ok" ? <Icon name="check" size={12} /> : "!"}</span>
      <span className="visually-hidden">{said}: </span>
    </>
  );
}

export function GroupHeader({ board = false, title, count, primaryCount, secondaryCount, target, status = "untargeted", delta = 0, projected, cost, matched }: GroupHeaderProps) {
  // A total of nothing priced would read as "free".
  const total = matched == null && cost?.priced ? cost : null;
  return (
    <h2 className={board ? `${styles.head} ${styles.board}` : styles.head}>
      <span className={styles.title}>{title}</span>
      <span className={styles.spacer} />
      {count != null && matched != null && (
        <span className={styles.count}>{matched} of {projected ?? count}</span>
      )}
      {count != null && matched == null && (
        <span className={`${styles.count} ${styles[status]}`} title={[describe(status, delta, target), primaryCount == null ? null : `${primaryCount} primary + ${secondaryCount ?? 0} secondary`].filter(Boolean).join("; ")}>
          <StatusSign status={status} delta={delta} target={target} />
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
    </h2>
  );
}
