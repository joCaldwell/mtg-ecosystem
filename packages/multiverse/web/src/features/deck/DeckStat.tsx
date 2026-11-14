import type { ReactNode } from "react";
import styles from "./DeckStat.module.css";

/** One of the header's numbers: a figure, a label, and a click that opens the tab that explains it. */
export function DeckStat({
  figure,
  tone = "neutral",
  title,
  onClick,
  children,
}: {
  figure: ReactNode;
  tone?: "neutral" | "add" | "warn" | "cut";
  title: string;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button type="button" className={`${styles.stat} ${styles[tone]}`} aria-description={title} onClick={onClick}>
      <b className={styles.figure}>{figure}</b>
      {children}
    </button>
  );
}
