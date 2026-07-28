// The tool panels (brief, audit, interop, session) open here rather than
// stacking above the decklist. They are reference-and-act surfaces you visit
// and leave; as inline disclosures they pushed the actual deck an arbitrary
// distance down the page every time one was expanded. Proposals deliberately
// stay inline — ruling on them is the main loop and wants the list in view.

import { useEffect, useRef, type ReactNode } from "react";
import { useDismissable } from "./lib.ts";

export function Modal({
  title,
  onClose,
  children,
  wide,
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
  // Interop and session run wide (an export box, a segment meter).
  wide?: boolean;
}) {
  const bodyRef = useRef<HTMLDivElement>(null);

  useDismissable(onClose);

  // Focus the panel so Escape and the scroll wheel land here, not on the
  // decklist underneath.
  useEffect(() => {
    bodyRef.current?.focus();
  }, []);

  return (
    <div
      className="modal-backdrop"
      // Only a press that lands on the backdrop itself dismisses. Tested by
      // target rather than by stopping propagation inside the panel: that
      // stopped the press at React's root and never let it reach the
      // document, where the card peek listens for the click that dismisses
      // it — so a card opened from inside a modal could not be closed.
      // mousedown rather than click, so a text selection that ends outside
      // the panel does not dismiss it either.
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className={`modal ${wide ? "wide" : ""}`} role="dialog" aria-modal="true" aria-label={title}>
        <header className="modal-head">
          <h2>{title}</h2>
          <button className="small" onClick={onClose} aria-label="Close">
            esc ✕
          </button>
        </header>
        <div className="modal-body" ref={bodyRef} tabIndex={-1}>
          {children}
        </div>
      </div>
    </div>
  );
}
