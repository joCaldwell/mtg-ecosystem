import type { HTMLAttributes, ReactNode } from "react";
import { cx } from "./internal.ts";
import styles from "./Banner.module.css";

export interface BannerProps extends Omit<HTMLAttributes<HTMLDivElement>, "children"> {
  tone: "agent" | "warn" | "error" | "neutral";
  children: ReactNode;
  /** Buttons and the like, shown on the right. */
  actions?: ReactNode;
}

export function Banner({ tone, children, actions, className, ...rest }: BannerProps) {
  return (
    <div
      role={tone === "error" ? "alert" : undefined}
      className={cx(styles.banner, styles[tone], className)}
      {...rest}
    >
      <div className={styles.body}>{children}</div>
      {actions ? <div className={styles.actions}>{actions}</div> : null}
    </div>
  );
}
