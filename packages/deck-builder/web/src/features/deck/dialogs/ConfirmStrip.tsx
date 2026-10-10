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
}: {
  children: ReactNode;
  confirmLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const cancel = useRef<HTMLButtonElement>(null);
  useEffect(() => cancel.current?.focus(), []);
  return (
    <div className={styles.strip} role="alert">
      <span className={styles.text}>{children}</span>
      <Button ref={cancel} size="sm" variant="ghost" onClick={onCancel}>
        Cancel
      </Button>
      <Button size="sm" variant="reject" onClick={onConfirm}>
        {confirmLabel}
      </Button>
    </div>
  );
}
