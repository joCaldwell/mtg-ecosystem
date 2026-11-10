import { useEffect, useRef, useState } from "react";
import { Button, Field, TextField } from "../../ui/index.ts";
import styles from "./DecisionControls.module.css";

export interface DecisionControlsProps {
  /** true: show the reason field instead of the two buttons. */
  rejecting: boolean;
  /** Called with the reason field's text when `reason` is given. */
  onAccept: (reason?: string) => void;
  onStartReject: () => void;
  onReject: (reason: string) => void;
  onCancelReject: () => void;
  /** Disables the controls while a decision is in flight. */
  busy?: boolean;
  /** Show the A / R key hints on the buttons. */
  showKeys?: boolean;
  acceptLabel?: string;
  rejectLabel?: string;
  /** An optional reason field above the buttons, as on the phone (§11.2),
   *  sent with either decision: Accept passes it to onAccept and Reject
   *  decides at once through onReject. The reject key (`rejecting`) still opens
   *  the focused reason field, starting from this text. */
  reason?: { value: string; onChange: (value: string) => void; label: string };
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
  reason,
}: DecisionControlsProps) {
  if (rejecting) {
    return <ReasonField busy={busy} label={rejectLabel} initial={reason?.value ?? ""} onReject={onReject} onCancel={onCancelReject} />;
  }
  const buttons = (
    <div className={styles.controls}>
      <Button variant="accept" size="md" kbd={showKeys ? "A" : undefined} disabled={busy} onClick={() => onAccept(reason?.value)}>
        {acceptLabel}
      </Button>
      <Button variant="reject" size="md" kbd={showKeys ? "R" : undefined} disabled={busy} onClick={reason ? () => onReject(reason.value) : onStartReject}>
        {rejectLabel}
      </Button>
    </div>
  );
  if (!reason) return buttons;
  return (
    <div className={styles.withReason}>
      <Field label="Reason (optional)"><TextField
        aria-label={reason.label}
        value={reason.value}
        maxLength={500}
        disabled={busy}
        onChange={(e) => reason.onChange(e.target.value)}
      /></Field>
      {buttons}
    </div>
  );
}

function ReasonField({
  busy,
  label,
  initial,
  onReject,
  onCancel,
}: {
  busy: boolean;
  label: string;
  initial: string;
  onReject: (reason: string) => void;
  onCancel: () => void;
}) {
  const [reason, setReason] = useState(initial);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => input.current?.focus(), []);

  return (
    <div className={styles.withReason}>
      <Field label="Reason for rejecting (optional)"><TextField
        ref={input}
        aria-label="Reason for rejecting, optional"
        // A sheet opened for the reason focuses it (ui/Dialog).
        data-autofocus
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
      /></Field>
      <div className={styles.controls}>
      <Button variant="reject" size="md" kbd="↵" disabled={busy} onClick={() => onReject(reason)}>
        {label}
      </Button>
      <Button variant="ghost" size="md" disabled={busy} onClick={onCancel}>
        Cancel
      </Button>
      </div>
    </div>
  );
}
