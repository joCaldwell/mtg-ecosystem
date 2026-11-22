import { forwardRef, useLayoutEffect, useRef, type CSSProperties, type TextareaHTMLAttributes } from "react";
import { cx, mergeRefs } from "./internal.ts";
import styles from "./TextArea.module.css";

export interface TextAreaProps extends TextareaHTMLAttributes<HTMLTextAreaElement> {
  /** Grow with the content instead of scrolling. */
  autosize?: boolean;
}

type InputEv = Parameters<NonNullable<TextareaHTMLAttributes<HTMLTextAreaElement>["onInput"]>>[0];

function fit(el: HTMLTextAreaElement) {
  if (!el.clientWidth) return;
  el.style.height = "auto";
  el.style.height = `${el.scrollHeight + el.offsetHeight - el.clientHeight}px`;
}

export const TextArea = forwardRef<HTMLTextAreaElement, TextAreaProps>(function TextArea(
  { autosize = false, className, value, onInput, rows, style, ...rest },
  ref,
) {
  const inner = useRef<HTMLTextAreaElement | null>(null);
  const lineCount = rows ?? (autosize ? 2 : 3);
  const needsMeasurement = autosize && !globalThis.CSS?.supports("field-sizing", "content");

  useLayoutEffect(() => {
    const el = inner.current;
    if (!needsMeasurement || !el) return;
    const original = el.style.height;
    let width = el.clientWidth;
    const observer = new ResizeObserver(() => {
      if (width === el.clientWidth) return;
      width = el.clientWidth;
      fit(el);
    });
    observer.observe(el);
    return () => { observer.disconnect(); el.style.height = original; };
  }, [needsMeasurement]);

  // Re-measure whenever the controlled value changes (and on mount).
  useLayoutEffect(() => {
    if (needsMeasurement && inner.current) fit(inner.current);
  }, [needsMeasurement, lineCount, value]);

  function handleInput(e: InputEv) {
    if (needsMeasurement) fit(e.currentTarget);
    onInput?.(e);
  }

  return (
    <textarea
      ref={mergeRefs(inner, ref)}
      rows={lineCount}
      value={value}
      onInput={handleInput}
      style={autosize ? { "--textarea-rows": lineCount, ...style } as CSSProperties : style}
      className={cx(styles.area, autosize && styles.autosize, className)}
      {...rest}
    />
  );
});
