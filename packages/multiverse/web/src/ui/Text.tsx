import { createElement, type HTMLAttributes } from "react";
import { cx } from "./internal.ts";
import styles from "./Text.module.css";

export interface TextProps extends HTMLAttributes<HTMLElement> {
  tone?: "default" | "soft" | "muted" | "faint" | "agent" | "add" | "cut" | "warn";
  size?: "xs" | "sm" | "md" | "lg" | "xl";
  /** Monospace with tabular numerals: counts, revisions, times. */
  mono?: boolean;
  weight?: "normal" | "medium" | "bold";
  as?: "span" | "p" | "div" | "h1" | "h2" | "h3";
}

export function Text({ tone = "default", size, mono = false, weight, as = "span", className, ...rest }: TextProps) {
  return createElement(as, {
    className: cx(
      tone !== "default" && styles[tone],
      size && styles[`size-${size}`],
      mono && styles.mono,
      weight && styles[weight],
      className,
    ),
    ...rest,
  });
}
