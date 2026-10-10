import { forwardRef, type InputHTMLAttributes } from "react";
import { cx } from "./internal.ts";
import styles from "./TextField.module.css";

export interface TextFieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "size"> {
  /** Named fieldSize because `size` is a native input attribute. */
  fieldSize?: "md" | "lg";
  invalid?: boolean;
}

export const TextField = forwardRef<HTMLInputElement, TextFieldProps>(function TextField(
  { fieldSize = "md", invalid = false, className, type = "text", ...rest },
  ref,
) {
  return (
    <input
      ref={ref}
      type={type}
      aria-invalid={invalid || undefined}
      className={cx(styles.field, fieldSize === "lg" && styles.lg, invalid && styles.invalid, className)}
      {...rest}
    />
  );
});
