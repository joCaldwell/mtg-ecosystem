import type { ReactNode } from "react";
import { Button } from "./Button.tsx";
import { Dialog } from "./Dialog.tsx";

export interface ConfirmDialogProps {
  title: ReactNode;
  children: ReactNode;
  confirmLabel: string;
  tone?: "default" | "danger";
  onConfirm: () => void;
  onClose: () => void;
}

/** A yes/no question. A dangerous confirm starts with focus on Cancel. */
export function ConfirmDialog({ title, children, confirmLabel, tone = "default", onConfirm, onClose }: ConfirmDialogProps) {
  const danger = tone === "danger";
  return (
    <Dialog
      title={title}
      onClose={onClose}
      footer={
        <>
          <Button variant="ghost" onClick={onClose} data-autofocus={danger ? "" : undefined}>
            Cancel
          </Button>
          <Button
            variant={danger ? "reject" : "primary"}
            onClick={onConfirm}
            data-autofocus={danger ? undefined : ""}
          >
            {confirmLabel}
          </Button>
        </>
      }
    >
      {children}
    </Dialog>
  );
}
