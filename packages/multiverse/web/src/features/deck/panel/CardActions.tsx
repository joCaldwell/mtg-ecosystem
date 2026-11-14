// What the owner can do with a card from here: move it between the deck, the
// maybe list and out. All of them apply at once.

import { api, type DeckCard } from "../../../api/client.ts";
import { Button } from "../../../ui/index.ts";
import { useDeck } from "../DeckStore.tsx";
import styles from "./CardActions.module.css";

export function CardActions({ oracleId, owned }: { oracleId: string; owned: DeckCard | undefined }) {
  const { deckId, run } = useDeck();
  const remove = () => run(() => api.removeCard(deckId, oracleId));

  if (!owned)
    return (
      <div className={styles.actions}>
        <Button variant="accept" onClick={() => void run(() => api.addCard(deckId, oracleId))}>
          Add to deck
        </Button>
        <Button onClick={() => void run(() => api.addCard(deckId, oracleId, { maybe: true }))}>Add to maybe list</Button>
      </div>
    );

  if (owned.maybe)
    return (
      <div className={styles.actions}>
        <Button variant="accept" onClick={() => void run(() => api.updateCard(deckId, oracleId, { maybe: false }))}>
          Add to the deck
        </Button>
        <Button variant="reject" onClick={() => void remove()}>
          Remove from maybe list
        </Button>
      </div>
    );

  // A commander or companion is changed through its role first.
  if (owned.role !== "card") return null;
  return (
    <div className={styles.actions}>
      <Button onClick={() => void run(() => api.updateCard(deckId, oracleId, { maybe: true }))}>Move to maybe</Button>
      <Button variant="reject" onClick={() => void remove()}>
        {owned.quantity > 1 ? "Cut all copies" : "Cut"}
      </Button>
    </div>
  );
}
