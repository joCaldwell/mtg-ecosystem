import type { CardView } from '../presentation/table-model.ts';
import { cardName } from '../presentation/table-model.ts';
import styles from './CardFace.module.css';

export function CardFace({ card, selected, onInspect }: {
  card: CardView;
  selected: boolean;
  onInspect: (id: string, trigger: HTMLButtonElement) => void;
}) {
  return <button className={styles.card} aria-label={`Inspect ${cardName(card)}`}
    aria-pressed={selected} onClick={event => onInspect(card.inspectionId, event.currentTarget)}>
    <span className={styles.heading}>{cardName(card)}</span>
    <span className={styles.mana}>{card.printedMana === '' ? 'No printed mana cost' : card.printedMana ?? 'Mana cost unavailable'}</span>
    <span className={styles.text}>{card.oracleText === '' ? 'No printed rules text' : card.oracleText ?? 'Source text unavailable'}</span>
    <span className={styles.type}>{card.printedType ?? 'Type unavailable'}</span>
    <span className={styles.inspect}>Inspect card <span aria-hidden="true">↗</span></span>
  </button>;
}
