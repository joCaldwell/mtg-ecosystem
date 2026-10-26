import { createContext, useContext, type MouseEvent, type ReactNode } from "react";
import type { CardInfo } from "./CardInfo.ts";
import { useCardHover, useCardInspect } from "./CardHover.tsx";
import styles from "./CardRef.module.css";

const OpenCtx = /* @__PURE__ */ createContext<((name: string) => void) | null>(null);

/** A screen that can show card details (the deck page) provides this, so a CardRef anywhere under it opens the card. */
export function CardOpenProvider({ onOpen, children }: { onOpen: (name: string) => void; children: ReactNode }) {
  return <OpenCtx.Provider value={onOpen}>{children}</OpenCtx.Provider>;
}

/**
 * A card's name inline in text, with the hover card attached. Clicking calls
 * `onOpen` if given, else the nearest CardOpenProvider, else does nothing.
 * Pass `card` when the caller already has the data, to skip the lookup.
 */
export function CardRef({ name, card, onOpen }: { name: string; card?: CardInfo; onOpen?: (name: string) => void }) {
  const inspect = useCardInspect();
  const hover = useCardHover();
  const fromContext = useContext(OpenCtx);
  const open = onOpen ?? fromContext;
  return (
    <button
      type="button"
      className={open ? `${styles.ref} ${styles.openable}` : styles.ref}
      {...hover(card ?? name)}
      onKeyDown={(e) => { if (e.key === "?") { e.preventDefault(); e.stopPropagation(); inspect(card ?? name); } }}
      onClick={(e: MouseEvent) => {
        // A reference inside a clickable row should open the card, not select the row.
        e.stopPropagation();
        if (open) open(name); else inspect(card ?? name);
      }}
    >
      {name}
    </button>
  );
}
