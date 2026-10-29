import { cx } from "./internal.ts";
import styles from "./DiffView.module.css";

export interface DiffLine {
  kind: "same" | "add" | "del";
  text: string;
}

/** Added and deleted lines. `foldSame` shows only the changes: each run of
 *  unchanged lines becomes one closed row ("4 unchanged lines") that opens
 *  in place. */
export function DiffView({ lines, foldSame = false, className }: { lines: DiffLine[]; foldSame?: boolean; className?: string }) {
  const runs: DiffLine[][] = [];
  for (const line of lines) {
    const run = runs.at(-1);
    if (foldSame && run && run[0].kind === "same" && line.kind === "same") run.push(line);
    else runs.push([line]);
  }
  return (
    <div className={cx(styles.diff, className)}>
      {runs.map((run, i) =>
        foldSame && run[0].kind === "same" ? (
          <details key={i} className={styles.fold}>
            <summary className={styles.summary}>
              {run.length} unchanged {run.length === 1 ? "line" : "lines"}
            </summary>
            {run.map((line, j) => <Line key={j} line={line} />)}
          </details>
        ) : (
          <Line key={i} line={run[0]} />
        ),
      )}
    </div>
  );
}

function Line({ line }: { line: DiffLine }) {
  return (
    <div className={cx(styles.line, line.kind === "add" && styles.add, line.kind === "del" && styles.del)}>
      {/* Screen readers get the sign too; sighted users get it as a gutter. */}
      {line.kind !== "same" ? (
        <span className={styles.sign} aria-label={line.kind === "add" ? "added" : "removed"}>
          {line.kind === "add" ? "+" : "−"}
        </span>
      ) : null}
      {line.text}
    </div>
  );
}
