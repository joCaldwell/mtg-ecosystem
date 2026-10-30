// A token or emblem opened from the Tokens section, in place of card details:
// the same header with Back, then its picture, text and the cards that make it.

import type { Ref } from "react";
import { ColorIdentity, TokenDetail } from "../../../mtg/index.ts";
import { Chip, Icon, IconButton } from "../../../ui/index.ts";
import { useDeckList } from "../DeckListStore.tsx";
import { useDeckUi, type OpenCard } from "../DeckUi.tsx";
import { panelViewLabel } from "../panelViews.ts";
import { TokenProducers } from "./TokenProducers.tsx";
import panelStyles from "./DeckPanel.module.css";
import styles from "./CardPanel.module.css";

export function TokenPanel({ token: open, backRef, bodyRef }: { token: OpenCard; backRef: Ref<HTMLButtonElement>; bodyRef: Ref<HTMLDivElement> }) {
  const ui = useDeckUi();
  const token = useDeckList().tokens.tokens.find((t) => t.oracle_id === open.oracleId);
  return (
    <>
      <div className={panelStyles.navigation} data-inspecting>
        <div className={styles.header}>
          <IconButton ref={backRef} size="sm" label={`Back to ${panelViewLabel(ui.tab)}`} onClick={() => ui.openCardPanel(null)}><Icon name="back" /></IconButton>
          <h2 className={styles.name}>{open.name}</h2>
          {token && <div className={styles.status}><Chip>{token.kind === "emblem" ? "Emblem" : "Token"}</Chip></div>}
          {token?.colors && <span className={styles.cost}><ColorIdentity colors={token.colors} label="Colour" /></span>}
        </div>
      </div>
      <div ref={bodyRef} className={panelStyles.body} role="region" aria-label={`Token details for ${open.name}`} tabIndex={0} data-panel-body>
        {token && <TokenDetail token={token} producers={<TokenProducers token={token} />} />}
      </div>
    </>
  );
}
