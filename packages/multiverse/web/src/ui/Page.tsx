import type { ReactNode, UIEventHandler } from "react";
import { cx } from "./internal.ts";
import styles from "./Page.module.css";

export interface PageProps {
  /** Usually a PageHeader. Stays put while the body scrolls. */
  header?: ReactNode;
  children: ReactNode;
  /** md is reading width, lg is wide content, full uses all the room. */
  width?: "md" | "lg" | "full";
  /** False when the body supplies independently scrolling panes. */
  scroll?: boolean;
  className?: string;
  onScroll?: UIEventHandler<HTMLDivElement>;
}

export function Page({ header, children, width = "lg", scroll = true, className, onScroll }: PageProps) {
  return (
    <div className={cx(styles.page, styles[width], className)}>
      {header}
      <div className={scroll ? styles.scroll : styles.workspace} onScroll={onScroll}>
        <div className={styles.content}>{children}</div>
      </div>
    </div>
  );
}
