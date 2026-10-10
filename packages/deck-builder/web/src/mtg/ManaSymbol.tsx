import type { CSSProperties } from "react";
import { parseSymbol, WUBRG } from "./manaSymbols.ts";
import styles from "./ManaSymbol.module.css";

/** The token a hybrid half is painted with. */
function halfColor(half: string): string {
  if (half.length === 1 && WUBRG.includes(half)) return `var(--mana-${half.toLowerCase()})`;
  if (half === "C") return "var(--mana-c)";
  return "var(--mana-generic)";
}

/**
 * One mana symbol, from inside the braces: "U", "3", "X", "C", "T", "S",
 * "B/G", "2/B", "W/P". Size comes from the `--sym-size` custom property
 * (default 14px) so a parent can scale a whole row of them.
 */
export function ManaSymbol({ symbol }: { symbol: string }) {
  const { shape, phyrexian, name } = parseSymbol(symbol);
  const cls = [styles.sym];
  let glyph: string | null = null;
  let corner: string | null = null;
  let style: CSSProperties | undefined;

  switch (shape.kind) {
    case "colour":
      cls.push(styles[shape.letter.toLowerCase()]);
      break;
    case "colourless":
      cls.push(styles.c);
      glyph = "C";
      break;
    case "generic":
      cls.push(styles.generic);
      glyph = shape.text;
      break;
    case "other":
      cls.push(shape.letter === "S" ? styles.c : styles.tap);
      glyph = shape.letter;
      break;
    case "hybrid": {
      const [a, b] = shape.halves;
      cls.push(styles.hybrid);
      if (a === "B" || b === "B") cls.push(styles.hasBlack);
      style = { "--a": halfColor(a), "--b": halfColor(b) } as CSSProperties;
      // A numeric half ("2/B") keeps its number, tucked into its own corner.
      const numeric = [a, b].find((h) => /^\d+$/.test(h));
      if (numeric) corner = numeric;
      break;
    }
  }

  if (phyrexian) {
    cls.push(styles.phyrexian);
    glyph = "P";
  }
  if (glyph && glyph.length > 1) cls.push(styles.long);

  return (
    <span className={cls.join(" ")} style={style} role="img" aria-label={name} title={name}>
      {glyph}
      {corner && <span className={styles.corner}>{corner}</span>}
    </span>
  );
}
