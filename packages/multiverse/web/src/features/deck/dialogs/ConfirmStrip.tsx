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
  busy = false,
  tone = "cut",
}: {
  children: ReactNode;
  confirmLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
  busy?: boolean;
  /** "accept" for a bulk accept, which is a deliberate step but not a loss. */
  tone?: "cut" | "accept";
}) {
  const cancel = useRef<HTMLButtonElement>(null);
  useEffect(() => cancel.current?.focus(), []);
  return (
    <div
      className={tone === "accept" ? `${styles.strip} ${styles.accept}` : styles.strip}
      onKeyDown={(event) => {
        if (event.key !== "Escape" || event.defaultPrevented || event.nativeEvent.isComposing || event.nativeEvent.keyCode === 229) return;
        event.preventDefault();
        event.stopPropagation();
        if (!busy) onCancel();
      }}
    >
      <span className={styles.text} role="alert">{children}</span>
      <Button ref={cancel} size="sm" aria-disabled={busy || undefined} onClick={() => { if (!busy) onCancel(); }}>
        Cancel
      </Button>
      <Button size="sm" variant={tone === "accept" ? "accept" : "reject"} aria-disabled={busy || undefined} onClick={() => { if (!busy) onConfirm(); }}>
        {confirmLabel}
      </Button>
    </div>
  );
}

/** Wait for the removed row to unmount, unless the user already moved focus. */
export function restoreFocusAfterRemoval(from: Element | null, next: () => HTMLElement | null) {
  requestAnimationFrame(() => {
    if (document.activeElement === from || (!from?.isConnected && document.activeElement === document.body)) next()?.focus();
  });
}
