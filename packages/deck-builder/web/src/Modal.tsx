// The tool panels (brief, audit, interop, session) open here rather than
// stacking above the decklist. They are reference-and-act surfaces you visit
// and leave; as inline disclosures they pushed the actual deck an arbitrary
// distance down the page every time one was expanded. Proposals deliberately
// stay inline — ruling on them is the main loop and wants the list in view.

import { useEffect, useRef, type ReactNode } from "react";

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

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("keydown", onKey);
    // The page behind must not scroll under the overlay.
    const prev = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    // Focus the panel so Escape and the scroll wheel land here, not on the
    // decklist underneath.
    bodyRef.current?.focus();
    return () => {
      document.removeEventListener("keydown", onKey);
      document.body.style.overflow = prev;
    };
  }, [onClose]);

  return (
    <div className="modal-backdrop" onMouseDown={onClose}>
      <div
        className={`modal ${wide ? "wide" : ""}`}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        // Clicks inside must not reach the backdrop's close handler. mousedown
        // rather than click so a text selection that ends outside the panel
        // does not dismiss it.
        onMouseDown={(e) => e.stopPropagation()}
      >
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
