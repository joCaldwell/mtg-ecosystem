// What is already known about a card at the moment of ruling on it: the
// memories linked to it, the owner's earlier rulings on it, and, for a cut,
// whether it is even in the paper deck. Loaded per item because the answer is
// the card's, not the proposal's.

import { api, type ProposalItem } from "../../../api/client.ts";
import { useQuery } from "../../../api/useQuery.ts";
import { InlineText } from "../../../mtg/index.ts";
import { Text } from "../../../ui/index.ts";
import { CardRulingRow } from "../../rulings/index.ts";
import { useDeck } from "../DeckStore.tsx";
import styles from "./ItemContext.module.css";

/** `inPaper` is null when the card is not in the deck. */
export function ItemContext({ item, inPaper }: { item: ProposalItem; inPaper: boolean | null }) {
  const { deckId } = useDeck();
  const history = useQuery(() => api.cardHistory(deckId, item.oracle_id), [deckId, item.oracle_id]);

  const memories = history.data?.memories ?? [];
  const rulings = history.data?.rulings ?? [];
  const unseen = item.action === "cut" && inPaper === false;
  if (!memories.length && !rulings.length && !unseen) return null;

  return (
    <div className={styles.context}>
      {memories.map((m) => (
        <div key={`m${m.id}`} className={`${styles.line} ${styles.memory}`}>
          <Text tone="agent">Memory {m.id}</Text> · {m.basis} · {m.author} · <InlineText text={m.text} />
        </div>
      ))}
      {rulings.map((r) => (
        <div key={`r${r.log_id}`} className={styles.line}>
          <CardRulingRow ruling={r} />
        </div>
      ))}
      {unseen && (
        <div className={styles.line}>
          Only you see this · it is not in the paper deck yet, so cutting it costs nothing.
        </div>
      )}
    </div>
  );
}
