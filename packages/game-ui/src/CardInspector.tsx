import surface from './surface.module.css';
import { useEffect, useRef } from 'react';
import type { CardView } from './table-model.ts';
import { cardName } from './table-model.ts';
import { CardPrint } from './CardPrint.tsx';
import { FactList } from './FactList.tsx';
import styles from './CardInspector.module.css';

export function CardInspector({ card, onClose, diagnostics = false }: { card: CardView | null; onClose: () => void; diagnostics?: boolean }) {
  const heading = useRef<HTMLHeadingElement>(null);
  // Only explicit user selection mounts/changes this inspector in the fixture UI.
  // Live remote updates must not use this focus effect to steal typing focus.
  useEffect(() => { if (card) heading.current?.focus(); }, [card?.inspectionId]);
  return <aside className={`${surface.surface} ${styles.inspector}`} aria-label="Card inspector" onKeyDown={event => {
    if (event.key === 'Escape' && card) { event.preventDefault(); onClose(); }
  }}>
    {card ? <>
      <div className={styles.top}>
        <h2 ref={heading} tabIndex={-1} className={styles.title}>{cardName(card)}</h2>
        <button type="button" className={surface.control} onClick={onClose}>Close</button>
      </div>
      <p className={styles.zone}>{card.zoneLabel}</p>
      <div className={styles.face}>
        {card.printedMana !== '' && <p className={styles.mana}>{card.printedMana ?? 'Mana cost unavailable'}</p>}
        <p className={styles.type}>{card.printedType ?? 'Type unavailable'}</p>
        <p className={styles.oracle}>{card.oracleText === '' ? 'No printed rules text' : card.oracleText ?? 'Source text is not available for this object.'}</p>
      </div>
      <h3>Current characteristics</h3>
      {card.currentFacts.length > 0 ? <FactList className={styles.facts} items={card.currentFacts} />
        : <p className={surface.muted}>No current characteristics supplied.</p>}
      <CardPrint source={card.printImage} className={styles.print} />
      {diagnostics && <details className={styles.source}><summary>Source & identity</summary><p>{card.sourceIdentity}</p><p>Object: {card.inspectionId}</p><p>Printed text is reference material. Current characteristics come from the permitted observation.</p></details>}
    </> : <div className={styles.empty}><h2>No card selected</h2></div>}
  </aside>;
}
