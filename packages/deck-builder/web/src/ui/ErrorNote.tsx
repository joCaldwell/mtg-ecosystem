import type { HTMLAttributes } from "react";
import { cx } from "./internal.ts";
import styles from "./ErrorNote.module.css";

export function ErrorNote({ className, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return <div role="alert" className={cx(styles.error, className)} {...rest} />;
}
