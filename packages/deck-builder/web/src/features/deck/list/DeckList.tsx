// The deck as a list (spec §5): commanders first, then the groups in columns,
// then the maybe list. In review mode it shows the deck as it would be if
// every pending item were accepted, drawn in place rather than in a side list,
// so the owner judges a swap by what the list looks like after it.

import { useEffect, useMemo, useRef, type ReactNode } from "react";
import type { ProposalItem } from "../../../api/client.ts";
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
  const matchingCards = matches.active ? matches.cardIds : undefined;
  const matchingAdds = matches.active ? matches.addIds : undefined;
  // The match find-in-deck has stepped to (DeckFilterSummary's "1 of 5").
  // Read-only lists have no find controls, so nothing there is current.
  const ordered = matches.ordered ?? [];
  const findCurrent = matches.active && !readOnly && ordered.length
    ? ordered[Math.min(ui.findIndex, ordered.length - 1)]
    : undefined;

  // Totals always describe the full deck, independent of the highlighted matches.
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

  // Query changes reveal the first match in the existing window/pane scroll
  // container. Leave focus in search so typing and clearing keep working.
  useEffect(() => {
    if (matches.active && !readOnly) scrollToFirstMatch(scroll.current);
  }, [ui.searchTerm, ui.tagIds, ui.tagMatch, ui.tagInverted, readOnly]);

  // Stepping through matches (Enter, Shift+Enter, previous/next) reveals the
  // new current match. Focus stays where it was, normally the find box.
  useEffect(() => {
    if (ui.findStep && !readOnly) scrollToFindCurrent(scroll.current);
  }, [ui.findStep, readOnly]);

  const nothingToShow = state.cards.length === 0 && !(reviewing && overlay.addsByGroup.size > 0);

  return (
    <div ref={scroll} data-deck-list={!readOnly ? "true" : undefined} className={readOnly ? styles.flow : styles.scroll}>
      {nothingToShow ? (
        <EmptyState title="No cards yet">
          {emptyAction ?? "This deck has no cards in it yet."}
        </EmptyState>
      ) : (
        <>
          <div className={styles.cols}>
            <GroupBlock
              matchingCards={matchingCards}
              findCurrent={findCurrent}
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
                matchingCards={matchingCards}
                matchingAdds={matchingAdds}
                findCurrent={findCurrent}
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
                matchingCards={matchingCards}
                findCurrent={findCurrent}
                title="Companion"
                cost={companionCost}
                cards={layout.companions}
                cuts={cuts}
                current={current}
                marks={marks}
              />
            )}
          </div>
          <MaybeList cards={layout.maybe} matchingCards={matchingCards} findCurrent={findCurrent} />
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

export function scrollToFirstMatch(list: HTMLElement | null): void {
  // A secondary contributor can be inside a closed details element. Its main
  // row remains in the list; don't open or collapse any groups while searching.
  const first = Array.from(list?.querySelectorAll<HTMLElement>('[data-filter-match="true"]') ?? [])
    .find(row => row.getClientRects().length > 0);
  first?.scrollIntoView({ block: "nearest", inline: "nearest" });
}

/** Like a browser's find: a match already in view stays put; one out of view
 *  is centred, so the rows around it come with it. */
export function scrollToFindCurrent(list: HTMLElement | null, reducedMotion = prefersReducedMotion()): void {
  if (!list) return;
  const row = Array.from(list.querySelectorAll<HTMLElement>('[data-find-current="true"]'))
    .find(row => row.getClientRects().length > 0);
  if (!row) return;
  // The desktop list scrolls itself; below 1100 px the window scrolls it.
  const view = list.scrollHeight > list.clientHeight ? list.getBoundingClientRect() : { top: 0, bottom: innerHeight };
  const box = row.getBoundingClientRect();
  if (box.top >= view.top && box.bottom <= view.bottom) return;
  row.scrollIntoView({ block: "center", inline: "nearest", behavior: reducedMotion ? "auto" : "smooth" });
}

function prefersReducedMotion(): boolean {
  return typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
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
