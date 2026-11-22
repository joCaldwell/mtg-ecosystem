import { useState } from "react";
import { useVoice } from "../../../app/Preferences.tsx";
// The Memories tab (spec §7): what agents have written down about this deck,
// with editing and retirement in place.

import { api } from "../../../api/client.ts";
import { useQuery } from "../../../api/useQuery.ts";
import { href } from "../../../app/routes.ts";
import { Button, ErrorNote, Loading, Text } from "../../../ui/index.ts";
import { AddMemory, MemoryCard } from "../../memories/index.ts";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import styles from "./MemoriesPanel.module.css";

type Cursor = { before_id: number; before_revised_at: string };

export function MemoriesPanel() {
  const voice = useVoice();
  const { deck } = useDeckState();
  const { deckId } = useDeck();
  const [cursors, setCursors] = useState<Cursor[]>([]);
  const cursor = cursors.at(-1);
  const list = useQuery(async () => ({
    cursor, page: await api.listMemories({ scope: "deck", deck: deckId, ...cursor }),
  }), [deckId, cursor], { resources: ["memories"], deckId });
  const current = list.data?.cursor === cursor ? list.data?.page : undefined;
  const loading = !current && !list.error;
  const memories = current?.memories ?? [];
  const last = memories.at(-1);
  const more = !!(current?.has_more && last);

  return (
    <>

      <section className={styles.section} aria-busy={loading}>
        {list.error && <ErrorNote>{list.error}</ErrorNote>}
        {loading && <Loading />}
        {memories.map((m) => (
          <MemoryCard key={m.id} memory={m} showScope={false} onChanged={list.reload} />
        ))}
        {!loading && !list.error && memories.length === 0 && (
          <Text tone="muted" size="sm" as="p">
            {cursor ? "No older memories on this page." : voice("No notes for this deck yet. Write a preference or lesson below.", "No memories for this deck yet. Agents add them as they learn how you play it, or you can write one below.")}
          </Text>
        )}
        {(cursors.length > 0 || (list.data && (!current || current.has_more))) && <nav aria-label="Deck memory pages" className={styles.pages}>
          <Button size="sm" aria-disabled={!cursor} onClick={() => { if (cursor) setCursors(cursors.slice(0, -1)); }}>Previous page</Button>
          <Text role="status" size="sm">{loading ? "Loading memories…" : `Page ${cursors.length + 1}`}</Text>
          <Button size="sm" aria-disabled={!more} onClick={() => {
            if (more && last) setCursors([...cursors, { before_id: last.id, before_revised_at: last.revised_at }]);
          }}>Next page</Button>
        </nav>}
        {deck.category !== "archived" && <AddMemory scope="deck" deckId={deckId} onAdded={() => { setCursors([]); list.reload(); }} />}
      </section>

      <a className={styles.all} href={href({ page: "memories" })}>
        All memories, including global ones →
      </a>
    </>
  );
}
