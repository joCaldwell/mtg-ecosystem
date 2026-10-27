import { useVoice } from "../../../app/Preferences.tsx";
// The Memories tab (spec §7): what agents have written down about this deck,
// with editing and retirement in place.

import { api } from "../../../api/client.ts";
import { useQuery } from "../../../api/useQuery.ts";
import { href } from "../../../app/routes.ts";
import { ErrorNote, Loading, Text } from "../../../ui/index.ts";
import { AddMemory, MemoryCard } from "../../memories/index.ts";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import styles from "./MemoriesPanel.module.css";

export function MemoriesPanel() {
  const voice = useVoice();
  const { deck } = useDeckState();
  const { deckId } = useDeck();
  const list = useQuery(() => api.listMemories({ scope: "deck", deck: deckId }), [deckId]);
  const memories = list.data?.memories ?? [];

  return (
    <>

      <section className={styles.section}>
        {list.error && <ErrorNote>{list.error}</ErrorNote>}
        {list.loading && <Loading />}
        {memories.map((m) => (
          <MemoryCard key={m.id} memory={m} showScope={false} onChanged={list.reload} />
        ))}
        {!list.loading && !list.error && memories.length === 0 && (
          <Text tone="muted" size="sm" as="p">
            {voice("No notes for this deck yet. Write a preference or lesson below.", "No memories for this deck yet. Agents add them as they learn how you play it, or you can write one below.")}
          </Text>
        )}
        {deck.category !== "archived" && <AddMemory scope="deck" deckId={deckId} onAdded={list.reload} />}
      </section>

      <a className={styles.all} href={href({ page: "memories" })}>
        All memories, including global ones →
      </a>
      <Text tone="faint" size="xs" as="p">
        {voice("Keep lessons here as the deck changes. Retired memories remain in the history.", "Agents write these without asking. Nothing is hidden, and nothing an agent wrote is deleted, only retired.")}
      </Text>
    </>
  );
}
