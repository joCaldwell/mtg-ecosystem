import { useEffect, useId, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { Button, type ButtonProps } from "./Button.tsx";
import { Icon, type IconName } from "./Icon.tsx";
import { IconButton } from "./IconButton.tsx";
import { Popover } from "./Popover.tsx";
import styles from "./Menu.module.css";
import { cx } from "./internal.ts";

export interface MenuItem {
  id: string;
  label: ReactNode;
  onSelect: () => void;
  tone?: "cut";
  disabled?: boolean;
}

export type MenuProps = {
  /** Accessible name when the visible label repeats across rows. */
  ariaLabel?: string;
  items: MenuItem[];
  variant?: ButtonProps["variant"];
  size?: ButtonProps["size"];
  /** Which edge of the trigger the list lines up with. */
  align?: "start" | "end";
} & (
  | { label: ReactNode; icon?: undefined }
  /** An icon trigger uses its label as the accessible name and tooltip. */
  | { label: string; icon: IconName }
);

export function Menu({ label, icon, ariaLabel, items, variant = "default", size = "md", align = "start" }: MenuProps) {
  const [open, setOpen] = useState(false);
  const [trigger, setTrigger] = useState<HTMLButtonElement | null>(null);
  const list = useRef<HTMLDivElement>(null);
  const menuId = useId();

  const enabled = () => Array.from(list.current?.querySelectorAll<HTMLElement>('[role="menuitem"]:not(:disabled)') ?? []);

  // Focus the first item as the menu appears.
  useEffect(() => {
    if (open) enabled()[0]?.focus({ preventScroll: true });
  }, [open]);

  function close() {
    setOpen(false);
    trigger?.focus();
  }

  function onTriggerKeyDown(e: KeyboardEvent) {
    if (e.key === "ArrowDown" && !open) {
      e.preventDefault();
      setOpen(true);
    }
  }

  function onListKeyDown(e: KeyboardEvent) {
    const els = enabled();
    const i = els.indexOf(document.activeElement as HTMLElement);
    let next = -1;
    if (e.key === "ArrowDown") next = (i + 1) % els.length;
    else if (e.key === "ArrowUp") next = (i - 1 + els.length) % els.length;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = els.length - 1;
    else if (e.key === "Tab") {
      setOpen(false);
      return;
    } else return;
    e.preventDefault();
    els[next]?.focus();
  }

  const triggerProps = {
    ref: setTrigger,
    size,
    "aria-haspopup": "menu" as const,
    "aria-expanded": open,
    "aria-controls": open ? menuId : undefined,
    onClick: () => setOpen((o) => !o),
    onKeyDown: onTriggerKeyDown,
  };

  return (
    <>
      {icon ? (
        <IconButton {...triggerProps} label={ariaLabel ?? label}>
          <Icon name={icon} />
        </IconButton>
      ) : (
        <Button {...triggerProps} variant={variant} aria-label={ariaLabel}>
          {label}
          <Icon name="chevronDown" size={12} />
        </Button>
      )}
      <Popover anchor={trigger} open={open} onClose={close} align={align}>
        <div
          id={menuId}
          ref={list}
          role="menu"
          className={styles.list}
          onKeyDown={onListKeyDown}
        >
          {items.map((item) => (
            <button
              key={item.id}
              type="button"
              role="menuitem"
              tabIndex={-1}
              disabled={item.disabled}
              className={cx(styles.item, item.tone === "cut" && styles.cut)}
              onClick={() => {
                setOpen(false);
                trigger?.focus();
                item.onSelect();
              }}
            >
              {item.label}
            </button>
          ))}
        </div>
      </Popover>
    </>
  );
}
