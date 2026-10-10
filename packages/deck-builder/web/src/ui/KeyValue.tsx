import { Fragment, type ReactNode } from "react";
import { cx } from "./internal.ts";
import styles from "./KeyValue.module.css";

export interface KeyValueProps {
  items: Array<[ReactNode, ReactNode]>;
  className?: string;
}

export function KeyValue({ items, className }: KeyValueProps) {
  return (
    <dl className={cx(styles.kv, className)}>
      {items.map(([k, v], i) => (
        <Fragment key={i}>
          <dt className={styles.key}>{k}</dt>
          <dd className={styles.value}>{v}</dd>
        </Fragment>
      ))}
    </dl>
  );
}
