import { useRef } from "react";
import { Icon, IconButton, Segmented, TextField } from "../../ui/index.ts";
import type { GroupBy } from "../../domain/grouping.ts";
import { AddBar } from "./AddBar.tsx";
import { DeckTagFilter } from "./DeckTagFilter.tsx";
import { useDeckUi } from "./DeckUi.tsx";
import styles from "./DeckToolbar.module.css";

const GROUP_OPTIONS: Array<{ value: GroupBy; label: string }> = [
  { value: "slot", label: "By slot" },
  { value: "type", label: "By type" },
];

export function DeckToolbar() {
  const ui = useDeckUi();
  const search = useRef<HTMLInputElement>(null);
  return (
    <div className={styles.toolbar}>
      <AddBar />
      <div className={styles.search}>
        <TextField
          ref={search}
          type="search"
          fieldSize="lg"
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
      </div>
      <Segmented label="Group the list" options={GROUP_OPTIONS} value={ui.groupBy} onChange={ui.setGroupBy} />
      <DeckTagFilter />
    </div>
  );
}
