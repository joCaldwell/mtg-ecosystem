import styles from "./Mark.module.css";

export type MarkKind = "memory" | "ruling" | "paper";

// What each marker says, shared by the rows and the legend under the list.
export const MARK_MEANING: Record<MarkKind, string> = {
  memory: "A memory is linked to this card",
  ruling: "You have ruled on this card before",
  paper: "Not in the paper deck",
};

/** A 6px marker on a card row. Shape as well as colour tells them apart. */
export function Mark({ kind, title = MARK_MEANING[kind] }: { kind: MarkKind; title?: string }) {
  return <i className={`${styles.mark} ${styles[kind]}`} title={title} />;
}
