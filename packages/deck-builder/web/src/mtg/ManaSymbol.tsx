import { useState, type CSSProperties } from "react";
import { parseSymbol, symbolSvgUrl, WUBRG } from "./manaSymbols.ts";
import styles from "./ManaSymbol.module.css";

/** The token a hybrid half is painted with in the fallback disc. */
function halfColor(half: string): string {
  if (half.length === 1 && WUBRG.includes(half)) return `var(--mana-${half.toLowerCase()})`;
  if (half === "C") return "var(--mana-c)";
  return "var(--mana-generic)";
}

/**
 * The fallback's numeral, centred on the disc's middle by its digits, not by
 * its line box. SVG text is placed on its alphabetic baseline, so the
 * baseline sits half a digit height below the centre: 0.33em is half the
 * cap height of the mono faces in --font-mono (0.64–0.73em), which keeps
 * the ink within half a pixel of the centre at 14 and 16 px. CSS centring
 * puts it there only if the font's ascent and descent happen to balance,
 * and for the Linux mono faces they leave the digit 1–1.5 px high.
 */
function Numeral({ text }: { text: string }) {
  return (
    <svg className={styles.numeral} viewBox="0 0 100 100" aria-hidden="true" focusable="false">
      <text x="50" y="50" dy="0.33em" textAnchor="middle" className={text.length > 1 ? styles.long : undefined}>
        {text}
      </text>
    </svg>
  );
}

/**
 * One mana symbol, from inside the braces: "U", "3", "X", "C", "T", "S",
 * "B/G", "2/B", "W/P". Drawn with Scryfall's own SVG (symbolSvgUrl). Until
 * that loads, or if it cannot (offline), the box shows a plain disc in the
 * symbol's colours: a numeral for generic costs, a glyph for tap and the
 * like, and no letter on a coloured disc. Size comes from the `--sym-size`
 * custom property (default 14px) so a parent can scale a whole row of them;
 * the image and the disc both fill that box, so nothing moves when it loads.
 * The spoken name is the wrapper's; the image itself is silent.
 */
export function ManaSymbol({ symbol }: { symbol: string }) {
  const { shape, phyrexian, name } = parseSymbol(symbol);
  const url = symbolSvgUrl(symbol);
  // Keyed by URL so a reused element showing another symbol starts over.
  const [loaded, setLoaded] = useState<string | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const cls = [styles.sym];
  let glyph: string | null = null;
  let style: CSSProperties | undefined;

  switch (shape.kind) {
    case "colour":
      cls.push(styles[shape.letter.toLowerCase()]);
      break;
    case "colourless":
      cls.push(styles.c);
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
      break;
    }
  }
  if (phyrexian) glyph = null;

  const showImage = url !== null && failed !== url;
  if (showImage && loaded === url) cls.push(styles.loaded);

  return (
    <span className={cls.join(" ")} style={style} role="img" aria-label={name} title={name}>
      {glyph && <Numeral text={glyph} />}
      {showImage && (
        <img
          key={url}
          className={styles.image}
          src={url}
          alt=""
          width={14}
          height={14}
          loading="lazy"
          decoding="async"
          draggable={false}
          onLoad={() => setLoaded(url)}
          onError={() => setFailed(url)}
        />
      )}
    </span>
  );
}
