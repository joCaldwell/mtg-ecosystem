import { useEffect, useRef, useState, type PointerEvent, type KeyboardEvent } from "react";

type Side = "left" | "right";
const settings = {
  left: { property: "--rail", key: "deck.railWidth", initial: 280, min: 250 },
  right: { property: "--side-w", key: "deck.sideWidth", initial: 372, min: 306 },
};
const changed = "deck-sidebar-resize";
const wide = () => window.matchMedia("(min-width: 1401px)").matches;
const readWidth = (side: Side) =>
  parseFloat(getComputedStyle(document.documentElement).getPropertyValue(settings[side].property)) ||
  settings[side].initial;

// In split-screen mode both tools share the left rail and therefore its width.
function limits(side: Side) {
  const active = wide() ? side : "right";
  const other = wide() ? readWidth(side === "left" ? "right" : "left") : 0;
  const min = settings[active].min;
  return { active, min, max: Math.max(min, Math.min(active === "left" ? 520 : Infinity, window.innerWidth - other - 400)) };
}

export function SidebarResize({ side }: { side: Side }) {
  const drag = useRef<{ x: number; width: number; direction: number } | null>(null);
  const [value, setValue] = useState<number>(settings[side].initial);
  const [bounds, setBounds] = useState({ min: settings[side].min, max: settings[side].initial });

  function setWidth(requested: number, commit = false) {
    const { active, min, max } = limits(side);
    const width = Math.min(max, Math.max(min, Math.round(requested)));
    document.documentElement.style.setProperty(settings[active].property, `${width}px`);
    if (commit) localStorage.setItem(settings[active].key, String(width));
    window.dispatchEvent(new Event(changed));
  }

  useEffect(() => {
    const config = settings[side];
    const stored = Number(localStorage.getItem(config.key));
    document.documentElement.style.setProperty(config.property, `${Number.isFinite(stored) && stored > 0 ? stored : config.initial}px`);
    const sync = () => {
      const { active, min, max } = limits(side);
      setValue(readWidth(active));
      setBounds({ min, max });
    };
    const resize = () => {
      // Only one handle owns the shared rail's viewport resizing. Preserve the
      // saved preference so returning to a larger window restores its width.
      if (!wide() && side === "left") return sync();
      const saved = Number(localStorage.getItem(config.key));
      setWidth(Number.isFinite(saved) && saved > 0 ? saved : config.initial);
    };
    window.addEventListener(changed, sync);
    window.addEventListener("resize", resize);
    resize();
    return () => {
      window.removeEventListener(changed, sync);
      window.removeEventListener("resize", resize);
      if (drag.current) document.body.classList.remove("col-resizing");
    };
  }, [side]);

  function start(e: PointerEvent<HTMLDivElement>) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.currentTarget.focus();
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { x: e.clientX, width: readWidth(limits(side).active), direction: side === "left" || !wide() ? 1 : -1 };
    document.body.classList.add("col-resizing");
  }

  function finish() {
    if (!drag.current) return;
    drag.current = null;
    document.body.classList.remove("col-resizing");
    setWidth(readWidth(limits(side).active), true);
  }

  function onKeyDown(e: KeyboardEvent) {
    const direction = side === "left" || !wide() ? 1 : -1;
    const step = (e.shiftKey ? 64 : 16) * direction;
    const current = readWidth(limits(side).active);
    if (e.key === "ArrowRight") setWidth(current + step, true);
    else if (e.key === "ArrowLeft") setWidth(current - step, true);
    else if (e.key === "Home") setWidth(limits(side).min, true);
    else if (e.key === "End") setWidth(limits(side).max, true);
    else if (e.key === "Enter") setWidth(settings[limits(side).active].initial, true);
    else return;
    e.preventDefault();
  }

  return (
    <div
      className={`sidebar-resize sidebar-resize-${side}`}
      role="separator"
      aria-label={side === "left" ? "Resize deck sidebar" : "Resize search and memories sidebar"}
      aria-orientation="vertical"
      aria-valuemin={bounds.min}
      aria-valuemax={bounds.max}
      aria-valuenow={value}
      aria-valuetext={`${value} pixels`}
      title="Drag or use arrow keys to resize. Double-click or press Enter to reset."
      tabIndex={0}
      onPointerDown={start}
      onPointerMove={(e) => {
        if (drag.current) setWidth(drag.current.width + (e.clientX - drag.current.x) * drag.current.direction);
      }}
      onPointerUp={finish}
      onPointerCancel={finish}
      onLostPointerCapture={finish}
      onDoubleClick={() => setWidth(settings[limits(side).active].initial, true)}
      onKeyDown={onKeyDown}
    />
  );
}
