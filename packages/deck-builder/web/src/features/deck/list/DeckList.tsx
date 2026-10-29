// The deck as a list (spec §5): commanders first, then the groups in columns,
// then the maybe list. In review mode it shows the deck as it would be if
// every pending item were accepted, drawn in place rather than in a side list,
// so the owner judges a swap by what the list looks like after it. A find
// query or tag filter hides the rows, groups and maybe cards that do not
// match; totals elsewhere keep describing the whole deck.

import { useEffect, useLayoutEffect, useMemo, useRef, type ReactNode } from "react";
import type { ProposalItem } from "../../../api/client.ts";
import { matchCount, noMatchesLine } from "../../../domain/deckSearch.ts";
import { costOf } from "../../../domain/price.ts";
import { EmptyState, Text } from "../../../ui/index.ts";
import { useDeckList } from "../DeckListStore.tsx";
import { useDeckUi } from "../DeckUi.tsx";
import type { DeckViewModel } from "../useDeckView.ts";
import { GroupBlock } from "./GroupBlock.tsx";
import { MARK_MEANING, Mark, type MarkKind } from "./Mark.tsx";
import { MaybeList } from "./MaybeList.tsx";
import styles from "./DeckList.module.css";

const LEGEND: Array<[MarkKind, string]> = [
  ["memory", "memory linked"],
  ["decision", "decided before"],
];

const NONE = new Set<string>();
const NO_ADDS = new Map<string, ProposalItem[]>();

export function DeckList({ view, emptyAction }: { view: DeckViewModel; emptyAction?: ReactNode }) {
  const { state, readOnly } = useDeckList();
  const ui = useDeckUi();
  const scroll = useRef<HTMLDivElement>(null);
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
          <div className={styles.cols}>
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
          <MaybeList cards={layout.maybe} filter={filter} />
        </>
      )}
      <p className={styles.legend}>
        {LEGEND.map(([kind, label]) => (
          <Text key={kind} tone="faint" size="xs" className={styles.legendItem} title={MARK_MEANING[kind]}>
            <Mark kind={kind} /> {label}
          </Text>
        ))}
        {!readOnly && (
          <Text tone="faint" size="xs">
            Ctrl-click to select several
          </Text>
        )}
      </p>
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

/** Reveal a decision's rows: all of them when they fit in the scroller's view
 *  together, otherwise from its first row (the named card, else the topmost).
 *  A swap's add and cut often sit in different groups. */
export function scrollToReviewCard(list: HTMLElement | null, oracleId: string | undefined): void {
  const rows = Array.from(list?.querySelectorAll<HTMLElement>('[data-current="true"]') ?? [])
    .filter(row => row.getClientRects().length > 0);
  if (!rows.length) return;
  const top = (row: HTMLElement) => row.getBoundingClientRect().top;
  const ordered = [...rows].sort((a, b) => top(a) - top(b));
  const first = ordered[0];
  const last = ordered[ordered.length - 1];
  // Revealing the lowest row and then the highest, both "nearest", leaves both
  // in view when they fit, and the highest in view when they do not.
  if (last !== first) last.scrollIntoView({ block: "nearest", inline: "nearest" });
  first.scrollIntoView({ block: "nearest", inline: "nearest" });
  // Keep the named card in view if the span pushed it out.
  const named = rows.find(row => row.dataset.oracleId === oracleId);
  if (named && named !== first) named.scrollIntoView({ block: "nearest", inline: "nearest" });
}
