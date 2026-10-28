import { forwardRef, type SelectHTMLAttributes } from "react";
import { Icon } from "./Icon.tsx";
import { cx } from "./internal.ts";
import styles from "./Select.module.css";

export type SelectProps = SelectHTMLAttributes<HTMLSelectElement>;

/** A native select, restyled. The chevron is drawn by us so it follows the text colour. */
export const Select = forwardRef<HTMLSelectElement, SelectProps>(function Select(
  { className, children, ...rest },
  ref,
) {
  return (
    <span className={styles.wrap}>
      <select ref={ref} className={cx(styles.select, className)} {...rest}>
        {children}
      </select>
      <span className={styles.chevron}>
        <Icon name="chevronDown" size={16} />
      </span>
    </span>
  );
});
