import { useEffect, useRef, type ReactNode } from "react";
import { Button } from "../../../ui/index.ts";
import styles from "./ConfirmStrip.module.css";

/**
 * A question asked in place, for a delete inside a dialog. A second dialog
 * would stack two focus traps and two Escape handlers. Focus starts on Cancel,
 * so a delete always takes a deliberate second step.
 */
export function ConfirmStrip({
  children,
  confirmLabel,
  onConfirm,
  onCancel,
  tone = "cut",
}: {
  children: ReactNode;
  confirmLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
  /** "accept" for a bulk accept, which is a deliberate step but not a loss. */
  tone?: "cut" | "accept";
}) {
  const cancel = useRef<HTMLButtonElement>(null);
  useEffect(() => cancel.current?.focus(), []);
  return (
    <div className={tone === "accept" ? `${styles.strip} ${styles.accept}` : styles.strip} role="alert">
      <span className={styles.text}>{children}</span>
      <Button ref={cancel} size="sm" onClick={onCancel}>
        Cancel
      </Button>
      <Button size="sm" variant={tone === "accept" ? "accept" : "reject"} onClick={onConfirm}>
        {confirmLabel}
      </Button>
    </div>
  );
}
