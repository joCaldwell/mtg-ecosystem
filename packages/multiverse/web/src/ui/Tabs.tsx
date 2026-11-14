import { useRef, type KeyboardEvent, type ReactNode } from "react";
import { Badge } from "./Badge.tsx";
import { cx } from "./internal.ts";
import styles from "./Tabs.module.css";

export interface TabSpec {
  id: string;
  label: ReactNode;
  badge?: number;
  tone?: "agent";
}

export interface TabsProps {
  tabs: TabSpec[];
  value: string;
  onChange: (id: string) => void;
  className?: string;
  "aria-label"?: string;
}

export function Tabs({ tabs, value, onChange, className, "aria-label": ariaLabel }: TabsProps) {
  const refs = useRef(new Map<string, HTMLButtonElement>());

  function onKeyDown(e: KeyboardEvent) {
    const i = tabs.findIndex((t) => t.id === value);
    let next = -1;
    if (e.key === "ArrowRight") next = (i + 1) % tabs.length;
    else if (e.key === "ArrowLeft") next = (i - 1 + tabs.length) % tabs.length;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = tabs.length - 1;
    if (next < 0 || i < 0) return;
    e.preventDefault();
    const tab = tabs[next];
    if (!tab) return;
    onChange(tab.id);
    refs.current.get(tab.id)?.focus();
  }

  return (
    <div role="tablist" aria-label={ariaLabel} className={cx(styles.tabs, className)} onKeyDown={onKeyDown}>
      {tabs.map((t) => {
        const on = t.id === value;
        return (
          <button
            key={t.id}
            ref={(node) => {
              if (node) refs.current.set(t.id, node);
              else refs.current.delete(t.id);
            }}
            type="button"
            role="tab"
            aria-selected={on}
            tabIndex={on ? 0 : -1}
            className={cx(styles.tab, on && styles.on, t.tone === "agent" && styles.agent)}
            onClick={() => onChange(t.id)}
          >
            {t.label}
            {t.badge ? <Badge tone={t.tone}>{t.badge}</Badge> : null}
          </button>
        );
      })}
    </div>
  );
}
