import { CardPrint } from './CardPrint.tsx';
import { CardRulesPreview, CardSurface } from './CardSurface.tsx';
import { cardName, type CardView } from './table-model.ts';
import styles from './CardFace.module.css';

export function CardFace({ card, selected, onInspect, compact = false }: {
  card: CardView;
  selected: boolean;
  compact?: boolean;
  onInspect: (id: string, trigger: HTMLButtonElement) => void;
}) {
  const label = `Inspect ${cardName(card)}${card.tapped ? ' (tapped)' : ''}`;
  return <CardSurface as="button" type="button" className={`${styles.card} ${compact ? styles.compact : ''}`} aria-label={label}
    aria-pressed={selected} onClick={event => onInspect(card.inspectionId, event.currentTarget)}>
    <span className={styles.heading}>{cardName(card)}</span>
    {card.printedMana !== '' && <span className={styles.mana}>{card.printedMana ?? 'Mana cost unavailable'}</span>}
    <span className={styles.type}>{card.printedType ?? 'Type unavailable'}</span>
    {!compact && <>
      <CardRulesPreview className={styles.text}>{card.oracleText === '' ? 'No printed rules text' : card.oracleText ?? 'Source text unavailable'}</CardRulesPreview>
      <CardPrint source={card.printImage} className={styles.print} />
    </>}
    {card.tapped && <span className={styles.tapped}>Tapped</span>}
  </CardSurface>;
}
