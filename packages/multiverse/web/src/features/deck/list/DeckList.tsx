// The deck as a list (spec §5): commanders first, then the groups in columns,
// then the maybe list, then the tokens the deck makes. In review mode it
// shows the deck as it would be if every pending item were accepted, drawn in
// place rather than in a side list, so the owner judges a swap by what the
// list looks like after it. A find query or tag filter hides the rows, groups
// and maybe cards that do not match, and the tokens no matching card makes;
// totals elsewhere keep describing the whole deck.

import { useMemo, useRef, type ComponentType, type ReactNode } from "react";
import type { ProposalItem } from "../../../api/client.ts";
import { matchCount, noMatchesLine } from "../../../domain/deckSearch.ts";
import { costOf } from "../../../domain/price.ts";
import { EmptyState, Text } from "../../../ui/index.ts";
import { useDeckList } from "../DeckListStore.tsx";
import { useDeckUi } from "../DeckUi.tsx";
import type { DeckViewModel } from "../useDeckView.ts";
import type { DeckListEditorEffectsProps } from "./DeckListEditorEffects.ts";
import { GroupHeader } from "./GroupHeader.tsx";
import { GroupBlock } from "./GroupBlock.tsx";
import { MARK_LABEL, Mark, type MarkKind } from "./Mark.tsx";
import { MaybeList } from "./MaybeList.tsx";
import { TokenList } from "./TokenList.tsx";
import { useContinuedGroups } from "./useContinuedGroups.ts";
import styles from "./DeckList.module.css";

const LEGEND: MarkKind[] = ["memory", "decision"];

const NONE = new Set<string>();
const NO_ADDS = new Map<string, ProposalItem[]>();

export function DeckList({ view, emptyAction, EditorEffects }: {
  view: DeckViewModel;
  emptyAction?: ReactNode;
  EditorEffects?: ComponentType<DeckListEditorEffectsProps>;
}) {
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

  // The legend names only the markers drawn on the rows now in the list.
  const legend = useMemo(() => {
    const ids = new Set<string>();
    const card = (id: string) => { if (!filter || filter.cardIds.has(id)) ids.add(id); };
    for (const c of [...layout.commanders, ...layout.companions]) card(c.oracle_id);
    for (const g of layout.groups) {
      for (const c of g.cards) card(c.oracle_id);
      if (!filter) for (const c of [...(g.secondaryCards ?? []), ...(g.commandZoneCards ?? [])]) ids.add(c.oracle_id);
    }
    for (const adds of addsByGroup.values())
      for (const item of adds) if (!filter || filter.addIds.has(item.id)) ids.add(item.oracle_id);
    const shown = [...ids].map((id) => marks.get(id));
    return LEGEND.filter((kind) => shown.some((m) => (kind === "memory" ? m?.memories : m?.decisions)));
  }, [layout, addsByGroup, filter, marks]);

  const nothingToShow = state.cards.length === 0 && !(reviewing && overlay.addsByGroup.size > 0);
  const nothingMatches = !!filter && matchCount(filter) === 0;

  return <>
    <div
      ref={scroll}
      data-deck-list={!readOnly ? "true" : undefined}
      role={readOnly ? undefined : "region"}
      aria-label={readOnly ? undefined : "Deck list"}
      tabIndex={readOnly ? undefined : -1}
      className={readOnly ? styles.flow : styles.scroll}
    >
      <div data-deck-list-content className={styles.content}>
        {nothingToShow ? (
          <EmptyState title="No cards yet">
            {emptyAction ?? "This deck has no cards in it yet."}
          </EmptyState>
        ) : nothingMatches ? (
          <p className={styles.noMatches}>{noMatchesLine(ui.searchTerm, ui.tagIds.length > 0)}</p>
        ) : (
          <>
            <div className={styles.frame}>
              <GroupHeader title="Main deck" board />
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
        {!viewer && !nothingToShow && !nothingMatches && legend.length > 0 && <p className={styles.legend}>
          {legend.map((kind) => (
            <Text key={kind} tone="faint" size="xs" className={styles.legendItem}>
              <Mark kind={kind} decorative /> {MARK_LABEL[kind]}
            </Text>
          ))}
        </p>}
      </div>
    </div>
    {EditorEffects && <EditorEffects scroll={scroll} unit={unit} filterActive={matches.active} />}
  </>;
}
