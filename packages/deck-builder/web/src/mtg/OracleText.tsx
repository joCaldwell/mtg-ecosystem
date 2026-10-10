import { Fragment, type ReactNode } from "react";
import { ManaSymbol } from "./ManaSymbol.tsx";
import styles from "./OracleText.module.css";

/** Braced symbols inside a run of text become inline ManaSymbols. */
function withSymbols(text: string): ReactNode[] {
  const out: ReactNode[] = [];
  let last = 0;
  for (const m of text.matchAll(/\{([^}]+)\}/g)) {
    if (m.index > last) out.push(text.slice(last, m.index));
    out.push(<ManaSymbol key={m.index} symbol={m[1]} />);
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

/** One ability's line: reminder text in parentheses is muted italic, symbols are inline. */
function line(text: string): ReactNode[] {
  // Reminder text can itself hold symbols ("({T}: Add {G}.)"), so split on the
  // parentheses first and expand symbols inside each piece.
  return text.split(/(\([^)]*\))/).map((piece, i) =>
    piece.startsWith("(") && piece.endsWith(")") ? (
      <span key={i} className={styles.reminder}>
        {withSymbols(piece)}
      </span>
    ) : (
      <Fragment key={i}>{withSymbols(piece)}</Fragment>
    ),
  );
}

/**
 * A card's rules text. Newlines separate abilities, each on its own line.
 * `bare` drops the left rule, for use inside a surface that already frames it.
 * Font size follows `--oracle-size` (default `--text-md`); symbols scale with it.
 */
export function OracleText({ text, bare = false }: { text: string; bare?: boolean }) {
  const lines = text.split("\n").filter((l) => l.trim());
  if (lines.length === 0) return null;
  return (
    <div className={bare ? `${styles.oracle} ${styles.bare}` : styles.oracle}>
      {lines.map((l, i) => (
        <p key={i} className={styles.line}>
          {line(l)}
        </p>
      ))}
    </div>
  );
}
