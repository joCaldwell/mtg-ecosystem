// The deck as a list (spec §5): commanders first, then the groups in columns,
// then the maybe list, then the tokens the deck makes. In review mode it
// shows the deck as it would be if every pending item were accepted, drawn in
// place rather than in a side list, so the owner judges a swap by what the
// list looks like after it. A find query or tag filter hides the rows, groups
// and maybe cards that do not match, and the tokens no matching card makes;
// totals elsewhere keep describing the whole deck.

import { useEffect, useLayoutEffect, useMemo, useRef, type ReactNode } from "react";
import type { ProposalItem } from "../../../api/client.ts";
import { matchCount, noMatchesLine } from "../../../domain/deckSearch.ts";
import { costOf } from "../../../domain/price.ts";
import { EmptyState, Text } from "../../../ui/index.ts";
import { useDeckList } from "../DeckListStore.tsx";
import { useDeckUi } from "../DeckUi.tsx";
import type { DeckViewModel } from "../useDeckView.ts";
import { GroupBlock } from "./GroupBlock.tsx";
import { MARK_LABEL, Mark, type MarkKind } from "./Mark.tsx";
import { MaybeList } from "./MaybeList.tsx";
import { TokenList } from "./TokenList.tsx";
import { useContinuedGroups } from "./useContinuedGroups.ts";
import styles from "./DeckList.module.css";

const LEGEND: MarkKind[] = ["memory", "decision"];

const NONE = new Set<string>();
const NO_ADDS = new Map<string, ProposalItem[]>();

export function DeckList({ view, emptyAction }: { view: DeckViewModel; emptyAction?: ReactNode }) {
  const { state, readOnly, viewer } = useDeckList();
  const ui = useDeckUi();
  const scroll = useRef<HTMLDivElement>(null);
  const [cols, continued] = useContinuedGroups();
  const { layout, overlay, marks, units, matches } = view;
  const reviewing = ui.reviewing;
  const addsByGroup = reviewing ? overlay.addsByGroup : NO_ADDS;
  const filter = matches.active ? matches : undefined;

  // Totals always describe the full deck, independent of the filter.
  const commanderCost = useMemo(() => costOf(view.layout.commanders), [view.layout.commanders]);
  const companionCost = useMemo(() => costOf(view.layout.companions), [view.layout.companions]);

  // Same clamp as useReview, so the outline and the panel agree on the unit.
  const unit = reviewing && units.length ? units[Math.min(ui.cursor, units.length - 1)] : null;
  const current = unit ? new Set(unit.items.map((i) => i.oracle_id)) : NONE;
  const cuts = reviewing ? overlay.cuts : undefined;

  // J and K move the cursor; keep the decision it lands on in view. A
  // read-only list (the phone view) has no keyboard review, and scrolling
  // there would carry the reader past the decisions drawn above the list.
  // Only a change of decision scrolls, never an unrelated re-render.
  const unitKey = unit?.key;
  useEffect(() => {
    if (!unitKey || readOnly) return;
    scrollToReviewCard(scroll.current, unit?.items[0]?.oracle_id);
  }, [unitKey, readOnly]);

  // Where the unfiltered list was scrolled, kept up to date only while no
  // filter is active, so clearing one returns the owner to it. The desktop
  // list scrolls itself; below 1100 px the window scrolls it.
  const unfiltered = useRef({ list: 0, window: 0 });
  const filtering = useRef(false);
  useEffect(() => {
    const list = scroll.current;
    if (!list || readOnly) return;
    const remember = () => { if (!filtering.current) unfiltered.current = { list: list.scrollTop, window: scrollY }; };
    remember();
    list.addEventListener("scroll", remember, { passive: true });
    addEventListener("scroll", remember, { passive: true });
    return () => { list.removeEventListener("scroll", remember); removeEventListener("scroll", remember); };
  }, [readOnly]);

  // Each filter change shows its results from the top; clearing the filter
  // restores the unfiltered position. Focus stays in the find box. Layout
  // effects, so the restore lands before the browser paints the full list.
  useLayoutEffect(() => {
    const was = filtering.current;
    filtering.current = matches.active;
    if (readOnly) return;
    if (matches.active) revealFilterResults(scroll.current);
    else if (was) restoreScroll(scroll.current, unfiltered.current);
  }, [matches.active, ui.searchTerm, ui.tagIds, ui.tagMatch, ui.tagInverted, readOnly]);

  const nothingToShow = state.cards.length === 0 && !(reviewing && overlay.addsByGroup.size > 0);
  const nothingMatches = !!filter && matchCount(filter) === 0;

  return (
    <div ref={scroll} data-deck-list={!readOnly ? "true" : undefined} className={readOnly ? styles.flow : styles.scroll}>
      {nothingToShow ? (
        <EmptyState title="No cards yet">
          {emptyAction ?? "This deck has no cards in it yet."}
        </EmptyState>
      ) : nothingMatches ? (
        <p className={styles.noMatches}>{noMatchesLine(ui.searchTerm, ui.tagIds.length > 0)}</p>
      ) : (
        <>
          <div className={styles.frame}>
            <div ref={cols} className={styles.cols}>
              <GroupBlock
                filter={filter}
                title="Commander"
                cost={commanderCost}
                cards={layout.commanders}
                cuts={cuts}
                current={current}
                marks={marks}
                bold
                empty={
                  <p className={styles.hint}>
                    {readOnly ? "No commander yet." : "No commander yet. Search for a legendary creature above and add it as the commander."}
                  </p>
                }
              />
              {layout.groups.map((g) => (
                <GroupBlock
                  filter={filter}
                  key={g.key}
                  title={g.title}
                  count={g.count}
                  target={g.target}
                  status={g.status}
                  delta={g.delta}
                  projected={reviewing ? overlay.projected.get(g.key) : undefined}
                  cost={g.cost}
                  cards={g.cards}
                  secondaryCards={g.secondaryCards}
                  commandZoneCards={g.commandZoneCards}
                  primaryCount={g.primaryCount}
                  secondaryCount={g.secondaryCount}
                  adds={addsByGroup.get(g.key)}
                  cuts={cuts}
                  current={current}
                  marks={marks}
                  empty={<p className={styles.hint}>Nothing filed here.</p>}
                />
              ))}
              {layout.companions.length > 0 && (
                <GroupBlock
                  filter={filter}
                  title="Companion"
                  cost={companionCost}
                  cards={layout.companions}
                  cuts={cuts}
                  current={current}
                  marks={marks}
                />
              )}
            </div>
            {/* A long group's name at the top of each column it continues into.
                Not a heading: the group already has one in the outline. */}
            {continued.map((c) => (
              <div key={c.key} className={styles.continued} style={{ top: c.top, left: c.left, width: c.width }} aria-hidden="true">
                <span className={styles.continuedTitle}>{c.title}</span> <span className={styles.continuedMark}>(cont.)</span>
              </div>
            ))}
          </div>
          <MaybeList cards={layout.maybe} filter={filter} />
          <TokenList filter={filter} />
        </>
      )}
      {/* The markers are the owner's workflow; a viewer's list has none. */}
      {!viewer && <p className={styles.legend}>
        {LEGEND.map((kind) => (
          <Text key={kind} tone="faint" size="xs" className={styles.legendItem}>
            <Mark kind={kind} decorative /> {MARK_LABEL[kind]}
          </Text>
        ))}
        {!readOnly && (
          <Text tone="faint" size="xs">
            Ctrl-click to select several
          </Text>
        )}
      </p>}
    </div>
  );
}

/** Show a filter's results from their start: the desktop list scrolls back
 *  to its top; below 1100 px, where the window scrolls, the list's top is
 *  brought up under the sticky filter row if it has gone above it. */
export function revealFilterResults(list: HTMLElement | null): void {
  if (!list) return;
  list.scrollTop = 0;
  const offset = parseFloat(getComputedStyle(list).scrollMarginTop) || 0;
  if (list.getBoundingClientRect().top < offset) list.scrollIntoView({ block: "start", inline: "nearest" });
}

/** Return to where the unfiltered list was. */
export function restoreScroll(list: HTMLElement | null, to: { list: number; window: number }): void {
  if (!list) return;
  list.scrollTop = to.list;
  if (scrollY !== to.window) scrollTo({ top: to.window, behavior: "instant" });
}

interface Span { top: number; bottom: number }

/** How far to scroll so a decision's rows sit inside `view` with `gap` (one
 *  row) clear above and below, so their group's context and the next row
 *  show too. All of them when they fit together with that clearance,
 *  otherwise the named row (or the first). Rows already clear stay put;
 *  anything that has to move is centred. */
export function reviewRevealDelta(view: Span, rows: Span[], named: Span | undefined, gap: number): number {
  if (!rows.length) return 0;
  const top = Math.min(...rows.map(r => r.top));
  const bottom = Math.max(...rows.map(r => r.bottom));
  const target = bottom - top + 2 * gap <= view.bottom - view.top ? { top, bottom } : named ?? rows.find(r => r.top === top)!;
  if (target.top - gap >= view.top && target.bottom + gap <= view.bottom) return 0;
  return Math.round((target.top + target.bottom) / 2 - (view.top + view.bottom) / 2);
}

/** Reveal a decision's rows (reviewRevealDelta). A swap's add and cut often
 *  sit in different groups. */
export function scrollToReviewCard(list: HTMLElement | null, oracleId: string | undefined): void {
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
