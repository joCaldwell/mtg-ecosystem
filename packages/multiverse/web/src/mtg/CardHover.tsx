import {
  lazy, Suspense,
  createContext,
  useCallback,
  useContext,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type FocusEvent as ReactFocusEvent,
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
  show: (card: string | CardInfo, el: HTMLElement, keyboard?: boolean) => void;
  hide: () => void;
  leave: () => void;
}

const Ctx = /* @__PURE__ */ createContext<HoverApi | null>(null);

export function CardHoverProvider({ children }: { children: ReactNode }) {
  const { lookup } = useCardLookup();
  const [inspected, setInspected] = useState<CardInfo | null>(null);
  const [shown, setShown] = useState<Shown | null>(null);
  const timer = useRef<number | undefined>(undefined);
  const leaveTimer = useRef<number | undefined>(undefined);
  const tooltipId = useId();
  const inspectionEpoch = useRef(0);
  const keyboardActive = useRef(false);
  const deferredPointer = useRef<{ card: string | CardInfo; el: HTMLElement } | null>(null);
  // Bumped on every show and hide, so a lookup that finishes late can tell it is stale.
  const epoch = useRef(0);
  const anchorEl = useRef<HTMLElement | null>(null);

  const hide = useCallback(() => {
    window.clearTimeout(timer.current);
    window.clearTimeout(leaveTimer.current);
    timer.current = undefined;
    epoch.current++;
    anchorEl.current = null;
    setShown(null);
  }, []);

  // Allow crossing the small gap between the name and preview. Content on
  // hover must remain hoverable, including with magnification/large pointers:
  // https://www.w3.org/WAI/WCAG22/Understanding/content-on-hover-or-focus.html
  const leave = useCallback(() => {
    window.clearTimeout(leaveTimer.current);
    leaveTimer.current = window.setTimeout(() => {
      const anchor = anchorEl.current;
      const preview = document.getElementById(tooltipId);
      if (anchor?.matches(":hover") || preview?.matches(":hover") || anchor?.contains(document.activeElement)) return;
      hide();
    }, 200);
  }, [hide, tooltipId]);

  const show = useCallback(
    (card: string | CardInfo, el: HTMLElement, keyboard = false) => {
      // Closing a top-layer preview exposes a different row under a resting
      // pointer and generates mouseenter without user movement. It must not
      // replace an explicit keyboard preview or undo Escape dismissal.
      if (!keyboard && keyboardActive.current) { deferredPointer.current = { card, el }; return; }
      if (keyboard) { keyboardActive.current = true; deferredPointer.current = null; }
      hide();
      // Touch screens emulate a hover on tap; there the card opens as a sheet
      // instead (CardSheet), so a tap never leaves a stray hover card behind.
      if (!keyboard && typeof window.matchMedia === "function" && window.matchMedia("(hover: none)").matches) return;
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

  useLayoutEffect(() => {
    const el = anchorEl.current;
    if (!shown || !el) return;
    const ids = el.getAttribute("aria-describedby")?.split(/\s+/).filter(Boolean) ?? [];
    el.setAttribute("aria-describedby", [...new Set([...ids, tooltipId])].join(" "));
    return () => {
      const remaining = el.getAttribute("aria-describedby")?.split(/\s+/).filter(id => id && id !== tooltipId) ?? [];
      if (remaining.length) el.setAttribute("aria-describedby", remaining.join(" "));
      else el.removeAttribute("aria-describedby");
    };
  }, [shown, tooltipId]);

  // Anything that could move the anchor or mean the reader has moved on closes it.
  useEffect(() => {
    const onOver = (e: MouseEvent) => {
      const el = anchorEl.current;
      // Also catches a trigger that unmounted under a resting pointer, which never fires mouseleave.
      if (el && !(e.target instanceof Node && (el.contains(e.target) || document.getElementById(tooltipId)?.contains(e.target)))) leave();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.isComposing || e.keyCode === 229 || e.key !== "Escape") return;
      keyboardActive.current = true;
      if (document.getElementById(tooltipId)) {
        e.preventDefault();
        e.stopImmediatePropagation();
      }
      hide();
    };
    const outsidePreview = (e: Event) => {
      // Focusing an offscreen row causes a native scroll after its focus event.
      // Let its delayed preview measure the final position rather than cancel it.
      if (e.type === "scroll" && timer.current !== undefined && keyboardActive.current && anchorEl.current?.contains(document.activeElement)) return;
      if (!(e.target instanceof Node && document.getElementById(tooltipId)?.contains(e.target))) hide();
    };
    const onMove = (e: MouseEvent) => {
      if (!e.movementX && !e.movementY) return;
      keyboardActive.current = false;
      const pending = deferredPointer.current;
      deferredPointer.current = null;
      if (pending && e.target instanceof Node && pending.el.contains(e.target)) show(pending.card, pending.el);
    };
    window.addEventListener("scroll", outsidePreview, true);
    window.addEventListener("resize", hide);
    document.addEventListener("mousedown", outsidePreview, true);
    // Pointer focus fires after mousedown, and can schedule another preview.
    // Activation must cancel it before the row opens its actual inspector.
    document.addEventListener("click", outsidePreview, true);
    document.addEventListener("keydown", onKey, true);
    document.addEventListener("mouseover", onOver);
    document.addEventListener("mousemove", onMove, true);
    return () => {
      window.removeEventListener("scroll", outsidePreview, true);
      window.removeEventListener("resize", hide);
      document.removeEventListener("mousedown", outsidePreview, true);
      document.removeEventListener("click", outsidePreview, true);
      document.removeEventListener("keydown", onKey, true);
      document.removeEventListener("mouseover", onOver);
      document.removeEventListener("mousemove", onMove, true);
      window.clearTimeout(timer.current);
      window.clearTimeout(leaveTimer.current);
      inspectionEpoch.current++;
    };
  }, [hide, leave, show, tooltipId]);

  const inspect = useCallback((card: string | CardInfo) => {
    hide();
    const mine = ++inspectionEpoch.current;
    const route = location.href;
    const invoker = document.activeElement;
    if (typeof card !== "string") setInspected(card);
    else void lookup(card).then(info => {
      if (info && inspectionEpoch.current === mine && location.href === route && invoker?.isConnected) setInspected(info);
    }, () => {});
  }, [hide, lookup]);
  const api = useMemo(() => ({ show, hide, leave, inspect }), [show, hide, leave, inspect]);
  return (
    <Ctx.Provider value={api}>
      {children}
      <Suspense fallback={null}>{inspected && <CardSheet key={inspected.name} card={inspected} sheet onClose={() => { inspectionEpoch.current++; setInspected(null); }} />}
      {shown && createPortal(<HoverCard key={shown.card.name} id={tooltipId} shown={shown} onLeave={leave} />,
        anchorEl.current?.closest("dialog:modal, [role=dialog]") ?? document.body)}</Suspense>
    </Ctx.Provider>
  );
}

const NO_HANDLERS = { onMouseEnter: () => {}, onMouseLeave: () => {}, onFocus: () => {}, onBlur: () => {} };

/**
 * Spread the result on any element that names a card. After the pointer rests
 * for HOVER_DELAY_MS the hover card appears beside it; leaving hides it at
 * once. Given a name it resolves the card through `useCardLookup`; given a
 * CardInfo it shows that after the same delay. Focus uses the same preview;
 * pointer travel into its contents keeps it open and Escape dismisses it.
 */
export function useCardHover(): (card: string | CardInfo) => {
  onMouseEnter: (e: ReactMouseEvent<HTMLElement>) => void;
  onMouseLeave: () => void;
  onFocus: (e: ReactFocusEvent<HTMLElement>) => void;
  onBlur: () => void;
} {
  const api = useContext(Ctx);
  return useCallback(
    (card) =>
      api
        ? {
            onMouseEnter: (e) => api.show(card, e.currentTarget),
            onMouseLeave: api.leave,
            onFocus: (e) => api.show(card, e.currentTarget, true),
            onBlur: api.leave,
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
