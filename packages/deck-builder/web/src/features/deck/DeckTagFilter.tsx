import { useEffect } from "react";
import { Button, Checkbox, MultiSelect, Segmented, Stack } from "../../ui/index.ts";
import { useDeckState } from "./DeckStore.tsx";
import { useDeckUi } from "./DeckUi.tsx";
import styles from "./DeckTagFilter.module.css";

export function DeckTagFilter() {
  const { tags, deck } = useDeckState();
  const { tagIds, tagMatch, tagInverted, setTagIds, setTagMatch, setTagInverted, openDialog } = useDeckUi();
  // A deleted tag must not leave an invisible filter muting the whole list.
  const liveIds = tagIds.filter((id) => tags.some((tag) => tag.id === id));
  useEffect(() => {
    if (liveIds.length !== tagIds.length) setTagIds(liveIds);
  }, [tags, tagIds, setTagIds]);

  if (tags.length === 0 && deck.category === "archived") return null;

  return (
    <div className={styles.filters}>
      {tags.length === 0 ? (
        <Button onClick={() => openDialog("slots")} aria-haspopup="dialog">Create tags</Button>
      ) : (
        <div className={styles.picker}>
          <MultiSelect
            label="Filter by tags"
            placeholder="Tags"
            options={tags.map((tag) => ({ value: String(tag.id), label: tag.name }))}
            value={liveIds.map(String)}
            compact
            summary={tagInverted
              ? liveIds.length === 1
                ? `Not ${tags.find((tag) => tag.id === liveIds[0])?.name}`
                : `Not ${tagMatch} of ${liveIds.length} tags`
              : undefined}
            onChange={(ids) => setTagIds(ids.map(Number))}
            emptyText="No tags yet. Create tags, then assign them to cards."
            footer={liveIds.length > 0 && (
              <Stack gap={3}>
                <div className={styles.actions}>
                  <Segmented label="Match selected tags" options={[{ value: "any", label: "Any tag" }, { value: "all", label: "All tags" }]} value={tagMatch} onChange={setTagMatch} />
                  <Button size="sm" variant="ghost" onClick={() => setTagIds([])}>Clear tags</Button>
                </div>
                <Checkbox label="Invert filter" checked={tagInverted} onChange={setTagInverted} />
              </Stack>
            )}
          />
        </div>
      )}
    </div>
  );
}
