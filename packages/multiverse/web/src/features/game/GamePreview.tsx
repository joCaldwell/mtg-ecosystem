import { OpeningTablePreview } from "@multiverse/game/preview";
import { href } from "../../app/routes.ts";
import { ButtonLink } from "../../ui/index.ts";
import styles from "./GamePreviewPage.module.css";

export function GamePreview({ deckId, deckName }: { deckId?: number; deckName?: string }) {
  return <div className={styles.preview}>
    <header className={styles.header}><a className={styles.brand} href={href({ page: "home" })}>Multiverse</a><h1>Table preview</h1>
      <div className={styles.actions}><ButtonLink href={href({ page: "play", deckId })}>Back to setup</ButtonLink>{deckId !== undefined && <ButtonLink href={href({ page: "deck", deckId })}>Edit deck</ButtonLink>}</div></header>
    <p className={styles.notice}>{deckName ? `${deckName} is selected for setup. This captured sample does not use that deck.` : "This table uses a captured sample."} Choices stay local. No game starts.</p>
    <div className={styles.table}><OpeningTablePreview headingLevel={2} /></div>
  </div>;
}
