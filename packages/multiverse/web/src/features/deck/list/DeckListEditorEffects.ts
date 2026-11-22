import { useEffect, useLayoutEffect, useRef, type RefObject } from "react";
import { useDeckUi } from "../DeckUi.tsx";
import type { DeckViewModel } from "../useDeckView.ts";

export interface DeckListEditorEffectsProps {
  scroll: RefObject<HTMLDivElement | null>;
  unit: DeckViewModel["units"][number] | null;
  filterActive: boolean;
}

/** Review and filter reveals belong to the editor, not its read-only lists. */
export function DeckListEditorEffects({ scroll, unit, filterActive }: DeckListEditorEffectsProps) {
  const ui = useDeckUi();

  // J and K move the cursor; keep the decision it lands on in view.
  // Only a change of decision scrolls, never an unrelated re-render.
  const unitKey = unit?.key;
  useEffect(() => {
    if (!unitKey) return;
    scrollToReviewCard(scroll.current, unit?.items[0]?.oracle_id);
  }, [unitKey]);

  // Where the unfiltered list was scrolled, kept up to date only while no
  // filter is active, so clearing one returns the owner to it. The desktop
  // list scrolls itself; below 1100 px the window scrolls it.
  const unfiltered = useRef({ list: 0, window: 0, header: 0 });
  const filtering = useRef(false);
  useEffect(() => {
    const list = scroll.current;
    if (!list) return;
    const page = list.closest<HTMLElement>("[data-deck-page]");
    const remember = () => { if (!filtering.current) unfiltered.current = { list: list.scrollTop, window: scrollY, header: page?.scrollTop ?? 0 }; };
    remember();
    page?.addEventListener("scroll", remember, { passive: true });
    list.addEventListener("scroll", remember, { passive: true });
    addEventListener("scroll", remember, { passive: true });
    return () => { page?.removeEventListener("scroll", remember); list.removeEventListener("scroll", remember); removeEventListener("scroll", remember); };
  }, [scroll]);

  // Each filter change shows its results from the top; clearing the filter
  // restores the unfiltered position. Focus stays in the find box. Layout
  // effects, so the restore lands before the browser paints the full list.
  useLayoutEffect(() => {
    const was = filtering.current;
    filtering.current = filterActive;
    if (filterActive) revealFilterResults(scroll.current);
    else if (was) restoreScroll(scroll.current, unfiltered.current);
  }, [filterActive, ui.searchTerm, ui.tagIds, ui.tagMatch, ui.tagInverted, scroll]);

  return null;
}

/** Show a filter's results from their start: the desktop list scrolls back
 *  to its top; below 1100 px, where the window scrolls, the list's top is
 *  brought up under the sticky filter row if it has gone above it. */
function revealFilterResults(list: HTMLElement | null): void {
  if (!list) return;
  list.scrollTop = 0;
  list.dispatchEvent(new Event("deck-workspace-reveal"));
  const offset = parseFloat(getComputedStyle(list).scrollMarginTop) || 0;
  if (list.getBoundingClientRect().top < offset) list.scrollIntoView({ block: "start", inline: "nearest" });
}

/** Return to where the unfiltered list was. */
function restoreScroll(list: HTMLElement | null, to: { list: number; window: number; header?: number }): void {
  if (!list) return;
  const page = list.closest<HTMLElement>("[data-deck-page]");
  if (page && to.header !== undefined) page.scrollTop = to.header;
  list.scrollTop = to.list;
  if (scrollY !== to.window) scrollTo({ top: to.window, behavior: "instant" });
}

interface Span { top: number; bottom: number }

/** How far to scroll so a decision's rows sit inside `view` with `gap` (one
 *  row) clear above and below, so their group's context and the next row
 *  show too. All of them when they fit together with that clearance,
 *  otherwise the named row (or the first). Rows already clear stay put;
 *  anything that has to move is centred. */
function reviewRevealDelta(view: Span, rows: Span[], named: Span | undefined, gap: number): number {
  if (!rows.length) return 0;
  const top = Math.min(...rows.map(r => r.top));
  const bottom = Math.max(...rows.map(r => r.bottom));
  const target = bottom - top + 2 * gap <= view.bottom - view.top ? { top, bottom } : named ?? rows.find(r => r.top === top)!;
  if (target.top - gap >= view.top && target.bottom + gap <= view.bottom) return 0;
  return Math.round((target.top + target.bottom) / 2 - (view.top + view.bottom) / 2);
}

/** Reveal a decision's rows (reviewRevealDelta). A swap's add and cut often
 *  sit in different groups. */
function scrollToReviewCard(list: HTMLElement | null, oracleId: string | undefined): void {
  const rows = Array.from(list?.querySelectorAll<HTMLElement>('[data-current="true"]') ?? [])
    .filter(row => row.getClientRects().length > 0);
  if (list && rows.length) revealRows(list, rows, rows.find(row => row.dataset.oracleId === oracleId));
}

/** Bring one card's row in the editable list into view, the same way, and
 *  focus it, so closing its details returns there. Nothing happens when a
 *  filter hides it. */
export function revealCardRow(root: ParentNode, oracleId: string): void {
  const list = root.querySelector<HTMLElement>("[data-deck-list]");
  const row = list?.querySelector<HTMLElement>(`[data-oracle-id="${CSS.escape(oracleId)}"]`);
  if (!list || !row || !row.getClientRects().length) return;
  revealRows(list, [row], row);
  (row.matches("button") ? row : row.querySelector("button"))?.focus({ preventScroll: true });
}

/** Scroll whichever holds the rows: the desktop list pane, or below 1100 px
 *  the window, under its sticky bars (the rows' scroll margin). */
function revealRows(list: HTMLElement, rows: HTMLElement[], named: HTMLElement | undefined): void {
  // Explicit reveals may need the whole viewport immediately. Measure after
  // the fold so centring uses the final viewport rather than the old hero.
  list.dispatchEvent(new Event("deck-workspace-reveal"));
  const boxes = rows.map(row => row.getBoundingClientRect());
  const namedBox = named ? boxes[rows.indexOf(named)] : undefined;
  const style = getComputedStyle(list);
  if (style.overflowY === "auto" || style.overflowY === "scroll") {
    const box = list.getBoundingClientRect();
    const view = { top: box.top + (parseFloat(style.scrollPaddingTop) || 0), bottom: box.top + list.clientHeight };
    list.scrollTop += reviewRevealDelta(view, boxes, namedBox, boxes[0].height);
  } else {
    const view = { top: parseFloat(getComputedStyle(rows[0]).scrollMarginTop) || 0, bottom: innerHeight };
    const delta = reviewRevealDelta(view, boxes, namedBox, boxes[0].height);
    if (delta) scrollBy({ top: delta, behavior: "instant" });
  }
}
