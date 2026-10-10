import { cx } from "./internal.ts";
import styles from "./DiffView.module.css";

export interface DiffLine {
  kind: "same" | "add" | "del";
  text: string;
}

export function DiffView({ lines, className }: { lines: DiffLine[]; className?: string }) {
  return (
    <div className={cx(styles.diff, className)}>
      {lines.map((line, i) => (
        <div key={i} className={cx(styles.line, line.kind === "add" && styles.add, line.kind === "del" && styles.del)}>
          {/* Screen readers get the sign too; sighted users get it as a gutter. */}
          {line.kind !== "same" ? (
            <span className={styles.sign} aria-label={line.kind === "add" ? "added" : "removed"}>
              {line.kind === "add" ? "+" : "−"}
            </span>
          ) : null}
          {line.text}
        </div>
      ))}
    </div>
  );
}
