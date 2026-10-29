// A card opened from anywhere on the deck screen (spec §5.4). It need not be in
// the deck: a proposed add, a card named in a memory and a search result open
// the same way. Facts come from the best source at hand, in order: the deck's
// own row, a proposal item naming it, then a lookup by name.

import { useLive } from "../../../api/live.tsx";
import { useEffect, useState, type Ref } from "react";
import { ptString } from "../../../domain/cards.ts";
import { formatPrice } from "../../../domain/price.ts";
import { pendingOf } from "../../../domain/grouping.ts";
import { CardImage, InlineText, ManaCost, OracleText, RichText, useCardLookup, type CardInfo } from "../../../mtg/index.ts";
import { Button, Chip, Icon, IconButton, Loading, Text } from "../../../ui/index.ts";
import { useDeckState } from "../DeckStore.tsx";
import { useDeckUi, type OpenCard } from "../DeckUi.tsx";
import { CardActions } from "./CardActions.tsx";
import { CardControls } from "./CardControls.tsx";
import { CardHistory } from "./CardHistory.tsx";
import { panelViewLabel } from "../panelViews.ts";
import panelStyles from "./DeckPanel.module.css";
import styles from "./CardPanel.module.css";

export function CardPanel({ card: open, backRef, bodyRef }: { card: OpenCard; backRef: Ref<HTMLButtonElement>; bodyRef: Ref<HTMLDivElement> }) {
  const state = useDeckState();
  const ui = useDeckUi();
  const { lookup } = useCardLookup();

  const owned = state.cards.find((c) => c.oracle_id === open.oracleId);
  const proposals = pendingOf(state).filter((i) => i.oracle_id === open.oracleId);
  const proposal = proposals[0];
  // Attribution for a pending item: who proposed it reads `handle via label`.
  const proposedBy = proposal ? state.proposals.find((p) => p.id === proposal.proposal_id) : undefined;
  const byAgent = proposedBy?.author_via !== "ui";
  const seen = state.proposals.flatMap((p) => p.items).find((i) => i.oracle_id === open.oracleId);

  const live = useLive();
  const [looked, setLooked] = useState<CardInfo | null>(null);
  const needsLookup = !owned && !seen;
  useEffect(() => {
    if (!needsLookup) return;
    let current = true;
    void lookup(open.name).then((found) => current && setLooked(found));
    return () => {
      current = false;
    };
  }, [needsLookup, lookup, open.name, live]);

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
  const type = info ? [info.type_line, pt].filter(Boolean).join(" · ") : "";

  return (
    <>
      <div className={panelStyles.navigation} data-inspecting>
        <div className={styles.header}>
          <IconButton ref={backRef} size="sm" label={`Back to ${panelViewLabel(ui.tab)}`} onClick={() => ui.openCardPanel(null)}><Icon name="back" /></IconButton>
          <h2 className={styles.name}>{open.name}</h2>
          <div className={styles.status}>
            <Chip>{where}</Chip>
            {proposal && <Chip tone={byAgent ? "agent" : "neutral"}>Proposed {proposal.action}</Chip>}
          </div>
          {info && <span className={styles.cost}><ManaCost cost={info.mana_cost} /></span>}
        </div>
      </div>
      <div ref={bodyRef} className={panelStyles.body} role="region" aria-label={`Card details for ${open.name}`} tabIndex={0} data-panel-body>
      {/* The type line, then the estimate and where it comes from: the date
          is one unit, so it never breaks. */}
      {info && (type || info.cents != null) && <div className={styles.head}>
        {type && <Text tone="soft" size="sm" as="p">{type}</Text>}
        {info.cents != null && <Text tone="soft" size="sm" as="p">
          <Text mono>{formatPrice(info.cents, info.currency)}</Text> cheapest printing · {info.store === "tcgplayer" ? "TCGplayer" : "Cardmarket"} · Scryfall{" "}
          {info.as_of ? <Text mono className={styles.date}>{info.as_of.slice(0, 10)}</Text> : "date unknown"}
        </Text>}
      </div>}

      {proposal && (
        <div className={styles.note}>
          <Text tone={byAgent ? "agent" : "muted"} size="sm">
            Proposal {proposal.proposal_id}{proposedBy ? ` · ${proposedBy.author_display ?? proposedBy.author}` : ""}
          </Text>
          {proposal.reason && (
            <Text tone="soft" size="sm" as="p">
              <InlineText text={proposal.reason} />
            </Text>
          )}
          <div><Button size="sm" variant="agent" onClick={() => ui.setTab("review")}>Open Review</Button></div>
        </div>
      )}

      {!!owned?.maybe && owned.maybe_note && (
        <section className={styles.maybeNote} aria-label="Maybe note">
          <Text as="h3" size="sm" tone="muted" weight="medium">Maybe note</Text>
          <RichText text={owned.maybe_note} />
        </section>
      )}

      {info ? info.oracle_text && <OracleText text={info.oracle_text} /> : <Loading />}

      <div className={styles.facts}>
        <CardImage name={open.name} card={info} />
        <div className={styles.side}>
          {owned && <CardControls card={owned} />}
        </div>
      </div>

      {state.deck.category !== "archived" && <CardActions oracleId={open.oracleId} owned={owned} />}
      <CardHistory oracleId={open.oracleId} />
      </div>
    </>
  );
}
