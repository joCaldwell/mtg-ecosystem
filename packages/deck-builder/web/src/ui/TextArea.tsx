import { forwardRef, useLayoutEffect, useRef, type TextareaHTMLAttributes } from "react";
import { cx, mergeRefs } from "./internal.ts";
import styles from "./TextArea.module.css";

export interface TextAreaProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  /** Grow with the content instead of scrolling. */
  autosize?: boolean;
}

type InputEv = Parameters<NonNullable<TextareaHTMLAttributes<HTMLTextAreaElement>["onInput"]>>[0];

function fit(el: HTMLTextAreaElement) {
  el.style.height = "auto";
  el.style.height = `${el.scrollHeight}px`;
}

export const TextArea = forwardRef<HTMLTextAreaElement, TextAreaProps>(function TextArea(
  { autosize = false, className, value, onInput, rows, ...rest },
  ref,
) {
  const inner = useRef<HTMLTextAreaElement | null>(null);

  // Re-measure whenever the controlled value changes (and on mount).
  useLayoutEffect(() => {
    if (autosize && inner.current) fit(inner.current);
  }, [autosize, value]);

  function handleInput(e: InputEv) {
    if (autosize) fit(e.currentTarget);
    onInput?.(e);
  }

  return (
    <textarea
      ref={mergeRefs(inner, ref)}
      rows={rows ?? (autosize ? 2 : 3)}
      value={value}
      onInput={handleInput}
      className={cx(styles.area, autosize && styles.autosize, className)}
      {...rest}
    />
  );
});
