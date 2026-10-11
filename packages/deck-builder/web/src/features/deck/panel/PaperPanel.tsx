// The Paper tab (spec §5.5): which cards are physically in the deck box. Only
// the owner can set this and agents never see it unless they ask for the paper
// list, so it cannot tilt what they recommend. The workflow is exception-first:
// mark the whole deck assembled, then un-mark the few that are not.

import { useState } from "react";
import { api } from "../../../api/client.ts";
import { useQuery } from "../../../api/useQuery.ts";
import { quantityOf } from "../../../domain/cards.ts";
import { costCaption, formatUsd } from "../../../domain/price.ts";
import { CardRef, ManaCost, Price } from "../../../mtg/index.ts";
import { Box, Button, Chip, ConfirmDialog, ErrorNote, Eyebrow, Text, useToast } from "../../../ui/index.ts";
import { useDeck, useDeckState } from "../DeckStore.tsx";
import styles from "./PaperPanel.module.css";

export function PaperPanel() {
  const state = useDeckState();
  const { deckId, run } = useDeck();
  const { toast } = useToast();
  const [confirming, setConfirming] = useState(false);

  const main = state.cards.filter((c) => !c.maybe);
  const missing = main.filter((c) => !c.in_paper);
  const total = quantityOf(main);
  const present = total - quantityOf(missing);

  // The pull and buy lists need other decks' paper flags, so the server answers
  // them; marking cards changes the answer without changing the revision.
  const status = useQuery(() => api.paperStatus(deckId), [deckId, state.deck.revision, missing.length]);

  async function exportMissing() {
    try {
      const out = await api.exportDeck(deckId, { onlyMissing: true });
      await navigator.clipboard.writeText(out.text);
      toast(`Copied ${out.line_count} ${out.line_count === 1 ? "line" : "lines"} of missing cards to the clipboard.`);
    } catch (e: unknown) {
      toast(`Could not export the missing cards: ${e instanceof Error ? e.message : String(e)}`);
    }
  }

  return (
    <>
      <section className={styles.section}>
        <Eyebrow>Paper deck</Eyebrow>
        <Text as="p" size="md">
          <span className={styles.num}>{present}</span> of <span className={styles.num}>{total}</span> cards are in the
          paper deck.
        </Text>
        {main.length === 0 && (
          <Text tone="muted" size="sm" as="p">
            Add cards to the deck, then mark which of them are in the physical deck.
          </Text>
        )}
        {missing.length > 0 && (
          <>
            <div>
              <Button size="sm" onClick={() => setConfirming(true)}>
                Mark all in paper
              </Button>
            </div>
            <Text tone="muted" size="sm" as="p">
              The quickest way to start: mark everything as assembled, then un-mark only the exceptions in the list.
            </Text>
          </>
        )}
      </section>

      <section className={styles.section}>
        <Eyebrow>Missing · {quantityOf(missing)}</Eyebrow>
        {missing.length === 0 ? (
          <Text tone="muted" size="sm" as="p">
            {main.length ? "Every card is in the paper deck." : "Nothing to be missing yet."}
          </Text>
        ) : (
          <div className={styles.rows}>
            {missing.map((c) => (
              <div key={c.oracle_id} className={styles.row}>
                <span className={styles.name}>
                  <CardRef name={c.name} card={c} />
                  {c.quantity > 1 && <span className={styles.qty}> ×{c.quantity}</span>}
                </span>
                <ManaCost cost={c.mana_cost} />
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => void run(() => api.updateCard(deckId, c.oracle_id, { in_paper: true }))}
                  aria-label={`Mark ${c.name} as in the paper deck`}
                >
                  In paper
                </Button>
              </div>
            ))}
          </div>
        )}
      </section>

      {status.error && <ErrorNote>{status.error}</ErrorNote>}
      {missing.length > 0 && status.data && (
        <>
          <section className={styles.section}>
            <Eyebrow>Pull from another deck · {status.data.pull.length}</Eyebrow>
            {status.data.pull.length === 0 && (
              <Text tone="muted" size="sm" as="p">
                Nothing to pull: no other deck holds a copy of a missing card.
              </Text>
            )}
            {status.data.pull.map((c) => (
              <Box key={c.name} padding="sm">
                <div className={styles.pullHead}>
                  <span className={styles.name}>
                    <CardRef name={c.name} />
                    {c.quantity > 1 && <span className={styles.qty}> ×{c.quantity}</span>}
                  </span>
                  <Chip>in {c.held_by.map((h) => h.deck_name).join(", ")}</Chip>
                </div>
                <Text tone="muted" size="sm" as="p">
                  Moving this copy leaves {c.held_by.map((h) => h.deck_name).join(" and ")} without it.
                </Text>
              </Box>
            ))}
          </section>

          <section className={styles.section}>
            <Eyebrow>Buy · {status.data.buy.length}</Eyebrow>
            {status.data.buy.length === 0 && (
              <Text tone="muted" size="sm" as="p">
                Nothing to buy.
              </Text>
            )}
            {status.data.buy.length > 0 && status.data.buy_cost.as_of && (
              <Text size="sm" as="p" title={costCaption(status.data.buy_cost)}>
                About <span className={styles.num}>{formatUsd(status.data.buy_cost.usd_cents)}</span> at the cheapest
                printing of each card.
              </Text>
            )}
            {status.data.buy.map((c) => (
              <div key={c.name} className={styles.row}>
                <span className={styles.name}>
                  <CardRef name={c.name} />
                  {c.quantity > 1 && <span className={styles.qty}> ×{c.quantity}</span>}
                </span>
                <Price cents={c.usd_cents} column />
              </div>
            ))}
          </section>
        </>
      )}

      {missing.length > 0 && (
        <div>
          <Button onClick={() => void exportMissing()}>Export the missing cards</Button>
        </div>
      )}

      <Text tone="faint" size="xs" as="p">
        Only you can set this, and agents never see it, so it cannot tilt what they recommend.
      </Text>

      {confirming && (
        <ConfirmDialog
          title="Mark every card as in the paper deck?"
          confirmLabel="Mark all in paper"
          onClose={() => setConfirming(false)}
          onConfirm={() => {
            setConfirming(false);
            void run(() => api.setAllInPaper(deckId, true));
          }}
        >
          Every card in the deck, not the maybe list, is marked as physically in the deck. Then mark only the
          exceptions as missing.
        </ConfirmDialog>
      )}
    </>
  );
}
