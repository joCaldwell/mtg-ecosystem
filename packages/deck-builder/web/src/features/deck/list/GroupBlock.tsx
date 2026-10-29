// One heading and its rows. In review mode the group also draws the cards
// the pending proposals would add, after its own rows. While a filter is
// active it draws only its matching rows, and nothing when none match.

import type { ReactNode } from "react";
import type { DeckCard, ProposalItem } from "../../../api/client.ts";
import { filterGroup, type DeckFilterMatches } from "../../../domain/deckSearch.ts";
import type { GroupStatus } from "../../../domain/grouping.ts";
import type { CardMark } from "../../../domain/marks.ts";
import type { GroupCost } from "../../../domain/price.ts";
import { CardRow } from "./CardRow.tsx";
import { GroupHeader } from "./GroupHeader.tsx";
import styles from "./GroupBlock.module.css";
import { useDeckList } from "../DeckListStore.tsx";
import { useDeckUi } from "../DeckUi.tsx";

// A group this long would leave the other columns empty if it stayed whole,
// as in a deck with no slots where every card is unfiled.
const LONG_GROUP = 30;

export interface GroupBlockProps {
  title: string;
  count?: number;
  primaryCount?: number;
  secondaryCount?: number;
  target?: string | null;
  status?: GroupStatus;
  delta?: number;
  projected?: number;
  cost?: GroupCost;
  cards: DeckCard[];
  secondaryCards?: DeckCard[];
  commandZoneCards?: DeckCard[];
  /** The active filter, if any; unfiltered when omitted or inactive. */
  filter?: DeckFilterMatches;
  /** Pending adds, drawn only while reviewing. */
  adds?: ProposalItem[];
  /** oracle_ids with a pending cut, drawn only while reviewing. */
  cuts?: Set<string>;
  /** oracle_ids belonging to the decision under the review cursor. */
  current: Set<string>;
  marks: Map<string, CardMark>;
  bold?: boolean;
  /** Shown when the group has nothing in it. */
  empty?: ReactNode;
}

export function GroupBlock({ cards: allCards, secondaryCards = [], commandZoneCards = [], filter, adds: allAdds = [], cuts, current, marks, bold, empty, ...head }: GroupBlockProps) {
  const { slots } = useDeckList().state;
  const showPrimary = useDeckUi().groupBy === "slot";
  const slotName = (id: number) => slots.find(s => s.id === id)?.name ?? "";
  const filtered = filter?.active ? filterGroup({ cards: allCards, adds: allAdds }, filter, cuts) : undefined;
  if (filtered === null) return null;
  const { cards, adds } = filtered ?? { cards: allCards, adds: allAdds };
  // Contributors are drawn in their own primary group when they match.
  const contributors = !filtered;
  return (
    <section className={`${styles.group} ${cards.length + adds.length > LONG_GROUP ? styles.long : ""}`}>
      <GroupHeader {...head} matched={filtered?.matched} />
      {cards.map((c) => (
        <CardRow
          key={c.oracle_id}
          card={c}
          purpose={showPrimary && c.role !== "card" && c.slot_id != null ? `Primary: ${slotName(c.slot_id)}` : undefined}
          filterMatch={!!filtered}
          quantity={c.quantity}
          change={cuts?.has(c.oracle_id) ? "cut" : undefined}
          mark={marks.get(c.oracle_id)}
          bold={bold}
          current={current.has(c.oracle_id)}
        />
      ))}
      {adds.map((item) => (
        <CardRow
          key={`add-${item.id}`}
          card={{ ...item, name: item.card_name }}
          filterMatch={!!filtered}
          change="add"
          mark={marks.get(item.oracle_id)}
          current={current.has(item.oracle_id)}
        />
      ))}
      {contributors && commandZoneCards.length > 0 && (
        <details className={styles.secondary}>
          <summary>Primary contributors in the command zone</summary>
          {commandZoneCards.map(c => (
            <CardRow key={`command-${c.oracle_id}`} card={c} quantity={c.quantity} purpose="Primary purpose · command zone"
              change={cuts?.has(c.oracle_id) ? "cut" : undefined}
              mark={marks.get(c.oracle_id)} current={current.has(c.oracle_id)} />
          ))}
        </details>
      )}
      {contributors && secondaryCards.length > 0 && (
        <details className={styles.secondary}>
          <summary>{head.primaryCount} primary · {head.secondaryCount} secondary {head.secondaryCount === 1 ? "contributor" : "contributors"}</summary>
          {secondaryCards.map(c => (
            <CardRow key={`secondary-${c.oracle_id}`} card={c} quantity={c.quantity}
              purpose={c.slot_id == null ? "Secondary purpose · no primary slot" : `Secondary purpose · primary: ${slotName(c.slot_id)}`}
              change={cuts?.has(c.oracle_id) ? "cut" : undefined}
              mark={marks.get(c.oracle_id)} current={current.has(c.oracle_id)} />
          ))}
        </details>
      )}
      {cards.length === 0 && secondaryCards.length === 0 && commandZoneCards.length === 0 && adds.length === 0 && empty}
    </section>
  );
}
