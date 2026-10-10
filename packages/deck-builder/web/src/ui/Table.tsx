import type { HTMLAttributes, KeyboardEvent, MouseEventHandler, TableHTMLAttributes } from "react";
import { cx } from "./internal.ts";
import styles from "./Table.module.css";

/** A styled table. Children are ordinary thead / tbody / tr / th / td. */
export function Table({ className, ...rest }: TableHTMLAttributes<HTMLTableElement>) {
  return <table className={cx(styles.table, className)} {...rest} />;
}

export interface ClickableRowProps extends Omit<HTMLAttributes<HTMLTableRowElement>, "onClick"> {
  onClick: MouseEventHandler<HTMLTableRowElement>;
}

/** A row that opens something. Reachable by keyboard: Enter or Space activates it. */
export function ClickableRow({ onClick, onKeyDown, className, ...rest }: ClickableRowProps) {
  function handleKeyDown(e: KeyboardEvent<HTMLTableRowElement>) {
    onKeyDown?.(e);
    if (e.defaultPrevented || e.target !== e.currentTarget) return;
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      e.currentTarget.click();
    }
  }
  return <tr tabIndex={0} className={cx(styles.clickable, className)} onClick={onClick} onKeyDown={handleKeyDown} {...rest} />;
}
