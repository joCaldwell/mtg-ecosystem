// Click any card the agent named ([[Card Name]] chips, proposal rows) to see
// what it actually does without leaving the turn you're reading. Deliberately
// not a `Modal`: that one dims and blurs the page, which is right for a surface
// you visit and leave, and wrong for a reference you consult mid-sentence. This
// one floats next to the word you clicked, leaves the interface untouched
// behind it, and closes on the next click anywhere.

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
} from "react";
import { api, type CardData } from "./api.ts";
import { ptString } from "./lib.ts";
import { ManaCost } from "./Mana.tsx";

const W = 340;
const GAP = 8;
const MARGIN = 8;

/** Name-keyed so every surface shares one lookup; misses are cached as []. */
const cache = new Map<string, CardData[]>();
const key = (name: string) => name.trim().toLowerCase();

type Peek = { name: string; anchor: DOMRect; cards: CardData[] | null };

const PeekContext = createContext<((name: string, anchor: DOMRect) => void) | null>(null);

export function CardPeekProvider({ children }: { children: ReactNode }) {
  const [open, setOpen] = useState<Peek | null>(null);
  // Guards against a slow resolve landing after you have moved on to another
  // card (or closed the popover) and overwriting what is on screen.
  const reqRef = useRef(0);

  const peek = useCallback((name: string, anchor: DOMRect) => {
    const k = key(name);
    const req = ++reqRef.current;
    const hit = cache.get(k);
    setOpen({ name, anchor, cards: hit ?? null });
    if (hit) return;
    api
      .resolveCard(name)
      .then((cards) => {
        cache.set(k, cards);
        if (reqRef.current === req) setOpen((p) => (p ? { ...p, cards } : p));
      })
      .catch(() => {
        if (reqRef.current === req) setOpen((p) => (p ? { ...p, cards: [] } : p));
      });
  }, []);

  const close = useCallback(() => {
    reqRef.current++;
    setOpen(null);
  }, []);

  return (
    <PeekContext.Provider value={peek}>
      {children}
      {open && <Popover peek={open} close={close} />}
    </PeekContext.Provider>
  );
}

export function useCardPeek() {
  const ctx = useContext(PeekContext);
  if (!ctx) throw new Error("useCardPeek outside CardPeekProvider");
  return ctx;
}

/** Everything needed to make an element a peek trigger for its own card. */
export function usePeekProps() {
  const peek = useCardPeek();
  return useCallback((name: string) => peekProps(name, peek), [peek]);
}

/**
 * Opens the popover for the element's own text. The trigger is a span, not a
 * button: these sit inside prose, where a nested button breaks text selection
 * and the browser's own word wrapping.
 */
export function peekProps(name: string, peek: (name: string, anchor: DOMRect) => void) {
  return {
    role: "button" as const,
    tabIndex: 0,
    title: `${name} — click for card text`,
    // mousedown, not click: the document-level dismiss listener runs on
    // mousedown too, and a click handler would fire after it and reopen.
    onMouseDown: (e: ReactMouseEvent) => {
      e.stopPropagation();
      peek(name, e.currentTarget.getBoundingClientRect());
    },
    onKeyDown: (e: ReactKeyboardEvent) => {
      if (e.key !== "Enter" && e.key !== " ") return;
      e.preventDefault();
      peek(name, e.currentTarget.getBoundingClientRect());
    },
  };
}

function Popover({ peek, close }: { peek: Peek; close: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null);

  // Placed after paint, because where it fits depends on how tall the oracle
  // text made it — and that is not known until the cards have loaded.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const { anchor } = peek;
    const h = el.offsetHeight;
    const below = window.innerHeight - anchor.bottom - GAP - MARGIN;
    // Prefer below the word; flip above only if there is genuinely more room
    // there, so the popover doesn't jump sides as the text streams in.
    const top =
      h <= below || anchor.top - GAP - MARGIN < below
        ? Math.min(anchor.bottom + GAP, window.innerHeight - MARGIN - h)
        : anchor.top - GAP - h;
    const left = Math.min(Math.max(MARGIN, anchor.left), window.innerWidth - MARGIN - W);
    setPos({ top: Math.max(MARGIN, top), left });
  }, [peek]);

  useEffect(() => {
    function onDown(e: MouseEvent) {
      if (!ref.current?.contains(e.target as Node)) close();
    }
    // Escape dismisses the popover and stops there. A modal underneath closes
    // on Escape too, and putting a card away should not also put away the
    // brief you were reading it against — so this runs in the capture phase,
    // ahead of every other Escape handler, and only the top layer goes.
    function onKey(e: KeyboardEvent) {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      close();
    }
    // Any scroll moves the anchor out from under us; there is nothing sensible
    // to reposition against, so it dismisses. Capture, since the scroll is on
    // an inner pane (chat transcript, decklist) and does not bubble — which
    // also means the popover's own overflow fires it, and that one must not
    // close the thing you are scrolling.
    function onScroll(e: Event) {
      // A scroll on the window itself targets `window`, which is not a Node —
      // `contains` throws on it rather than answering false.
      const t = e.target;
      if (!(t instanceof Node) || !ref.current?.contains(t)) close();
    }
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey, true);
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", close);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey, true);
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", close);
    };
  }, [close]);

  const { cards } = peek;
  return (
    <div
      ref={ref}
      className="card-peek"
      role="dialog"
      aria-label={peek.name}
      // Invisible until placed, rather than flashing at 0,0 first.
      style={{ top: pos?.top ?? 0, left: pos?.left ?? 0, visibility: pos ? "visible" : "hidden" }}
    >
      {cards == null ? (
        <div className="muted">resolving {peek.name}…</div>
      ) : cards.length === 0 ? (
        <div className="muted">No card named “{peek.name}”.</div>
      ) : (
        // More than one row means two printings share this name; showing both
        // beats silently picking one.
        cards.map((c) => <Face key={c.oracle_id} card={c} />)
      )}
    </div>
  );
}

function Face({ card }: { card: CardData }) {
  const pt = ptString(card);
  return (
    <div className="card-peek-face">
      <div className="card-peek-head">
        <span className="name">{card.name}</span>
        <ManaCost cost={card.mana_cost} />
      </div>
      <div className="card-peek-type">
        {card.type_line}
        {pt && <span className="pt"> {pt}</span>}
      </div>
      {card.oracle_text && <pre className="oracle-text">{card.oracle_text}</pre>}
      {card.commander_legality !== "legal" && (
        <span className="chip over">{card.commander_legality} in commander</span>
      )}
    </div>
  );
}
