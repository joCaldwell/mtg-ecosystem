import { DECK_CATEGORY_OPTIONS } from "../../domain/deckCategory.ts";
import { Button, Select, Text, TextField } from "../../ui/index.ts";
import styles from "./DeckFilters.module.css";

/** The roster's controls keep their names without a second label row. */
export function DeckFilters({ search, category, shown, total, onSearch, onCategory }: {
  search: string; category: string; shown: number; total: number;
  onSearch: (value: string) => void; onCategory: (value: string) => void;
}) {
  const filtered = search.trim() !== "" || category !== "all";
  return <div className={styles.toolbar} role="group" aria-label="Deck filters">
    <TextField className={styles.search} type="search" aria-label="Find a deck" value={search}
      onChange={event => onSearch(event.target.value)} placeholder="Deck or commander name" />
    <Select aria-label="Lifecycle" className={styles.lifecycle} options={[{ value: "all", label: "All lifecycles" }, ...DECK_CATEGORY_OPTIONS.map(({ value, label }) => ({ value, label }))]} value={category} onChange={onCategory} />
    <Text className={styles.count} size="sm" tone="muted" role="status">{filtered ? `${shown} of ${total} decks` : `${total} decks`}</Text>
    {filtered && <Button variant="ghost" onClick={() => { onSearch(""); onCategory("all"); }}>Clear filters</Button>}
  </div>;
}
