import type { HTMLAttributes } from "react";
import { cx } from "./internal.ts";
import styles from "./Eyebrow.module.css";

/** The small uppercase label above a group of content. */
export function Eyebrow({ as: Tag = "div", className, ...rest }: HTMLAttributes<HTMLElement> & { as?: "div" | "h2" | "h3" | "h4" }) {
  return <Tag className={cx(styles.eyebrow, className)} {...rest} />;
}
