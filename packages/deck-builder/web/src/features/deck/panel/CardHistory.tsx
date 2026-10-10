// What the deck knows about a card (spec §7.2, §8.3): memories linked to it
// and the owner's earlier rulings on it. The same context an agent is shown
// beside the card.

import { api } from "../../../api/client.ts";
import { useQuery } from "../../../api/useQuery.ts";
import { Eyebrow, Text } from "../../../ui/index.ts";
import { MemoryCard } from "../../memories/index.ts";
import { CardRulingRow } from "../../rulings/index.ts";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import styles from "./CardHistory.module.css";

export function CardHistory({ oracleId }: { oracleId: string }) {
  const { deckId } = useDeck();
  const { deck } = useDeckState();
  const history = useQuery(() => api.cardHistory(deckId, oracleId), [deckId, oracleId, deck.revision]);
  const memories = history.data?.memories ?? [];
  const rulings = history.data?.rulings ?? [];

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
        <Eyebrow>Your rulings on it in this deck · {rulings.length}</Eyebrow>
        {rulings.map((r) => (
          <CardRulingRow key={r.log_id} ruling={r} />
        ))}
        {rulings.length === 0 && <Text tone="faint">None.</Text>}
      </section>
    </>
  );
}
