import { Button, Text } from "../../ui/index.ts";
import { useDeckState } from "./DeckStore.tsx";
import { useDeckUi } from "./DeckUi.tsx";
import { useSharedDeckView } from "./DeckViewContext.ts";
import styles from "./DeckFilterSummary.module.css";

/** Only active filter context stays with the tablet list as its toolbar leaves. */
export function DeckFilterSummary() {
  const ui = useDeckUi();
  const { tags } = useDeckState();
  const { matches } = useSharedDeckView();
  if (!matches.active) return null;
  const count = matches.cardIds.size + matches.addIds.size;
  const selected = tags.filter(tag => ui.tagIds.includes(tag.id)).map(tag => tag.name);
  const query = [ui.searchTerm.trim() ? `“${ui.searchTerm.trim()}”` : "", selected.length
    ? `${ui.tagInverted ? "Not " : ""}${ui.tagMatch} of: ${selected.join(", ")}` : ""].filter(Boolean).join(" · ");
  return <div className={styles.summary} role="group" aria-label="Active deck filters">
    <Text className={styles.query} title={query}>{query}</Text>
    <Text mono size="sm" role="status">{count} {count === 1 ? "match" : "matches"}</Text>
    <Button size="sm" onClick={() => { ui.setSearchTerm(""); ui.setTagIds([]); }}>Clear filters</Button>
  </div>;
}
