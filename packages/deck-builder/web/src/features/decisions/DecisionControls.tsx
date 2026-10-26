import { useEffect, useRef, useState } from "react";
import { Button, TextField } from "../../ui/index.ts";
import styles from "./DecisionControls.module.css";

export interface DecisionControlsProps {
  /** true: show the reason field instead of the two buttons. */
  rejecting: boolean;
  onAccept: () => void;
  onStartReject: () => void;
  onReject: (reason: string) => void;
  onCancelReject: () => void;
  /** Disables the controls while a decision is in flight. */
  busy?: boolean;
  /** Show the A / R key hints on the buttons. */
  showKeys?: boolean;
  acceptLabel?: string;
  rejectLabel?: string;
}

/**
 * Accept, or reject with an optional reason (spec §8.2). The reason is one
 * keystroke away and may be empty, because a reason that costs effort does not
 * get written.
 */
export function DecisionControls({
  rejecting,
  onAccept,
  onStartReject,
  onReject,
  onCancelReject,
  busy = false,
  showKeys = false,
  acceptLabel = "Accept",
  rejectLabel = "Reject",
}: DecisionControlsProps) {
  if (rejecting) {
    return <ReasonField busy={busy} label={rejectLabel} onReject={onReject} onCancel={onCancelReject} />;
  }
  return (
    <div className={styles.controls}>
      <Button variant="accept" size="sm" kbd={showKeys ? "A" : undefined} disabled={busy} onClick={onAccept}>
        {acceptLabel}
      </Button>
      <Button variant="reject" size="sm" kbd={showKeys ? "R" : undefined} disabled={busy} onClick={onStartReject}>
        {rejectLabel}
      </Button>
    </div>
  );
}

function ReasonField({
  busy,
  label,
  onReject,
  onCancel,
}: {
  busy: boolean;
  label: string;
  onReject: (reason: string) => void;
  onCancel: () => void;
}) {
  const [reason, setReason] = useState("");
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => input.current?.focus(), []);

  return (
    <div className={styles.controls}>
      <TextField
        ref={input}
        className={styles.reason}
        aria-label="Reason for rejecting, optional"
        placeholder="Why? Optional. The agent reads this next session."
        value={reason}
        disabled={busy}
        onChange={(e) => setReason(e.target.value)}
        onKeyDown={(e) => {
          // Typing a reason must not trigger the screen's single-letter
          // shortcuts, and Escape closes this field, not the panel around it.
          e.stopPropagation();
          if (e.nativeEvent.isComposing) return;
          if (e.key === "Enter") {
            e.preventDefault();
            onReject(reason);
          } else if (e.key === "Escape") {
            e.preventDefault();
            onCancel();
          }
        }}
      />
      <Button variant="reject" size="sm" kbd="↵" disabled={busy} onClick={() => onReject(reason)}>
        {label}
      </Button>
      <Button variant="ghost" size="sm" disabled={busy} onClick={onCancel}>
        Cancel
      </Button>
    </div>
  );
}
