import type { DeckState } from "../../api/client.ts";
import { DECK_CATEGORY_LABELS } from "../../domain/deckCategory.ts";
import { Chip } from "../../ui/index.ts";
import styles from "./DeckScrollHeading.module.css";

export function DeckScrollHeading({ deck, shown }: { deck: Pick<DeckState["deck"], "name" | "category">; shown: boolean }) {
  return <div className={styles.heading} role="region" aria-label="Current deck" hidden={!shown}>
    <span className={styles.name}>{deck.name}</span>
    <Chip>{DECK_CATEGORY_LABELS[deck.category]}</Chip>
  </div>;
}
