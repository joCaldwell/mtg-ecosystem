import { forwardRef, type AnchorHTMLAttributes } from "react";
import { cx } from "./internal.ts";
import styles from "./Button.module.css";

export interface ButtonLinkProps extends AnchorHTMLAttributes<HTMLAnchorElement> {
  href: string;
  variant?: "default" | "ghost" | "primary";
  /** `touch` is one touch target tall (--tap), for phone and touch layouts. */
  size?: "sm" | "md" | "touch";
}

/** A link drawn as a Button, for an action that goes somewhere else. */
export const ButtonLink = forwardRef<HTMLAnchorElement, ButtonLinkProps>(function ButtonLink(
  { variant = "default", size = "md", className, children, ...rest },
  ref,
) {
  return (
    <a
      ref={ref}
      className={cx(styles.button, styles[variant], size === "sm" && styles.sm, size === "touch" && styles.touch, className)}
      {...rest}
    >
      {children}
    </a>
  );
});
