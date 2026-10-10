import type { HTMLAttributes } from "react";
import { cx } from "./internal.ts";
import styles from "./Box.module.css";

export interface BoxProps extends HTMLAttributes<HTMLDivElement> {
  tone?: "neutral" | "agent" | "warn";
  dashed?: boolean;
  padding?: "none" | "sm" | "md";
}

export function Box({ tone = "neutral", dashed = false, padding = "md", className, ...rest }: BoxProps) {
  return (
    <div
      className={cx(styles.box, styles[tone], dashed && styles.dashed, styles[`pad-${padding}`], className)}
      {...rest}
    />
  );
}
