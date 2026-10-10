import { cloneElement, isValidElement, useId, type ReactElement, type ReactNode } from "react";
import { cx } from "./internal.ts";
import styles from "./Field.module.css";

export interface FieldProps {
  label: ReactNode;
  hint?: ReactNode;
  /** The control. A single element without its own id is wired to the label. */
  children: ReactNode;
  className?: string;
}

export function Field({ label, hint, children, className }: FieldProps) {
  const id = useId();
  const hintId = `${id}-hint`;
  let control = children;
  if (isValidElement(children) && typeof children.type !== "symbol") {
    const el = children as ReactElement<{ id?: string; "aria-describedby"?: string }>;
    if (el.props.id === undefined) {
      control = cloneElement(el, { id, ...(hint ? { "aria-describedby": hintId } : {}) });
    }
  }
  return (
    <div className={cx(styles.field, className)}>
      <label htmlFor={id} className={styles.label}>
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
