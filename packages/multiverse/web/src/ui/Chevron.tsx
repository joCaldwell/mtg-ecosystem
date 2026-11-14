import { Icon } from "./Icon.tsx";
import styles from "./Chevron.module.css";

/** The one dropdown chevron. It turns over while the control around it has
 *  `aria-expanded="true"`, so a trigger only has to say whether it is open. */
export function Chevron() {
  return <Icon name="chevronDown" className={styles.chevron} />;
}
