import type { ReactNode } from "react";
import type { ArtFacts } from "../domain/art.ts";
import { DECK_CATEGORY_LABELS } from "../domain/deckCategory.ts";
import { Chip, Text, useNearView } from "../ui/index.ts";
import { ColorIdentity } from "./ColorIdentity.tsx";
import { DeckArt } from "./DeckArt.tsx";
import styles from "./DeckCard.module.css";

/** Shared roster presentation. Hosts supply navigation and owner-only actions. */
export function DeckCard({ name, href, art, identity, category, count, commanders, lineage, actions, children, roster = false }: {
  name: string; href: string; art: readonly ArtFacts[]; identity: string;
  category: keyof typeof DECK_CATEGORY_LABELS; count: number; commanders: ReactNode;
  lineage?: ReactNode; actions?: ReactNode; children?: ReactNode; roster?: boolean;
}) {
  const [ref, near] = useNearView<HTMLLIElement>();
  return <li ref={ref} className={`${styles.card} ${roster ? styles.ownerRoster : ""}`}>
    <DeckArt cards={art} identity={identity} load={near}>
      <div className={styles.deckCard}>
        <div className={styles.cardHeading}>
          <h2><a className={styles.deckLink} href={href}>{name}</a></h2>
        </div>
        <div className={styles.commander}>{commanders}</div>
        {lineage && <div className={styles.lineage}>{lineage}</div>}
        <div className={styles.cardStats}>
          <ColorIdentity colors={identity} />
          <Chip className={styles.lifecycle} data-lifecycle={category}>{DECK_CATEGORY_LABELS[category]}</Chip>
          <Text mono tone={count === 100 ? "soft" : "warn"}>{count}/100</Text>
          {children}
        </div>
      </div>
      {actions && <div className={styles.actions}>{actions}</div>}
    </DeckArt>
  </li>;
}
