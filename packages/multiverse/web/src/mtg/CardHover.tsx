import {
  lazy, Suspense,
  createContext,
  useCallback,
  useContext,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import type { CardInfo } from "./CardInfo.ts";
import { useCardLookup } from "./CardLookup.tsx";

const CardSheet = lazy(() => import("./CardSheet.tsx").then(m => ({ default: m.CardSheet })));
const HoverCard = lazy(() => import("./CardHoverView.tsx").then(m => ({ default: m.HoverCard })));

/** How long the pointer must rest on a card name before the hover card appears. */
export const HOVER_DELAY_MS = 500;

interface Shown {
  card: CardInfo;
  anchor: DOMRect;
}

interface HoverApi {
  inspect: (card: string | CardInfo) => void;
  show: (card: string | CardInfo, el: HTMLElement) => void;
  hide: () => void;
}

const Ctx = /* @__PURE__ */ createContext<HoverApi | null>(null);

export function CardHoverProvider({ children }: { children: ReactNode }) {
  const { lookup } = useCardLookup();
  const [inspected, setInspected] = useState<CardInfo | null>(null);
  const [shown, setShown] = useState<Shown | null>(null);
  const timer = useRef<number | undefined>(undefined);
  // Bumped on every show and hide, so a lookup that finishes late can tell it is stale.
  const epoch = useRef(0);
  const anchorEl = useRef<HTMLElement | null>(null);

  const hide = useCallback(() => {
    window.clearTimeout(timer.current);
    timer.current = undefined;
    epoch.current++;
    anchorEl.current = null;
    setShown(null);
  }, []);

  const show = useCallback(
    (card: string | CardInfo, el: HTMLElement) => {
      hide();
      // Touch screens emulate a hover on tap; there the card opens as a sheet
      // instead (CardSheet), so a tap never leaves a stray hover card behind.
      if (typeof window.matchMedia === "function" && window.matchMedia("(hover: none)").matches) return;
      anchorEl.current = el;
      const mine = epoch.current;
      timer.current = window.setTimeout(() => {
        timer.current = undefined;
        if (!el.isConnected) return;
        const anchor = el.getBoundingClientRect();
        if (typeof card !== "string") {
          setShown({ card, anchor });
          return;
        }
        lookup(card).then(
          (info) => {
            if (info && epoch.current === mine && el.isConnected) setShown({ card: info, anchor });
          },
          () => {},
        );
      }, HOVER_DELAY_MS);
    },
    [hide, lookup],
  );

  // Anything that could move the anchor or mean the reader has moved on closes it.
  useEffect(() => {
    const onOver = (e: MouseEvent) => {
      const el = anchorEl.current;
      // Also catches a trigger that unmounted under a resting pointer, which never fires mouseleave.
      if (el && !(e.target instanceof Node && el.contains(e.target))) hide();
    };
    window.addEventListener("scroll", hide, true);
    window.addEventListener("resize", hide);
    document.addEventListener("mousedown", hide, true);
    document.addEventListener("keydown", hide, true);
    document.addEventListener("mouseover", onOver);
    return () => {
      window.removeEventListener("scroll", hide, true);
      window.removeEventListener("resize", hide);
      document.removeEventListener("mousedown", hide, true);
      document.removeEventListener("keydown", hide, true);
      document.removeEventListener("mouseover", onOver);
      window.clearTimeout(timer.current);
    };
  }, [hide]);

  const inspect = useCallback((card: string | CardInfo) => {
    hide();
    if (typeof card !== "string") setInspected(card);
    else void lookup(card).then(info => info && setInspected(info));
  }, [hide, lookup]);
  const api = useMemo(() => ({ show, hide, inspect }), [show, hide, inspect]);
  return (
    <Ctx.Provider value={api}>
      {children}
      <Suspense fallback={null}>{inspected && <CardSheet card={inspected} sheet onClose={() => setInspected(null)} />}
      {shown && createPortal(<HoverCard key={shown.card.name} shown={shown} />, document.body)}</Suspense>
    </Ctx.Provider>
  );
}

const NO_HANDLERS = { onMouseEnter: () => {}, onMouseLeave: () => {} };

/**
 * Spread the result on any element that names a card. After the pointer rests
 * for HOVER_DELAY_MS the hover card appears beside it; leaving hides it at
 * once. Given a name it resolves the card through `useCardLookup`; given a
 * CardInfo it shows that immediately.
 */
export function useCardHover(): (card: string | CardInfo) => {
  onMouseEnter: (e: ReactMouseEvent<HTMLElement>) => void;
  onMouseLeave: () => void;
} {
  const api = useContext(Ctx);
  return useCallback(
    (card) =>
      api
        ? {
            onMouseEnter: (e) => api.show(card, e.currentTarget),
            onMouseLeave: api.hide,
          }
        : NO_HANDLERS,
    [api],
  );
}

/** Explicit inspection is a modal, so its facts are read by assistive tech. */
export function useCardInspect() {
  const api = useContext(Ctx);
  return (card: string | CardInfo) => api?.inspect(card);
}
