import styles from "./Mark.module.css";

export type MarkKind = "memory" | "decision";

// What each marker says: a row's marker is named with it, and the legend
// under the list prints it beside the same shape. No tooltip (owner rule).
export const MARK_LABEL: Record<MarkKind, string> = {
  memory: "Memory linked",
  decision: "Decided before",
};

/** A small marker on a card row. Shape as well as colour tells them apart.
 *  `decorative` when its words are printed beside it, as in the legend. */
export function Mark({ kind, decorative }: { kind: MarkKind; decorative?: boolean }) {
  const cls = `${styles.mark} ${styles[kind]}`;
  return decorative
    ? <i className={cls} aria-hidden="true" />
    : <i className={cls} role="img" aria-label={MARK_LABEL[kind]} />;
}
