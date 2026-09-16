import type { ReactNode } from "react";
import { Modal } from "./Modal.tsx";

/** In-app replacement for native confirm(). The cancel action receives focus
 * first so a destructive choice always takes an intentional second step. */
export function ConfirmDialog({
  title,
  confirmLabel,
  onConfirm,
  onClose,
  children,
  destructive = false,
}: {
  title: string;
  confirmLabel: string;
  onConfirm: () => void;
  onClose: () => void;
  children: ReactNode;
  destructive?: boolean;
}) {
  return (
    <Modal title={title} onClose={onClose}>
      <div className="confirm-copy">{children}</div>
      <div className="dialog-actions">
        <button data-modal-autofocus onClick={onClose}>
          Cancel
        </button>
        <button className={destructive ? "destructive" : "primary"} onClick={onConfirm}>
          {confirmLabel}
        </button>
      </div>
    </Modal>
  );
}
