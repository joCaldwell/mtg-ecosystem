// Small helpers shared by the primitives. Not part of the public surface.

import type { Ref } from "react";

/** Join class names, skipping falsy entries. */
export function cx(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(" ");
}

/** Point several refs (callback or object) at one element. */
export function mergeRefs<T>(...refs: Array<Ref<T> | undefined>): (node: T | null) => void {
  return (node) => {
    for (const ref of refs) {
      if (typeof ref === "function") ref(node);
      else if (ref) (ref as { current: T | null }).current = node;
    }
  };
}

export const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

// Dialogs and popovers share one Escape stack. Only the innermost surface
// dismisses; stopImmediatePropagation also protects other capture listeners.
export const escapeLayers: Array<() => void> = [];
export function dismissOnEscape(close: () => void): () => void {
  escapeLayers.push(close);
  const onKey = (event: KeyboardEvent) => {
    if (event.key !== "Escape" || escapeLayers.at(-1) !== close) return;
    event.preventDefault(); event.stopImmediatePropagation(); close();
  };
  document.addEventListener("keydown", onKey, true);
  return () => {
    const index = escapeLayers.indexOf(close);
    if (index >= 0) escapeLayers.splice(index, 1);
    document.removeEventListener("keydown", onKey, true);
  };
}
