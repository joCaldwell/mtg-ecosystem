import { cx } from "./internal.ts";
import styles from "./Meter.module.css";

export interface MeterProps {
  value: number;
  max: number;
  tone?: "neutral" | "agent" | "warn" | "add";
  className?: string;
  "aria-label"?: string;
}

export function Meter({ value, max, tone = "neutral", className, "aria-label": ariaLabel }: MeterProps) {
  const pct = max > 0 ? Math.min(100, Math.max(0, (value / max) * 100)) : 0;
  return (
    <div
      role="meter"
      aria-label={ariaLabel}
      aria-valuenow={value}
      aria-valuemin={0}
      aria-valuemax={max}
      className={cx(styles.meter, className)}
    >
      <i className={cx(styles.fill, styles[tone])} style={{ width: `${pct}%` }} />
    </div>
  );
}
