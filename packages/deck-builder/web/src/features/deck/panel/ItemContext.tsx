// What is already known about a card at the moment of decision on it: the
// memories linked to it and the owner's earlier decisions on it. Loaded per
// item because the answer is the card's, not the proposal's.

import { api, type ProposalItem } from "../../../api/client.ts";
import { useQuery } from "../../../api/useQuery.ts";
import { InlineText } from "../../../mtg/index.ts";
import { Text } from "../../../ui/index.ts";
import { CardDecisionRow } from "../../decisions/index.ts";
import { useDeck } from "../DeckStore.tsx";
import styles from "./ItemContext.module.css";

export function ItemContext({ item }: { item: ProposalItem }) {
  const { deckId } = useDeck();
  const history = useQuery(() => api.cardHistory(deckId, item.oracle_id), [deckId, item.oracle_id]);

  const memories = history.data?.memories ?? [];
  const decisions = history.data?.decisions ?? [];
  if (!memories.length && !decisions.length) return null;

  return (
    <div className={styles.context}>
      {memories.map((m) => (
        <div key={`m${m.id}`} className={`${styles.line} ${styles.memory}`}>
          <Text tone="agent">Memory {m.id}</Text> · {m.basis} · {m.author_display ?? m.author} · <InlineText text={m.text} />
        </div>
      ))}
      {decisions.map((r) => (
        <div key={`r${r.log_id}`} className={styles.line}>
          <CardDecisionRow decision={r} />
        </div>
      ))}
    </div>
  );
}
