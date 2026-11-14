import surface from './surface.module.css';
import type { CardView } from './table-model.ts';
import { cardName } from './table-model.ts';
import styles from './CardFace.module.css';

export function CardFace({ card, selected, onInspect, compact = false }: {
  card: CardView;
  selected: boolean;
  compact?: boolean;
  onInspect: (id: string, trigger: HTMLButtonElement) => void;
}) {
  return <button type="button" className={`${surface.surface} ${styles.card} ${compact ? styles.compact : ''}`} aria-label={`Inspect ${cardName(card)}`}
    aria-pressed={selected} onClick={event => onInspect(card.inspectionId, event.currentTarget)}>
    <span className={styles.heading}>{cardName(card)}</span>
    <span className={styles.mana}>{card.printedMana === '' ? 'No printed mana cost' : card.printedMana ?? 'Mana cost unavailable'}</span>
    <span className={styles.text}>{card.oracleText === '' ? 'No printed rules text' : card.oracleText ?? 'Source text unavailable'}</span>
    <span className={styles.type}>{card.printedType ?? 'Type unavailable'}</span>
    <span className={styles.inspect}>Inspect card <span aria-hidden="true">↗</span></span>
  </button>;
}
