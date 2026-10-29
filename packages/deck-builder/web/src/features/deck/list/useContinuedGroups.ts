// Where a long group carries on into the next column of the list.
//
// The list uses native CSS columns, so the browser alone decides where a long
// group breaks. Each long group reserves --group-cont-h at the top of every
// column fragment (`box-decoration-break: clone` repeats its top padding), and
// this hook reads where those fragments start so the list can draw the
// group's name there. The labels sit in that reserved space, outside the
// columns' flow, so drawing them cannot move a break: no measure-draw loop.
// Browsers without cloned fragment padding get no labels and lose nothing.
//
// It measures only when the columns change size or content, not per render.

import { useLayoutEffect, useState } from "react";

export interface ContinuedGroup {
  key: string;
  title: string;
  top: number;
  left: number;
  width: number;
}

const NONE: ContinuedGroup[] = [];

const supported = () => typeof CSS !== "undefined" && CSS.supports("box-decoration-break", "clone");

function measure(cols: HTMLElement, frame: HTMLElement): ContinuedGroup[] {
  const origin = frame.getBoundingClientRect();
  const found: ContinuedGroup[] = [];
  cols.querySelectorAll<HTMLElement>("[data-continues]").forEach((group, g) => {
    // One rect per column fragment; the first holds the real header.
    const fragments = Array.from(group.getClientRects());
    fragments.slice(1).forEach((r, i) => {
      found.push({
        key: `${g}-${i}`,
        title: group.dataset.continues ?? "",
        top: Math.round(r.top - origin.top),
        left: Math.round(r.left - origin.left),
        width: Math.round(r.width),
      });
    });
  });
  return found;
}

const same = (a: ContinuedGroup[], b: ContinuedGroup[]) =>
  a.length === b.length && a.every((x, i) => x.key === b[i].key && x.top === b[i].top && x.left === b[i].left && x.width === b[i].width);

/** A ref for the columns element, and the continuation labels for its long
 *  groups, positioned against its parent, where the list draws them. A
 *  callback ref, because the columns unmount while no card matches a filter. */
export function useContinuedGroups(): [(el: HTMLElement | null) => void, ContinuedGroup[]] {
  const [el, setEl] = useState<HTMLElement | null>(null);
  const [labels, setLabels] = useState<ContinuedGroup[]>(NONE);
  useLayoutEffect(() => {
    const frame = el?.parentElement;
    if (!el || !frame || !supported() || typeof ResizeObserver === "undefined") return;
    let pending = 0;
    const update = () => {
      pending = 0;
      const next = measure(el, frame);
      setLabels((prev) => (same(prev, next) ? prev : next.length ? next : NONE));
    };
    const schedule = () => { if (!pending) pending = requestAnimationFrame(update); };
    update();
    // Width changes (window, panel drag) and height changes (rows, disclosures,
    // fonts) both move breaks; a row swap that keeps the height is caught by
    // the mutation observer.
    const resize = new ResizeObserver(schedule);
    resize.observe(el);
    const mutate = new MutationObserver(schedule);
    mutate.observe(el, { childList: true, subtree: true });
    return () => { resize.disconnect(); mutate.disconnect(); if (pending) cancelAnimationFrame(pending); setLabels(NONE); };
  }, [el]);
  return [setEl, labels];
}
