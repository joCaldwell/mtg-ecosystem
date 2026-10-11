import { formatUsd } from "../domain/price.ts";
import styles from "./Price.module.css";

/**
 * What a card costs: its cheapest printing, in dollars (spec §4.4). Draws
 * nothing for a card with no price, so a list of prices stays a list of
 * numbers.
 */
export function Price({
  cents,
  column,
  title = "Cheapest printing",
}: {
  cents: number | null | undefined;
  column?: boolean;
  /** Says what the figure is; a total passes its own. */
  title?: string;
}) {
  if (cents == null) return column ? <span className={styles.column} /> : null;
  return (
    <span className={column ? `${styles.price} ${styles.column}` : styles.price} title={title}>
      {formatUsd(cents)}
    </span>
  );
}
