import type { MouseEvent } from "react";
import { matchCount } from "../../domain/deckSearch.ts";
import { Button, Text } from "../../ui/index.ts";
import { useSharedDeckView } from "./DeckViewContext.ts";
import { useDeckState } from "./DeckStore.tsx";
import { useDeckUi } from "./DeckUi.tsx";
import styles from "./DeckFilterSummary.module.css";

/** The active filters, how many cards they leave in the list, and what to do
 *  with them. The first line of the list area, so the proposal banner stays
 *  under the toolbar; on tablets it stays below the navigation as the
 *  toolbar scrolls away. */
export function DeckFilterSummary() {
  const ui = useDeckUi();
  const { tags } = useDeckState();
  const { matches } = useSharedDeckView();
  if (!matches.active) return null;
  const total = matchCount(matches);
  const selected = tags.filter(tag => ui.tagIds.includes(tag.id)).map(tag => tag.name);
  const query = [ui.searchTerm.trim() ? `“${ui.searchTerm.trim()}”` : "", selected.length
    ? `${ui.tagInverted ? "Not " : ""}${ui.tagMatch} of: ${selected.join(", ")}` : ""].filter(Boolean).join(" · ");

  function clear(e: MouseEvent<HTMLButtonElement>) {
    const trigger = e.currentTarget;
    const page = trigger.closest<HTMLElement>("[data-deck-page]");
    ui.setSearchTerm("");
    ui.setTagIds([]);
    requestAnimationFrame(() => {
      if (!page?.isConnected || (document.activeElement !== trigger && document.activeElement !== document.body)) return;
      const search = page.querySelector<HTMLElement>("[data-deck-filter-input]");
      const bounds = search?.getBoundingClientRect();
      const visible = bounds && bounds.top >= 0 && bounds.bottom <= innerHeight && document.elementFromPoint(bounds.left + bounds.width / 2, bounds.top + bounds.height / 2) === search;
      // A tablet can restore to a position where the toolbar is offscreen.
      // Focus the list there without undoing the restored scroll offset.
      (visible ? search : page.querySelector<HTMLElement>("[data-deck-list]"))?.focus({ preventScroll: true });
    });
  }

  return <div className={styles.summary} role="group" aria-label="Active deck filters">
    <Text className={styles.query} title={query}>{query}</Text>
    <Text mono size="sm" role="status" className={styles.count}>
      {total === 0 ? "No matches" : total === 1 ? "1 match" : `${total} matches`}
    </Text>
    <Button size="sm" onClick={clear}>Clear filters</Button>
    <Button size="sm" onClick={() => ui.openDialog("export")}>Export matches</Button>
  </div>;
}
