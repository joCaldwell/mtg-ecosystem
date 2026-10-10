import type { GroupStatus } from "../../../domain/grouping.ts";
import styles from "./GroupHeader.module.css";

export interface GroupHeaderProps {
  title: string;
  /** Omitted for the commander and companion groups, which are not counted. */
  count?: number;
  /** Formatted target, e.g. "8–12". */
  target?: string | null;
  status?: GroupStatus;
  /** How far outside the target; negative means under. */
  delta?: number;
  /** The count if everything pending were accepted. Only passed while reviewing. */
  projected?: number;
}

function describe(status: GroupStatus, delta: number, target: string | null | undefined): string | undefined {
  if (!target) return undefined;
  if (status === "under") return `${-delta} short of the target of ${target}`;
  if (status === "over") return `${delta} over the target of ${target}`;
  if (status === "ok") return `Within the target of ${target}`;
  return undefined;
}

export function GroupHeader({ title, count, target, status = "untargeted", delta = 0, projected }: GroupHeaderProps) {
  return (
    <h3 className={styles.head}>
      <span className={styles.title}>{title}</span>
      {count != null && (
        <span className={`${styles.count} ${styles[status]}`} title={describe(status, delta, target)}>
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
    </h3>
  );
}
