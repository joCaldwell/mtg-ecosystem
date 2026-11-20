import { OpeningTablePreview } from "@multiverse/game/preview";
import { href } from "../../app/routes.ts";
import { ButtonLink } from "../../ui/index.ts";
import styles from "./GamePreviewPage.module.css";

export function GamePreview({ deckId, deckName }: { deckId?: number; deckName?: string }) {
  return <OpeningTablePreview title="Table preview" toolbar={<div className={styles.actions}>
    <span className={styles.sample}>Sample game</span>
    {deckId !== undefined && <ButtonLink variant="ghost" href={href({ page: "deck", deckId })}>Edit deck</ButtonLink>}
    <ButtonLink title={deckName ? `Return to setup with ${deckName}` : undefined} href={href({ page: "play", deckId })}>Back to setup</ButtonLink>
  </div>} />;
}
