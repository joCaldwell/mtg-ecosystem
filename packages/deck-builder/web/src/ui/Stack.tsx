import { createElement, type HTMLAttributes } from "react";
import { cx } from "./internal.ts";
import styles from "./Stack.module.css";

export type Gap = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8;

export interface StackProps extends HTMLAttributes<HTMLElement> {
  /** Maps to the spacing scale, --s-N. */
  gap?: Gap;
  as?: "div" | "section" | "ul" | "ol" | "nav" | "header" | "footer" | "main" | "form";
}

/** Vertical stack. */
export function Stack({ gap = 4, as = "div", className, ...rest }: StackProps) {
  return createElement(as, { className: cx(styles.stack, styles[`gap${gap}`], className), ...rest });
}
