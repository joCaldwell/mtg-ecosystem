import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import type { MenuItem } from "./Menu.tsx";
import { Popover } from "./Popover.tsx";
import styles from "./ContextMenu.module.css";
import menuStyles from "./Menu.module.css";

export interface ContextMenuTarget {
  element: HTMLElement;
  x: number;
  y: number;
}
export interface ContextMenuProps {
  target: ContextMenuTarget | null;
  label: string;
  items: (MenuItem & { separator?: boolean })[];
  onClose: () => void;
}

/** A pointer-positioned menu with keyboard navigation and focus restoration. */
export function ContextMenu({ target, label, items, onClose }: ContextMenuProps) {
  const [anchor, setAnchor] = useState<HTMLSpanElement | null>(null);
  const list = useRef<HTMLDivElement>(null);
  const enabled = () => Array.from(
    list.current?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)') ?? [],
  );
  const firstEnabled = items.find((item) => !item.disabled)?.id;
  useEffect(() => {
    if (target && anchor) enabled()[0]?.focus({ preventScroll: true });
  }, [target, anchor, firstEnabled]);
  function close() {
    onClose();
    target?.element.focus({ preventScroll: true });
  }
  function keyDown(e: KeyboardEvent) {
    // The menu owns these keys; table hotkeys must not fire while it is open.
    e.stopPropagation();
    const buttons = enabled(), index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    let next = -1;
    if (e.key === "ArrowDown") next = (index + 1) % buttons.length;
    else if (e.key === "ArrowUp") next = (index - 1 + buttons.length) % buttons.length;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = buttons.length - 1;
    else if (e.key === "Tab") { close(); return; }
    else return;
    e.preventDefault();
    buttons[next]?.focus();
  }
  if (!target) return null;
  return <>
    <span ref={setAnchor} className={styles.anchor} style={{ left: target.x, top: target.y }} />
    <Popover anchor={anchor} open onClose={close}>
      <div ref={list} role="menu" aria-label={label} className={menuStyles.list} onKeyDown={keyDown}>
        <div className={styles.heading}>{label}</div>
        {items.map((item) => <button
          key={item.id}
          type="button"
          role="menuitem"
          tabIndex={-1}
          disabled={item.disabled}
          className={`${menuStyles.item} ${item.tone === "cut" ? menuStyles.cut : ""} ${item.separator ? styles.separator : ""}`}
          onClick={() => { close(); item.onSelect(); }}
        >{item.label}</button>)}
      </div>
    </Popover>
  </>;
}
