// The deck as a list (spec §5): commanders first, then the groups in columns,
// then the maybe list. In review mode it shows the deck as it would be if
// every pending item were accepted, drawn in place rather than in a side list,
// so the owner judges a swap by what the list looks like after it.

import { useEffect, useRef } from "react";
import { paperMarksUseful } from "../../../domain/marks.ts";
import { EmptyState, Text } from "../../../ui/index.ts";
import { useDeckState } from "../DeckStore.tsx";
import { useDeckUi } from "../DeckUi.tsx";
import type { DeckViewModel } from "../useDeckView.ts";
import { GroupBlock } from "./GroupBlock.tsx";
import { MARK_MEANING, Mark, type MarkKind } from "./Mark.tsx";
import { MaybeList } from "./MaybeList.tsx";
import styles from "./DeckList.module.css";

const LEGEND: Array<[MarkKind, string]> = [
  ["memory", "memory linked"],
  ["ruling", "ruled on before"],
  ["paper", "not in paper"],
];

const NONE = new Set<string>();

export function DeckList({ view }: { view: DeckViewModel }) {
  const state = useDeckState();
  const paperMarks = paperMarksUseful(state);
  const ui = useDeckUi();
  const scroll = useRef<HTMLDivElement>(null);
  const { layout, overlay, marks, units } = view;
  const reviewing = ui.reviewing;

  // Same clamp as useReview, so the outline and the panel agree on the unit.
  const unit = reviewing && units.length ? units[Math.min(ui.cursor, units.length - 1)] : null;
  const current = unit ? new Set(unit.items.map((i) => i.oracle_id)) : NONE;
  const cuts = reviewing ? overlay.cuts : undefined;

  // J and K move the cursor; keep the decision it lands on in view.
  const unitKey = unit?.key;
  useEffect(() => {
    if (!unitKey) return;
    scroll.current?.querySelector('[data-current="true"]')?.scrollIntoView({ block: "nearest" });
  }, [unitKey]);

  const nothingToShow = state.cards.length === 0 && !(reviewing && overlay.addsByGroup.size > 0);

  return (
    <div ref={scroll} className={styles.scroll}>
      {nothingToShow ? (
        <EmptyState title="No cards yet">
          Use the bar above to add the first one, or import a list from the More menu.
        </EmptyState>
      ) : (
        <>
          <div className={styles.cols}>
            <GroupBlock
              paperMarks={paperMarks}
              title="Commander"
              cards={layout.commanders}
              cuts={cuts}
              current={current}
              marks={marks}
              bold
              empty={
                <p className={styles.hint}>
                  No commander yet. Search for a legendary creature above and add it as the commander.
                </p>
              }
            />
            {layout.groups.map((g) => (
              <GroupBlock
                paperMarks={paperMarks}
                key={g.key}
                title={g.title}
                count={g.count}
                target={g.target}
                status={g.status}
                delta={g.delta}
                projected={reviewing ? overlay.projected.get(g.key) : undefined}
                cards={g.cards}
                adds={reviewing ? overlay.addsByGroup.get(g.key) : undefined}
                cuts={cuts}
                current={current}
                marks={marks}
                empty={<p className={styles.hint}>Nothing filed here.</p>}
              />
            ))}
            {layout.companions.length > 0 && (
              <GroupBlock
                paperMarks={paperMarks}
                title="Companion"
                cards={layout.companions}
                cuts={cuts}
                current={current}
                marks={marks}
              />
            )}
          </div>
          <MaybeList cards={layout.maybe} slots={state.slots} />
        </>
      )}
      <p className={styles.legend}>
        {LEGEND.map(([kind, label]) => (
          <Text key={kind} tone="faint" size="xs" className={styles.legendItem} title={MARK_MEANING[kind]}>
            <Mark kind={kind} /> {label}
          </Text>
        ))}
        <Text tone="faint" size="xs">
          Ctrl-click to select several
        </Text>
      </p>
    </div>
  );
}
