import type { ReactNode } from "react";
import { CardRef, ManaCost } from "../../../mtg/index.ts";
import { Chip } from "../../../ui/index.ts";
import { useDeckUi } from "../DeckUi.tsx";
import styles from "./CardLine.module.css";

export interface CardLineProps {
  oracleId: string;
  name: string;
  quantity?: number;
  cost?: string | null;
  /** Colours the whole line as a change coming in or going out. */
  tone?: "add" | "cut";
  /** Names the change in a chip, so the colour is not the only signal. */
  label?: string;
  /** Muted text at the right edge. */
  detail?: ReactNode;
}

/** One card in a list of differences: a line, with the card openable in the side panel. */
export function CardLine({ oracleId, name, quantity = 1, cost, tone, label, detail }: CardLineProps) {
  const { openCardPanel, closeDialog } = useDeckUi();
  // The side panel is behind the dialog, so opening a card closes it.
  const open = () => {
    openCardPanel({ oracleId, name });
    closeDialog();
  };
  return (
    <li className={`${styles.line} ${tone ? styles[tone] : ""}`}>
      {label ? (
        <Chip tone={tone ?? "neutral"} className={styles.label}>
          {label}
        </Chip>
      ) : null}
      <span className={styles.name}>
        {quantity > 1 ? <span className={styles.qty}>{quantity}×</span> : null}
        <CardRef name={name} onOpen={open} />
      </span>
      {cost ? <ManaCost cost={cost} /> : null}
      {detail ? <span className={styles.detail}>{detail}</span> : null}
    </li>
  );
}
