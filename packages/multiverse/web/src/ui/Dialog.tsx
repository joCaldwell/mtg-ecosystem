import { useId, useLayoutEffect, useRef, type PointerEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Icon } from "./Icon.tsx";
import { IconButton } from "./IconButton.tsx";
import styles from "./Dialog.module.css";

export interface DialogProps {
  title: ReactNode;
  onClose: () => void;
  /** An inner inspection view can go back before Escape closes the sheet. */
  onEscape?: () => void;
  width?: "md" | "lg" | "wide";
  /** `sheet` rises from the bottom edge at full width: the touch form (DESIGN.md). */
  placement?: "center" | "sheet" | "drawer";
  children: ReactNode;
  footer?: ReactNode;
  /** A workflow with its own scrolling region can size the body. */
  bodyClassName?: string;
}

/** Native modal semantics keep the surrounding document inert and restore focus. */
export function Dialog({ title, onClose, onEscape, width = "md", placement = "center", children, footer, bodyClassName }: DialogProps) {
  const titleId = useId();
  const panel = useRef<HTMLDialogElement>(null);
  const pressedOutside = useRef(false);

  useLayoutEffect(() => {
    const dialog = panel.current;
    if (!dialog) return;
    // Keep the existing data-autofocus convention for composed workflow footers.
    const target = dialog.querySelector<HTMLElement>("[data-autofocus]");
    target?.setAttribute("autofocus", "");
    dialog.showModal();
    return () => dialog.close();
  }, []);

  function outside(event: PointerEvent<HTMLDialogElement>) {
    const rect = event.currentTarget.getBoundingClientRect();
    return event.clientX < rect.left || event.clientX > rect.right || event.clientY < rect.top || event.clientY > rect.bottom;
  }

  return createPortal(
    <dialog
      ref={panel}
      aria-labelledby={titleId}
      {...({ closedby: onEscape ? "closerequest" : "any" } as Record<string, string>)}
      className={`${styles.dialog} ${placement === "sheet" ? styles.sheet : placement === "drawer" ? styles.drawer : styles[width]}`}
      onCancel={(event) => {
        // The controlled workflow can refuse dismissal while a write is in flight,
        // or go back from an inner inspection without closing the sheet.
        event.preventDefault();
        (onEscape ?? onClose)();
      }}
      onClose={(event) => { if (!event.currentTarget.open) onClose(); }}
      onPointerDown={(event) => {
        if (onEscape || !("closedBy" in event.currentTarget)) pressedOutside.current = outside(event);
      }}
      onPointerUp={(event) => {
        if ((onEscape || !("closedBy" in event.currentTarget)) && pressedOutside.current && outside(event)) onClose();
        pressedOutside.current = false;
      }}
      onPointerCancel={() => { pressedOutside.current = false; }}
    >
      <div className={styles.head}>
        <h2 id={titleId} className={styles.title}>{title}</h2>
        <IconButton label="Close" size={placement !== "center" ? "touch" : "sm"} onClick={onClose}>
          <Icon name="close" />
        </IconButton>
      </div>
      <div className={`${styles.body} ${bodyClassName ?? ""}`}>{children}</div>
      {footer ? <div className={styles.footer}>{footer}</div> : null}
    </dialog>,
    document.body,
  );
}
