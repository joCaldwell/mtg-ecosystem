import { forwardRef, type ButtonHTMLAttributes } from "react";
import { Kbd } from "./Kbd.tsx";
import { cx } from "./internal.ts";
import styles from "./Button.module.css";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "default" | "ghost" | "primary" | "accept" | "reject" | "agent";
  size?: "sm" | "md";
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
      className={cx(styles.button, styles[variant], size === "sm" && styles.sm, className)}
      {...rest}
    >
      {children}
      {kbd ? <Kbd>{kbd}</Kbd> : null}
    </button>
  );
});
