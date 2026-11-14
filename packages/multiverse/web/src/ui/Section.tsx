import type { ReactNode } from "react";
import { Eyebrow } from "./Eyebrow.tsx";
import { cx } from "./internal.ts";
import styles from "./Section.module.css";

export interface SectionProps {
  title: ReactNode;
  /** Shown to the right of the title. */
  aside?: ReactNode;
  heading?: "h2" | "h3" | "h4";
  children: ReactNode;
  className?: string;
}

export function Section({ title, aside, heading = "h2", children, className }: SectionProps) {
  return (
    <section className={cx(styles.section, className)}>
      <div className={styles.head}>
        <Eyebrow as={heading}>{title}</Eyebrow>
        {aside ? <div className={styles.aside}>{aside}</div> : null}
      </div>
      {children}
    </section>
  );
}
