import type { HTMLAttributes } from "react";
import { cx } from "./internal.ts";
import styles from "./Dot.module.css";

export interface DotProps extends HTMLAttributes<HTMLSpanElement> {
  tone?: "neutral" | "agent" | "warn" | "add" | "cut";
  /** A soft halo ring, for "live" or "waiting". */
  pulse?: boolean;
}

export function Dot({ tone = "neutral", pulse = false, className, ...rest }: DotProps) {
  return <span className={cx(styles.dot, styles[tone], pulse && styles.pulse, className)} {...rest} />;
}
