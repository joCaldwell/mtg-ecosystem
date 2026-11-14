import styles from "./Spacer.module.css";

/** Pushes whatever follows it to the far end of a Row. */
export function Spacer() {
  return <span className={styles.spacer} aria-hidden="true" />;
}
