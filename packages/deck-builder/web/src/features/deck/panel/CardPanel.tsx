// A card opened from anywhere on the deck screen (spec §5.4). It need not be in
// the deck: a proposed add, a card named in a memory and a search result open
// the same way. Facts come from the best source at hand, in order: the deck's
// own row, a proposal item naming it, then a lookup by name.

import { useEffect, useState } from "react";
import { ptString } from "../../../domain/cards.ts";
import { pendingOf } from "../../../domain/grouping.ts";
import { CardImage, InlineText, ManaCost, OracleText, useCardLookup, type CardInfo } from "../../../mtg/index.ts";
import { Button, Chip, Icon, Loading, Text } from "../../../ui/index.ts";
import { useDeckState } from "../DeckStore.tsx";
import { useDeckUi, type OpenCard } from "../DeckUi.tsx";
import { CardActions } from "./CardActions.tsx";
import { CardControls } from "./CardControls.tsx";
import { CardHistory } from "./CardHistory.tsx";
import styles from "./CardPanel.module.css";

export function CardPanel({ card: open }: { card: OpenCard }) {
  const state = useDeckState();
  const ui = useDeckUi();
  const { lookup } = useCardLookup();

  const owned = state.cards.find((c) => c.oracle_id === open.oracleId);
  const proposals = pendingOf(state).filter((i) => i.oracle_id === open.oracleId);
  const proposal = proposals[0];
  const seen = state.proposals.flatMap((p) => p.items).find((i) => i.oracle_id === open.oracleId);

  const [looked, setLooked] = useState<CardInfo | null>(null);
  const needsLookup = !owned && !seen;
  useEffect(() => {
    if (!needsLookup) return;
    let current = true;
    void lookup(open.name).then((found) => current && setLooked(found));
    return () => {
      current = false;
    };
  }, [needsLookup, lookup, open.name]);

  const info: CardInfo | null = owned ?? (seen ? { ...seen, name: seen.card_name } : looked);
  const where = !owned
    ? "Not in the deck"
    : owned.maybe
      ? "On the maybe list"
      : owned.role === "commander"
        ? "Commander"
        : owned.role === "companion"
          ? "Companion"
          : "In the deck";
  const pt = info ? ptString(info) : "";
  const parts = info ? [info.type_line, pt].filter(Boolean) : [];

  return (
    <>
      <div className={styles.top}>
        <Button size="sm" variant="ghost" kbd="Esc" onClick={() => ui.openCardPanel(null)}>
          <Icon name="back" size={12} /> Back
        </Button>
        <div className={styles.status}>
          <Chip>{where}</Chip>
          {proposal && <Chip tone={proposal.action === "add" ? "add" : "cut"}>Proposed {proposal.action}</Chip>}
        </div>
      </div>

      <div className={styles.head}>
        <div className={styles.titles}>
          <h2 className={styles.name}>{open.name}</h2>
          {parts.length > 0 && <Text tone="muted">{parts.join(" · ")}</Text>}
        </div>
        {info && <ManaCost cost={info.mana_cost} />}
      </div>

      {info ? info.oracle_text && <OracleText text={info.oracle_text} /> : <Loading />}

      <div className={styles.facts}>
        <CardImage name={open.name} width={150} />
        <div className={styles.side}>
          {owned ? (
            <CardControls card={owned} />
          ) : (
            proposal && (
              <div className={styles.note}>
                <Text tone="agent" size="sm">
                  Proposal {proposal.proposal_id}
                </Text>
                {proposal.reason && (
                  <Text tone="soft" size="sm" as="p">
                    <InlineText text={proposal.reason} />
                  </Text>
                )}
                <Text tone="muted" size="sm" as="p">
                  Rule on it in the Review tab.
                </Text>
              </div>
            )
          )}
          {!!owned?.maybe && owned.maybe_note && (
            <Text tone="muted" size="sm" as="p">
              Shortlisted because: <InlineText text={owned.maybe_note} />
            </Text>
          )}
        </div>
      </div>

      <CardActions oracleId={open.oracleId} owned={owned} />
      <CardHistory oracleId={open.oracleId} />
    </>
  );
}
