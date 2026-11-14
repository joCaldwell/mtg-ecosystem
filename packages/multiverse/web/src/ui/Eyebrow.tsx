import type { HTMLAttributes } from "react";
import { cx } from "./internal.ts";
import styles from "./Eyebrow.module.css";

/** The small uppercase label above a group of content. */
export function Eyebrow({ className, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cx(styles.eyebrow, className)} {...rest} />;
}
