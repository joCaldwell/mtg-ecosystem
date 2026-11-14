import { Fragment } from "react";
import { ManaSymbol } from "./ManaSymbol.tsx";
import { costFaces } from "./manaSymbols.ts";
import styles from "./ManaCost.module.css";

/**
 * A mana cost as a row of symbols. `cost` is Scryfall's string: "{3}{W}{U}",
 * "{B/G}", "{X}{U}", or two faces joined by " // ". Renders nothing when the
 * card has no cost.
 */
export function ManaCost({ cost }: { cost: string | null | undefined }) {
  const faces = costFaces(cost);
  if (faces.length === 0) return null;
  return (
    <span className={styles.cost}>
      {faces.map((symbols, f) => (
        <Fragment key={f}>
          {f > 0 && (
            <span className={styles.slash} aria-hidden="true">
              //
            </span>
          )}
          <span className={styles.face}>
            {symbols.map((s, i) => (
              <ManaSymbol key={i} symbol={s} />
            ))}
          </span>
        </Fragment>
      ))}
    </span>
  );
}
