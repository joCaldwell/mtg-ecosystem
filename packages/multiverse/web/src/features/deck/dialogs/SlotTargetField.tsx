import { useId, useLayoutEffect, type RefObject } from "react";
import { Text, TextField, type TextFieldProps } from "../../../ui/index.ts";
import styles from "./SlotsTable.module.css";

interface SlotTargetFieldProps extends TextFieldProps {
  inputRef: RefObject<HTMLInputElement | null>;
  error?: string;
}

export function SlotTargetField({ inputRef, error, ...props }: SlotTargetFieldProps) {
  const errorId = useId();
  useLayoutEffect(() => {
    inputRef.current?.setCustomValidity(error ?? "");
  }, [inputRef, error]);

  return <>
    <TextField
      {...props}
      ref={inputRef}
      className={styles.num}
      inputMode="numeric"
      placeholder="none"
      invalid={!!error}
      aria-describedby={error ? errorId : undefined}
    />
    {error && <Text as="span" id={errorId} tone="cut" size="xs" className={styles.targetError}>{error}</Text>}
  </>;
}
