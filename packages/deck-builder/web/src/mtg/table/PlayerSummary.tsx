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
}: {
  name: string;
  life: number;
  turn?: number;
  status?: string;
  onLifeChange?: (life: number) => void;
  actions?: ReactNode;
}) {
  return (
    <div className={styles.summary}>
      <div className={styles.identity}>
        <strong>{name}</strong>
        {status && <span>{status}</span>}
      </div>
      <span>Life</span>
      {onLifeChange ? (
        <CounterControl
          label={`${name} life`}
          value={life}
          onChange={onLifeChange}
        />
      ) : (
        <span className={styles.number}>{life}</span>
      )}
      {turn !== undefined && <span className={styles.number}>Turn {turn}</span>}
      {actions}
    </div>
  );
}
