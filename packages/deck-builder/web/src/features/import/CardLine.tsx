import type { ReactNode } from "react";
import { CardRef, ManaCost, useCardInspect } from "../../mtg/index.ts";
import { Chip } from "../../ui/index.ts";
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

/** One card in a list of differences: a line, with card inspection available without leaving the preview. */
export function CardLine({ name, quantity = 1, cost, tone, label, detail }: CardLineProps) {
  const inspect = useCardInspect();
  const open = () => inspect(name);
  return (
    <li data-import-change className={`${styles.line} ${tone ? styles[tone] : ""}`}>
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
