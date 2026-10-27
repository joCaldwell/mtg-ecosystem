import { useRef } from "react";
import { Button, Icon, IconButton, Segmented, Text, TextField } from "../../ui/index.ts";
import type { SortBy } from "../../domain/cards.ts";
import type { GroupBy } from "../../domain/grouping.ts";
import { AddBar } from "./AddBar.tsx";
import { DeckTagFilter } from "./DeckTagFilter.tsx";
import { useDeckState } from "./DeckStore.tsx";
import { useDeckUi } from "./DeckUi.tsx";
import { useSharedDeckView } from "./DeckViewContext.ts";
import styles from "./DeckToolbar.module.css";

const GROUP_OPTIONS: Array<{ value: GroupBy; label: string }> = [
  { value: "slot", label: "By slot" },
  { value: "type", label: "By type" },
];

const SORT_OPTIONS: Array<{ value: SortBy; label: string; title: string }> = [
  { value: "curve", label: "Mana value", title: "Cheapest to cast first, then by name" },
  { value: "name", label: "Name", title: "A to Z" },
  { value: "price", label: "Price", title: "Most expensive first; cards with no price last" },
];

export function DeckToolbar() {
  const ui = useDeckUi();
  const readOnly = useDeckState().deck.category === "archived";
  const { matches } = useSharedDeckView();
  const matchCount = matches.cardIds.size + matches.addIds.size;
  const search = useRef<HTMLInputElement>(null);
  return (
    <div className={styles.toolbar}>
      <div className={styles.add}>
        {!readOnly && <AddBar />}
      </div>
      <div className={styles.view} role="group" aria-label="Deck view controls">
        <div className={styles.filters}>
          <div className={styles.search}>
            <TextField
              ref={search}
              type="search"
              aria-label="Search this deck"
              placeholder="Search this deck…"
              autoComplete="off"
              spellCheck={false}
              value={ui.searchTerm}
              onChange={(e) => ui.setSearchTerm(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  e.preventDefault();
                  e.stopPropagation();
                  ui.setSearchTerm("");
                }
              }}
            />
            {ui.searchTerm && (
              <IconButton
                label="Clear deck search"
                onClick={() => {
                  ui.setSearchTerm("");
                  search.current?.focus();
                }}
              >
                <Icon name="close" />
              </IconButton>
            )}
            {matches.active && (
              <Text mono tone="soft" size="sm" role="status" title="Matching cards are highlighted; other cards are muted">
                {matchCount} {matchCount === 1 ? "match" : "matches"}
              </Text>
            )}
          </div>
          {matches.active && (
            <div className={styles.filterSummary}>
              <Button size="sm" variant="ghost" onClick={() => ui.openDialog("export")}>Export matches</Button>
              <Button size="sm" variant="ghost" onClick={() => { ui.setSearchTerm(""); ui.setTagIds([]); }}>Clear filters</Button>
            </div>
          )}
          <DeckTagFilter />
        </div>
        <div className={styles.layout}>
          <Segmented label="Group the list" options={GROUP_OPTIONS} value={ui.groupBy} onChange={ui.setGroupBy} />
          <span className={styles.sort}>
            <Text size="xs" tone="faint">
              Sort
            </Text>
            <Segmented label="Order the cards in each group" options={SORT_OPTIONS} value={ui.sortBy} onChange={ui.setSortBy} />
          </span>
        </div>
      </div>
    </div>
  );
}
