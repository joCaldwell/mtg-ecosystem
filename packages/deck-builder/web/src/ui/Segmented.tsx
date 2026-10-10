import type { ReactNode } from "react";
import { cx } from "./internal.ts";
import styles from "./Segmented.module.css";

export interface SegmentedProps<T extends string> {
  options: Array<{ value: T; label: ReactNode; title?: string }>;
  value: T;
  onChange: (value: T) => void;
  /** Accessible name for the group. */
  label: string;
  className?: string;
}

export function Segmented<T extends string>({ options, value, onChange, label, className }: SegmentedProps<T>) {
  return (
    <div role="group" aria-label={label} className={cx(styles.seg, className)}>
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          title={o.title}
          aria-pressed={o.value === value}
          className={cx(styles.option, o.value === value && styles.on)}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}
