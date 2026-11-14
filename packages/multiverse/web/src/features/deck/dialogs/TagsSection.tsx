import { useState, type FormEvent } from "react";
import { api } from "../../../api/client.ts";
import { Button, ErrorNote, Row, Stack, Text, TextField } from "../../../ui/index.ts";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import { TagRow } from "./TagRow.tsx";
import { useAttempt } from "./useAttempt.ts";
import styles from "./TagsSection.module.css";

/** The deck's tags: rename or delete each, and add another. */
export function TagsSection() {
  const { deckId } = useDeck();
  const { tags } = useDeckState();
  const { error, setError, busy, mutate } = useAttempt();
  const [name, setName] = useState("");

  async function rename(tagId: number, next: string) {
    return (await mutate(() => api.renameTag(deckId, tagId, next))) != null;
  }

  async function remove(tagId: number) {
    await mutate(() => api.deleteTag(deckId, tagId));
  }

  async function add(e: FormEvent) {
    e.preventDefault();
    if (busy || !name.trim()) return;
    const result = await mutate(() => api.createTag(deckId, name.trim()));
    if (result) setName("");
  }

  return (
    <Stack gap={4}>
      {error ? <ErrorNote>{error}</ErrorNote> : null}
      {tags.length ? (
        <ul className={styles.list}>
          {tags.map((tag) => (
            <TagRow key={tag.id} tag={tag} onRename={rename} onDelete={remove} onRevert={() => setError(null)} />
          ))}
        </ul>
      ) : (
        <Text tone="muted" as="p">
          No tags yet. Add one below, then put it on cards from the card panel.
        </Text>
      )}
      <form method="post" onSubmit={add}>
        <Row>
          <TextField name="name" required pattern={".*\\S.*"} disabled={busy} value={name} onChange={(e) => setName(e.target.value)} placeholder="Add a tag" aria-label="Name of the new tag" />
          <Button size="sm" variant="primary" type="submit" disabled={busy}>
            Add
          </Button>
        </Row>
      </form>
    </Stack>
  );
}
