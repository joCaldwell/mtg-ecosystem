import type { HTMLAttributes, MouseEventHandler } from "react";
import { cx } from "./internal.ts";
import styles from "./Chip.module.css";

export interface ChipProps extends Omit<HTMLAttributes<HTMLElement>, "onClick"> {
  tone?: "neutral" | "agent" | "warn" | "add" | "cut";
  /** Outline only, dashed: for things not yet confirmed. */
  dashed?: boolean;
  /** When given, the chip renders as a button. */
  onClick?: MouseEventHandler<HTMLButtonElement>;
  title?: string;
}

export function Chip({ tone = "neutral", dashed = false, onClick, className, children, ...rest }: ChipProps) {
  const cls = cx(styles.chip, styles[tone], dashed && styles.dashed, onClick && styles.interactive, className);
  if (onClick) {
    return (
      <button type="button" className={cls} onClick={onClick} {...(rest as HTMLAttributes<HTMLButtonElement>)}>
        {children}
      </button>
    );
  }
  return (
    <span className={cls} {...rest}>
      {children}
    </span>
  );
}
