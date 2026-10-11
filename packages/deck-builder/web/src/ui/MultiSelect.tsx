import { useEffect, useId, useRef, useState } from "react";
import { Button } from "./Button.tsx";
import { Checkbox } from "./Checkbox.tsx";
import { Icon } from "./Icon.tsx";
import { Popover } from "./Popover.tsx";
import { TextField } from "./TextField.tsx";
import styles from "./MultiSelect.module.css";

export interface MultiSelectProps {
  label: string;
  placeholder?: string;
  options: Array<{ value: string; label: string }>;
  value: string[];
  /** Controlled selection; async writes keep controls disabled until they settle. */
  onChange: (value: string[]) => void | Promise<unknown>;
  emptyText?: string;
}

/** A searchable checkbox dropdown. Only selected values appear under the trigger. */
export function MultiSelect({ label, placeholder = "Select…", options, value, onChange, emptyText = "No options available." }: MultiSelectProps) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  const search = useRef<HTMLInputElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const pending = useRef(false);
  const restoreFocus = useRef<HTMLElement | null>(null);
  const id = useId();
  const selected = options.filter((option) => value.includes(option.value));
  const visible = options.filter((option) => option.label.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()));

  useEffect(() => {
    if (open) search.current?.focus();
    else setQuery("");
  }, [open]);

  useEffect(() => {
    if (!busy && open && restoreFocus.current) {
      restoreFocus.current.focus();
      restoreFocus.current = null;
    }
  }, [busy, open]);

  function close() {
    setOpen(false);
    anchor?.focus();
  }

  async function toggle(option: string) {
    if (pending.current) return;
    const result = onChange(value.includes(option) ? value.filter((v) => v !== option) : [...value, option]);
    if (!result) return;
    const focused = document.activeElement as HTMLElement;
    if (panel.current?.contains(focused)) restoreFocus.current = focused;
    pending.current = true;
    setBusy(true);
    try {
      await result;
    } finally {
      pending.current = false;
      setBusy(false);
    }
  }

  return (
    <div className={styles.control} aria-busy={busy}>
      <Button
        ref={setAnchor}
        className={styles.trigger}
        aria-label={label}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        onClick={() => setOpen((current) => !current)}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown") {
            event.preventDefault();
            setOpen(true);
          }
        }}
      >
        <span>{placeholder}</span>
        <Icon name="chevronDown" size={12} />
      </Button>
      {selected.length > 0 && (
        <div className={styles.selected} aria-label={`${label}: selected`}>
          {selected.map((option) => (
            <button
              key={option.value}
              type="button"
              className={styles.chip}
              disabled={busy}
              aria-label={`Remove ${option.label} from ${label.toLowerCase()}`}
              onClick={() => void toggle(option.value)}
            >
              <span>{option.label}</span><Icon name="close" size={12} />
            </button>
          ))}
        </div>
      )}
      <Popover anchor={anchor} open={open} onClose={close} className={styles.popover}>
        <div
          ref={panel}
          id={id}
          role="dialog"
          aria-label={label}
          className={styles.panel}
          onBlur={(event) => {
            // Disabling a focused checkbox during a write can blur it with
            // no next target. Keep the dropdown open for further selections.
            if (event.relatedTarget && !event.currentTarget.contains(event.relatedTarget) && !anchor?.contains(event.relatedTarget)) setOpen(false);
          }}
          onKeyDown={(event) => {
            if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
            const inputs = Array.from(panel.current?.querySelectorAll<HTMLInputElement>("input:not(:disabled)") ?? []);
            const current = inputs.indexOf(document.activeElement as HTMLInputElement);
            event.preventDefault();
            inputs[(current + (event.key === "ArrowDown" ? 1 : inputs.length - 1)) % inputs.length]?.focus();
          }}
        >
          <TextField ref={search} type="search" aria-label={`Search ${label.toLowerCase()}`} placeholder="Search…" value={query} onChange={(event) => setQuery(event.target.value)} />
          <div className={styles.options} role="group" aria-label="Available choices">
            {visible.map((option) => (
              <Checkbox key={option.value} className={styles.option} label={option.label} checked={value.includes(option.value)} disabled={busy} onChange={() => void toggle(option.value)} />
            ))}
            {visible.length === 0 && <p className={styles.empty}>{options.length ? "No matching options." : emptyText}</p>}
          </div>
          <Button size="sm" variant="ghost" onClick={close}>Done</Button>
        </div>
      </Popover>
    </div>
  );
}
