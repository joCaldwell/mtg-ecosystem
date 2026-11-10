import type { ReactNode } from "react";
import { CounterControl } from "../../ui/index.ts";
import styles from "./PlayerSummary.module.css";
export function PlayerSummary({
  name,
  life,
  turn,
  status,
  onLifeChange,
  actions,
  showName = true,
}: {
  name: string;
  /** A solo table has one player; the name is still the controls' label. */
  showName?: boolean;
  life: number;
  turn?: number;
  status?: string;
  onLifeChange?: (life: number) => void;
  actions?: ReactNode;
}) {
  return (
    <div className={styles.summary}>
      {(showName || status) && (
        <div className={styles.identity}>
          {showName && <strong>{name}</strong>}
          {status && <span>{status}</span>}
        </div>
      )}
      {turn !== undefined && (
        <span className={styles.stat}>
          <span className={styles.label}>Turn</span>
          <span className={styles.number}>{turn}</span>
        </span>
      )}
      <span className={styles.stat}>
        <span className={styles.label}>Life</span>
        {onLifeChange ? (
          <CounterControl
            label={`${name} life`}
            value={life}
            onChange={onLifeChange}
          />
        ) : (
          <span className={styles.number}>{life}</span>
        )}
      </span>
      {actions}
    </div>
  );
}
