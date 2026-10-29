import { Button, Icon, IconButton, Text } from "../../ui/index.ts";
import { useDeckState } from "./DeckStore.tsx";
import { useDeckUi } from "./DeckUi.tsx";
import { useFindInDeck } from "./useFindInDeck.ts";
import styles from "./DeckFilterSummary.module.css";

/** The active filters, where the owner is among their matches, and what to do
 *  with them. Part of the toolbar; on tablets it stays below the navigation
 *  as the toolbar scrolls away. */
export function DeckFilterSummary() {
  const ui = useDeckUi();
  const { tags } = useDeckState();
  const find = useFindInDeck();
  if (!find.active) return null;
  const selected = tags.filter(tag => ui.tagIds.includes(tag.id)).map(tag => tag.name);
  const query = [ui.searchTerm.trim() ? `“${ui.searchTerm.trim()}”` : "", selected.length
    ? `${ui.tagInverted ? "Not " : ""}${ui.tagMatch} of: ${selected.join(", ")}` : ""].filter(Boolean).join(" · ");
  return <div className={styles.summary} role="group" aria-label="Active deck filters">
    <Text className={styles.query} title={query}>{query}</Text>
    <div className={styles.position}>
      <Text mono size="sm" role="status" className={styles.count}>
        {find.total ? <>{find.index + 1} of {find.total}<span className="visually-hidden"> matches</span></> : "No matches"}
      </Text>
      {find.total > 0 && <>
        <IconButton label="Previous match" size="sm" className={styles.previous} onClick={() => find.step(-1)}><Icon name="chevronDown" /></IconButton>
        <IconButton label="Next match" size="sm" onClick={() => find.step(1)}><Icon name="chevronDown" /></IconButton>
      </>}
    </div>
    <Button size="sm" onClick={() => { ui.setSearchTerm(""); ui.setTagIds([]); }}>Clear filters</Button>
    <Button size="sm" onClick={() => ui.openDialog("export")}>Export matches</Button>
  </div>;
}
