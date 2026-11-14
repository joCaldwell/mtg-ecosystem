import type { HTMLAttributes } from "react";
import { cx } from "./internal.ts";
import type { Gap } from "./Stack.tsx";
import styles from "./Row.module.css";

export interface RowProps extends HTMLAttributes<HTMLDivElement> {
  gap?: Gap;
  align?: "start" | "center" | "baseline" | "end";
  justify?: "start" | "between" | "end";
  wrap?: boolean;
}

/** Horizontal flex row. */
export function Row({ gap = 4, align = "center", justify = "start", wrap = false, className, ...rest }: RowProps) {
  return (
    <div
      className={cx(
        styles.row,
        styles[`gap${gap}`],
        styles[`align-${align}`],
        styles[`justify-${justify}`],
        wrap && styles.wrap,
        className,
      )}
      {...rest}
    />
  );
}
