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
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = "default", size = "md", kbd, type = "button", className, children, ...rest },
  ref,
) {
  return (
    <button
      ref={ref}
      type={type}
      className={cx(styles.button, styles[variant], size === "sm" && styles.sm, size === "touch" && styles.touch, className)}
      {...rest}
    >
      {children}
      {kbd ? <Kbd>{kbd}</Kbd> : null}
    </button>
  );
});
