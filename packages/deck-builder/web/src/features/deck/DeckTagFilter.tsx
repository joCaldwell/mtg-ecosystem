import { useEffect } from "react";
import { Button, MultiSelect, Segmented, Text } from "../../ui/index.ts";
import { useDeckState } from "./DeckStore.tsx";
import { useDeckUi } from "./DeckUi.tsx";
import styles from "./DeckTagFilter.module.css";

export function DeckTagFilter() {
  const { tags } = useDeckState();
  const { tagIds, tagMatch, setTagIds, setTagMatch, openDialog } = useDeckUi();
  // A deleted tag must not leave an invisible filter muting the whole list.
  const liveIds = tagIds.filter((id) => tags.some((tag) => tag.id === id));
  useEffect(() => {
    if (liveIds.length !== tagIds.length) setTagIds(liveIds);
  }, [tags, tagIds, setTagIds]);

  return (
    <div className={styles.filters}>
      <Text tone="muted">Tags</Text>
      {tags.length === 0 ? (
        <Button variant="ghost" onClick={() => openDialog("slots")}>Create tags</Button>
      ) : (
        <div className={styles.picker}>
          <MultiSelect
            label="Filter by tags"
            placeholder="Filter by tags…"
            options={tags.map((tag) => ({ value: String(tag.id), label: tag.name }))}
            value={liveIds.map(String)}
            compact
            onChange={(ids) => setTagIds(ids.map(Number))}
            emptyText="No tags yet. Create tags, then assign them to cards."
            footer={liveIds.length > 0 && (
              <div className={styles.actions}>
                <Segmented label="Match selected tags" options={[{ value: "any", label: "Any tag" }, { value: "all", label: "All tags" }]} value={tagMatch} onChange={setTagMatch} />
                <Button size="sm" variant="ghost" onClick={() => setTagIds([])}>Clear tags</Button>
              </div>
            )}
          />
        </div>
      )}
    </div>
  );
}
