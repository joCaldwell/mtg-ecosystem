import type { ReactNode } from "react";
import { Eyebrow } from "./Eyebrow.tsx";
import { cx } from "./internal.ts";
import styles from "./Section.module.css";

export interface SectionProps {
  title: ReactNode;
  /** Shown to the right of the title. */
  aside?: ReactNode;
  children: ReactNode;
  className?: string;
}

export function Section({ title, aside, children, className }: SectionProps) {
  return (
    <section className={cx(styles.section, className)}>
      <div className={styles.head}>
        <Eyebrow>{title}</Eyebrow>
        {aside ? <div className={styles.aside}>{aside}</div> : null}
      </div>
      {children}
    </section>
  );
}
