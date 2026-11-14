import { cloneElement, isValidElement, useId, type ReactElement, type ReactNode } from "react";
import { cx } from "./internal.ts";
import styles from "./Field.module.css";

export interface FieldProps {
  label: ReactNode;
  hint?: ReactNode;
  /** One control; existing IDs and descriptions are preserved when wiring the label and hint. */
  children: ReactNode;
  className?: string;
}

export function Field({ label, hint, children, className }: FieldProps) {
  const id = useId();
  const hintId = `${id}-hint`;
  let control = children;
  let controlId = id;
  if (isValidElement(children) && typeof children.type !== "symbol") {
    const el = children as ReactElement<{ id?: string; "aria-describedby"?: string }>;
    controlId = el.props.id ?? id;
    const descriptions = [el.props["aria-describedby"], hint ? hintId : undefined].filter(Boolean).join(" ");
    control = cloneElement(el, { id: controlId, ...(descriptions ? { "aria-describedby": descriptions } : {}) });
  }
  return (
    <div className={cx(styles.field, className)}>
      <label htmlFor={controlId} className={styles.label}>
        {label}
      </label>
      {control}
      {hint ? (
        <div id={hintId} className={styles.hint}>
          {hint}
        </div>
      ) : null}
    </div>
  );
}
