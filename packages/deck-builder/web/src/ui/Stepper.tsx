import { forwardRef } from "react";
import { Icon } from "./Icon.tsx";
import { cx } from "./internal.ts";
import { TextField, type TextFieldProps } from "./TextField.tsx";
import styles from "./Stepper.module.css";

export interface StepperProps extends TextFieldProps {
  onStepUp: () => void;
  onStepDown: () => void;
  /** Accessible names and hover labels for the two icon buttons. */
  upLabel: string;
  downLabel: string;
  /** At a limit. The button stays focusable, so holding the keyboard on it
   *  does not drop focus to the page as a disabled button would. */
  upDisabled?: boolean;
  downDisabled?: boolean;
}

/** A number you can type or nudge. Arrows with shafts, side by side: nothing
 *  here opens a list, and each button is a whole target at every width. */
export const Stepper = forwardRef<HTMLInputElement, StepperProps>(function Stepper(
  { onStepUp, onStepDown, upLabel, downLabel, upDisabled = false, downDisabled = false, className, disabled, ...rest },
  ref,
) {
  const arrow = (name: "arrowDown" | "arrowUp", label: string, off: boolean, step: () => void) => (
    <button
      type="button"
      className={styles.arrow}
      aria-label={label}
      title={label}
      aria-disabled={off || undefined}
      onClick={() => { if (!off) step(); }}
    >
      <Icon name={name} />
    </button>
  );
  return (
    <span className={cx(styles.stepper, className)}>
      <TextField ref={ref} className={styles.input} inputMode="numeric" disabled={disabled} {...rest} />
      {arrow("arrowDown", downLabel, downDisabled || !!disabled, onStepDown)}
      {arrow("arrowUp", upLabel, upDisabled || !!disabled, onStepUp)}
    </span>
  );
});
