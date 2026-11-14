import { ManaSymbol } from "./ManaSymbol.tsx";
import { COLOR_NAMES, WUBRG } from "./manaSymbols.ts";
import styles from "./ColorIdentity.module.css";

/**
 * A colour identity as small mana symbols in WUBRG order, announced once as
 * the group's name (an image's children are presentational). `colors` is bare letters
 * ("UBR") or "" for colourless, which shows as the single colourless symbol.
 * `label` names what the colours are: a card's identity, or a token's colour.
 */
export function ColorIdentity({ colors, label: what = "Colour identity" }: { colors: string; label?: string }) {
  const upper = colors.toUpperCase();
  const letters = [...WUBRG].filter((c) => upper.includes(c));
  const shown = letters.length > 0 ? letters : ["C"];
  const label = letters.length > 0 ? letters.map((c) => COLOR_NAMES[c]).join(", ") : "colourless";
  return (
    <span className={styles.identity} role="img" aria-label={`${what}: ${label}`} title={`${what}: ${label}`}>
      {shown.map((c) => (
        <ManaSymbol key={c} symbol={c} />
      ))}
    </span>
  );
}
