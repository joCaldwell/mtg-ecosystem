import { useId } from "react";
import styles from "./BrandMark.module.css";

// Three cards fanned from one point below them, the front card on the right.
// The same geometry is inlined in web/index.html as the favicon.
const CARD = { x: 7, y: 6, width: 10, height: 14, rx: 1.6 };
const ANGLES = [-16, 0, 16];
const PIVOT = "12 30";
// Each card is cut back from the one in front of it, so the stack reads as
// separate cards in one colour at 16 px.
const GAP = 2.6;

/**
 * The product mark: a fanned stack of cards in `currentColor`. Decorative;
 * the wordmark or the link around it carries the name. Its height is the
 * surrounding text's cap height unless a host sets `--brand-mark-h`.
 */
export function BrandMark({ className }: { className?: string }) {
  const id = useId().replace(/[^a-zA-Z0-9_-]/g, "");
  const card = (angle: number) => <rect {...CARD} transform={`rotate(${angle} ${PIVOT})`} />;
  return (
    <svg className={className ? `${styles.mark} ${className}` : styles.mark} viewBox="0.5 5.5 23 16.3"
      fill="currentColor" aria-hidden="true" focusable="false">
      <defs>
        {ANGLES.slice(0, -1).map((_, i) => (
          <mask key={i} id={`${id}-${i}`} maskUnits="userSpaceOnUse">
            <rect x="-10" y="-10" width="44" height="44" fill="white" />
            <g fill="black" stroke="black" strokeWidth={GAP}>{card(ANGLES[i + 1])}</g>
          </mask>
        ))}
      </defs>
      {ANGLES.map((angle, i) => (
        <g key={angle} mask={i < ANGLES.length - 1 ? `url(#${id}-${i})` : undefined}>{card(angle)}</g>
      ))}
    </svg>
  );
}
