import { forwardRef, type ButtonHTMLAttributes } from "react";
import { Kbd } from "./Kbd.tsx";
import { cx } from "./internal.ts";
import styles from "./Button.module.css";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "default" | "ghost" | "primary" | "accept" | "reject" | "agent";
  /** `touch` is one touch target tall (--tap), for phone and touch layouts. */
  size?: "sm" | "md" | "touch";
  /** Shortcut hint shown as a key cap after the label. */
  kbd?: string;
  /** Extends the hit area to one touch target (--tap) tall with invisible
   *  padding, keeping the drawn size. Leave room around it in the layout. */
  reach?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = "default", size = "md", kbd, reach = false, type = "button", className, children, ...rest },
  ref,
) {
  return (
    <button
      ref={ref}
      type={type}
      className={cx(styles.button, styles[variant], size === "sm" && styles.sm, size === "touch" && styles.touch, reach && styles.reach, className)}
      {...rest}
    >
      {children}
      {kbd ? <Kbd>{kbd}</Kbd> : null}
    </button>
  );
});
