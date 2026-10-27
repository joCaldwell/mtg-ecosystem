import type { ReactNode } from "react";
import { cx } from "./internal.ts";
import styles from "./Page.module.css";

export interface PageProps {
  /** Usually a PageHeader. Stays put while the body scrolls. */
  header?: ReactNode;
  children: ReactNode;
  /** md is reading width, lg is wide content, full uses all the room. */
  width?: "md" | "lg" | "full";
  className?: string;
}

export function Page({ header, children, width = "lg", className }: PageProps) {
  return (
    <div className={cx(styles.page, styles[width], className)}>
      {header}
      <div className={styles.scroll}>
        <div className={styles.content}>{children}</div>
      </div>
    </div>
  );
}
