import type { ReactNode } from "react";
import { TableHeader } from "@multiverse/game-ui";
import { href } from "../../app/routes.ts";
import { Icon } from "../../ui/index.ts";
import styles from "./PlaytestTable.module.css";

export function PlaytestHeader({ deckId, title, status, actions }: {
  deckId: number;
  title: string;
  status?: ReactNode;
  actions?: ReactNode;
}) {
  return <TableHeader className={styles.header} title={title} status={status} actions={actions} leading={
    <a className={styles.deckLink} href={href({ page: "deck", deckId })}
      aria-label="Back to deck overview" title="Back to deck overview">
      <Icon name="back" />
    </a>
  } />;
}
