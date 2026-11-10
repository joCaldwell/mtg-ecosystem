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
  /** The touch form: a bottom sheet with tap-sized buttons. */
  sheet?: boolean;
}

/** A yes/no question. A dangerous confirm starts with focus on Cancel. */
export function ConfirmDialog({ title, children, confirmLabel, tone = "default", onConfirm, onClose, sheet = false }: ConfirmDialogProps) {
  const danger = tone === "danger";
  const size = sheet ? "touch" : "md";
  return (
    <Dialog
      title={title}
      onClose={onClose}
      placement={sheet ? "sheet" : "center"}
      footer={
        <>
          <Button variant="ghost" size={size} onClick={onClose} data-autofocus={danger ? "" : undefined}>
            Cancel
          </Button>
          <Button
            variant={danger ? "reject" : "primary"}
            size={size}
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
