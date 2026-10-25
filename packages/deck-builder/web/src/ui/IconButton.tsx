import { forwardRef, type ButtonHTMLAttributes } from "react";
import { cx } from "./internal.ts";
import styles from "./IconButton.module.css";

export interface IconButtonProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "aria-label" | "title"> {
  /** What the button does. Becomes the accessible name and the tooltip. */
  label: string;
  /** `touch` is one touch target tall (--tap), for phone and touch layouts. */
  size?: "sm" | "md" | "touch";
}

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  { label, size = "md", type = "button", className, children, ...rest },
  ref,
) {
  return (
    <button
      ref={ref}
      type={type}
      aria-label={label}
      title={label}
      className={cx(styles.iconButton, size === "sm" && styles.sm, size === "touch" && styles.touch, className)}
      {...rest}
    >
      {children}
    </button>
  );
});
