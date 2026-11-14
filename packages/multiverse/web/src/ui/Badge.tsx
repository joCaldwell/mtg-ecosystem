import type { HTMLAttributes } from "react";
import { cx } from "./internal.ts";
import styles from "./Badge.module.css";

export interface BadgeProps extends HTMLAttributes<HTMLSpanElement> {
  tone?: "neutral" | "agent" | "warn";
}

/** A small count pill. */
export function Badge({ tone = "neutral", className, ...rest }: BadgeProps) {
  return <span className={cx(styles.badge, styles[tone], className)} {...rest} />;
}
