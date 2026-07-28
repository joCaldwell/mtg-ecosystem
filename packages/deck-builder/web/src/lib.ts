// Small shared hooks and formatters. Anything here is used by at least two
// panels; single-panel helpers stay with their panel.

import { useEffect, useLayoutEffect, useState, type RefObject } from "react";

export function ago(iso: string): string {
  // SQLite writes 'YYYY-MM-DD HH:MM:SS' in UTC with no zone marker.
  const then = Date.parse(iso.replace(" ", "T") + "Z");
  if (Number.isNaN(then)) return iso;
  const secs = Math.max(0, Math.round((Date.now() - then) / 1000));
  if (secs < 60) return `${secs}s ago`;
  if (secs < 3600) return `${Math.round(secs / 60)}m ago`;
  if (secs < 86400) return `${Math.round(secs / 3600)}h ago`;
  return `${Math.round(secs / 86400)}d ago`;
}

/** The corner stat, printed the way the card prints it: power/toughness, or
 *  loyalty in brackets. Empty for cards that have neither. */
export function ptString(card: {
  power: string | null;
  toughness: string | null;
  loyalty: string | null;
}): string {
  return card.power != null && card.toughness != null
    ? `${card.power}/${card.toughness}`
    : card.loyalty != null
      ? `[${card.loyalty}]`
      : "";
}

/**
 * Keeps a textarea as tall as its content, re-measured whenever `value`
 * changes. Keyed on the value rather than wired to onChange because drafts
 * also change from outside — a reference dropped in, a send clearing it.
 * Height has to go back to auto first: scrollHeight is measured against the
 * current box, so a box that already grew would never shrink again. CSS owns
 * the floor and the ceiling. `pad` puts back what scrollHeight leaves out —
 * a bordered textarea needs its border widths added; a chromeless one doesn't.
 */
export function useAutosize(ref: RefObject<HTMLTextAreaElement | null>, value: unknown, pad = 0) {
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${el.scrollHeight + pad}px`;
  }, [ref, value, pad]);
}

/** useState persisted under `key`. `decode` maps what was stored (null on
 *  first visit) to a value and must fall back rather than throw — the stored
 *  string is user data; `encode` maps the value back. */
export function useLocalStorage<T>(
  key: string,
  decode: (raw: string | null) => T,
  encode: (value: T) => string,
) {
  const [value, setValue] = useState<T>(() => decode(localStorage.getItem(key)));
  // encode is deliberately not a dependency: call sites pass inline arrows,
  // and the write only needs to happen when the value moves.
  useEffect(() => {
    localStorage.setItem(key, encode(value));
  }, [key, value]);
  return [value, setValue] as const;
}

/** A surface that covers the page: Escape closes it, and the page behind must
 *  not scroll underneath it. `enabled` gates the whole behaviour, for a
 *  surface that only sometimes covers the page (the maximized side panel).
 *  The Escape listener is bubble-phase on the document, so anything inside
 *  that wants Escape for itself (a field editor, the card peek) can stop the
 *  event before it gets here. */
export function useDismissable(onClose: () => void, enabled = true) {
  useEffect(() => {
    if (!enabled) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    const prev = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.removeEventListener("keydown", onKey);
      document.body.style.overflow = prev;
    };
  }, [onClose, enabled]);
}
