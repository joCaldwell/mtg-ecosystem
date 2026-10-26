// What the deck knows about a card (spec §7.2, §8.3): memories linked to it
// and the owner's earlier decisions on it. The same context an agent is shown
// beside the card.

import { api } from "../../../api/client.ts";
import { useQuery } from "../../../api/useQuery.ts";
import { Eyebrow, Text } from "../../../ui/index.ts";
import { MemoryCard } from "../../memories/index.ts";
import { CardDecisionRow } from "../../decisions/index.ts";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import styles from "./CardHistory.module.css";

export function CardHistory({ oracleId }: { oracleId: string }) {
  const { deckId } = useDeck();
  const { deck } = useDeckState();
  const history = useQuery(() => api.cardHistory(deckId, oracleId), [deckId, oracleId, deck.revision]);
  const memories = history.data?.memories ?? [];
  const decisions = history.data?.decisions ?? [];

  return (
    <>
      <section className={styles.section}>
        <Eyebrow>Memories linked to this card · {memories.length}</Eyebrow>
        {memories.map((m) => (
          <MemoryCard key={m.id} memory={m} onChanged={history.reload} />
        ))}
        {memories.length === 0 && <Text tone="faint">None.</Text>}
      </section>
      <section className={styles.section}>
        <Eyebrow>Your decisions on it in this deck · {decisions.length}</Eyebrow>
        {decisions.map((r) => (
          <CardDecisionRow key={r.log_id} decision={r} />
        ))}
        {decisions.length === 0 && <Text tone="faint">None.</Text>}
      </section>
    </>
  );
}
