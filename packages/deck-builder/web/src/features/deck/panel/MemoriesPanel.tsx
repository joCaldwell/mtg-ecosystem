// The Memories tab (spec §7): what agents have written down about this deck,
// and how much of it fits in what an agent is shown when it opens the deck.

import { api } from "../../../api/client.ts";
import { useQuery } from "../../../api/useQuery.ts";
import { href } from "../../../app/routes.ts";
import { Eyebrow, ErrorNote, Loading, Meter, Text } from "../../../ui/index.ts";
import { AddMemory, MemoryCard } from "../../memories/index.ts";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import styles from "./MemoriesPanel.module.css";

export function MemoriesPanel() {
  const { memory_budget: budget, deck } = useDeckState();
  const { deckId } = useDeck();
  const list = useQuery(() => api.listMemories({ scope: "deck", deck: deckId }), [deckId]);
  const memories = list.data?.memories ?? [];

  return (
    <>
      <section className={styles.section}>
        <Eyebrow>Loaded whenever an agent opens this deck</Eyebrow>
        <Meter
          value={budget.used}
          max={budget.budget}
          tone={budget.omitted > 0 ? "warn" : "agent"}
          aria-label="Share of the deck memory budget in use"
        />
        <Text tone="faint" size="xs" mono>
          {budget.used} of {budget.budget} estimated tokens
        </Text>
        {budget.omitted > 0 && (
          <Text tone="warn" size="sm" as="p">
            {budget.omitted} did not fit and {budget.omitted === 1 ? "is" : "are"} found by search only. Ask an agent
            to consolidate this deck's memories.
          </Text>
        )}
      </section>

      <section className={styles.section}>
        {list.error && <ErrorNote>{list.error}</ErrorNote>}
        {list.loading && <Loading />}
        {memories.map((m) => (
          <MemoryCard key={m.id} memory={m} showScope={false} onChanged={list.reload} />
        ))}
        {!list.loading && !list.error && memories.length === 0 && (
          <Text tone="muted" size="sm" as="p">
            No memories for this deck yet. Agents add them as they learn how you play it, or you can write one below.
          </Text>
        )}
        {deck.category !== "legacy" && <AddMemory scope="deck" deckId={deckId} onAdded={list.reload} />}
      </section>

      <a className={styles.all} href={href({ page: "memories" })}>
        All memories, including global ones →
      </a>
      <Text tone="faint" size="xs" as="p">
        Agents write these without asking. Nothing is hidden, and nothing an agent wrote is deleted, only retired.
      </Text>
    </>
  );
}
