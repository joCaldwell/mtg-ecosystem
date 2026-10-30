// The tokens and emblems the deck makes, at the foot of the list, under the
// same kind of heading as the groups so it reads as part of the deck. The
// server derives them from the main deck, commanders and companion; the list
// draws them. While a filter is active a token shows when a card that makes
// it does, so the section narrows with the cards around it.

import type { DeckFilterMatches } from "../../../domain/deckSearch.ts";
import { filterTokens } from "../../../domain/tokens.ts";
import { TokenRow } from "../../../mtg/index.ts";
import { useDeckList } from "../DeckListStore.tsx";
import { useDeckUi } from "../DeckUi.tsx";
import { GroupHeader } from "./GroupHeader.tsx";
import styles from "./TokenList.module.css";

// Short lists stay together in one column, as a group does; longer ones flow on.
const LONG_LIST = 12;

export function TokenList({ filter }: { filter?: DeckFilterMatches }) {
  const { tokens: { tokens, status } } = useDeckList();
  const ui = useDeckUi();
  const filtering = !!filter?.active;
  const shown = filterTokens(tokens, filter);
  if (filtering && shown.length === 0) return null;

  const line = !status.ready ? "Token list not available yet." : tokens.length === 0 ? "This deck makes no tokens." : null;
  return (
    <div className={styles.cols}>
      <section className={shown.length > LONG_LIST ? `${styles.tokens} ${styles.long}` : styles.tokens} aria-label="Tokens">
        <GroupHeader title="Tokens" count={line ? undefined : tokens.length} matched={filtering ? shown.length : undefined} />
        {line
          ? <p className={styles.line}>{line}</p>
          : shown.map((t) => {
              const open = !!ui.openCard?.token && ui.openCard.oracleId === t.oracle_id;
              return (
                <TokenRow
                  key={t.oracle_id}
                  token={t}
                  open={open}
                  filterMatch={filtering}
                  onOpen={() => ui.openCardPanel(open ? null : { oracleId: t.oracle_id, name: t.name, token: true })}
                />
              );
            })}
      </section>
    </div>
  );
}
