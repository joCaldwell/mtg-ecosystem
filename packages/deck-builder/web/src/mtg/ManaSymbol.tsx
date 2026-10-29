import type { CSSProperties } from "react";
import { parseSymbol, WUBRG } from "./manaSymbols.ts";
import styles from "./ManaSymbol.module.css";

/** The token a hybrid half is painted with. */
function halfColor(half: string): string {
  if (half.length === 1 && WUBRG.includes(half)) return `var(--mana-${half.toLowerCase()})`;
  if (half === "C") return "var(--mana-c)";
  return "var(--mana-generic)";
}

/** The ink that reads on a half's fill: pale on black, dark on the rest. */
function halfInk(half: string): string {
  return half === "B" ? "var(--mana-b-ring)" : "var(--mana-ink)";
}

/** A hybrid half's glyph: its colour letter, or the number of a "2/B" half. */
function halfGlyph(half: string): string | null {
  if (half.length === 1 && (WUBRG.includes(half) || half === "C")) return half;
  if (/^\d+$/.test(half)) return half;
  return null;
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
  let halves: [string | null, string | null] | null = null;
  let style: CSSProperties | undefined;

  switch (shape.kind) {
    case "colour":
      // The letter is drawn as well as the fill, so colour is never the only
      // cue (WCAG 1.4.1). Its name stays the aria-label below.
      cls.push(styles[shape.letter.toLowerCase()]);
      glyph = shape.letter;
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
      style = { "--a": halfColor(a), "--b": halfColor(b), "--a-ink": halfInk(a), "--b-ink": halfInk(b) } as CSSProperties;
      // Each half carries its own letter (or a "2/B" half its number), tucked
      // into its corner of the diagonal and inked to suit that half's fill.
      halves = [halfGlyph(a), halfGlyph(b)];
      break;
    }
  }

  if (phyrexian) {
    cls.push(styles.phyrexian);
    glyph = "P";
    halves = null;
  }
  if (glyph && glyph.length > 1) cls.push(styles.long);

  return (
    <span className={cls.join(" ")} style={style} role="img" aria-label={name} title={name}>
      {glyph}
      {halves?.[0] && <span className={`${styles.half} ${styles.first}`}>{halves[0]}</span>}
      {halves?.[1] && <span className={`${styles.half} ${styles.second}`}>{halves[1]}</span>}
    </span>
  );
}
