import type { ReactNode } from "react";
import styles from "./Pill.module.css";

export function Pill({ on, onClick, children }: { on: boolean; onClick: () => void; children: ReactNode }) {
  return <button type="button" aria-pressed={on} className={`${styles.pill} ${on ? styles.pillOn : ""}`} onClick={onClick}>{children}</button>;
}
