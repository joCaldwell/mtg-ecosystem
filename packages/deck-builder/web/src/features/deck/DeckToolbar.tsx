import { useRef } from "react";
import { Icon, IconButton, Select, Text, TextField } from "../../ui/index.ts";
import type { SortBy } from "../../domain/cards.ts";
import type { GroupBy } from "../../domain/grouping.ts";
import { AddBar } from "./AddBar.tsx";
import { DeckTagFilter } from "./DeckTagFilter.tsx";
import { useDeckState } from "./DeckStore.tsx";
import { useDeckUi } from "./DeckUi.tsx";
import styles from "./DeckToolbar.module.css";

const GROUP_OPTIONS: Array<{ value: GroupBy; label: string }> = [
  { value: "slot", label: "Slots" },
  { value: "type", label: "Card type" },
];

const SORT_OPTIONS: Array<{ value: SortBy; label: string; title: string }> = [
  { value: "curve", label: "Mana value", title: "Cheapest to cast first, then by name" },
  { value: "name", label: "Name", title: "A to Z" },
  { value: "price", label: "Price", title: "Most expensive first; cards with no price last" },
];

export function DeckToolbar() {
  const ui = useDeckUi();
  const readOnly = useDeckState().deck.category === "archived";
  const search = useRef<HTMLInputElement>(null);
  return (
    <div className={styles.toolbar}>
      {!readOnly && <div className={styles.add}><AddBar /></div>}
      <div className={styles.view} role="group" aria-label="Deck view controls">
        <div className={styles.filters}>
          <div className={styles.search}>
            <Icon name="search" size={14} />
            <TextField
              ref={search}
              type="search"
              aria-label="Search this deck"
              placeholder="Find in this deck…"
              className={styles.searchInput}
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
          </div>
          <DeckTagFilter />
        </div>
        <div className={styles.layout}>
          <label className={styles.choice}><Text size="sm" tone="muted">Group</Text>
            <Select aria-label="Group the list" value={ui.groupBy} onChange={e => ui.setGroupBy(e.target.value as GroupBy)}>
              {GROUP_OPTIONS.map(o => <option key={o.value} value={o.value}>{o.label}</option>)}
            </Select>
          </label>
          <label className={styles.choice}><Text size="sm" tone="muted">Sort</Text>
            <Select aria-label="Order the cards in each group" value={ui.sortBy} onChange={e => ui.setSortBy(e.target.value as SortBy)}>
              {SORT_OPTIONS.map(o => <option key={o.value} value={o.value} title={o.title}>{o.label}</option>)}
            </Select>
          </label>
        </div>
      </div>
    </div>
  );
}
