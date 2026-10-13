import type { ReactNode } from "react";
import type { CardObservation } from "../../domain/table.ts";
import { TextCard } from "./TextCard.tsx";
import styles from "./CardInspection.module.css";
/** Contextual tools belong to the host; the display is usable without decks. */
export function CardInspection({
  observation,
  context,
  children,
}: {
  observation: CardObservation;
  context: string;
  children?: ReactNode;
}) {
  return (
    <section className={styles.inspection} aria-label={context}>
      <p className={styles.context}>{context}</p>
      <TextCard observation={observation} full />
      {children}
    </section>
  );
}
