import { useEffect, useRef } from 'react';
import type { CardView } from '../../presentation/table-model.ts';
import { cardName } from '../../presentation/table-model.ts';
import { FactList } from '../../ui/FactList.tsx';
import styles from './Inspector.module.css';

export function Inspector({ card, onClose }: { card: CardView | null; onClose: () => void }) {
  const heading = useRef<HTMLHeadingElement>(null);
  // Only explicit user selection mounts/changes this inspector in the fixture UI.
  // Live remote updates must not use this focus effect to steal typing focus.
  useEffect(() => { if (card) heading.current?.focus(); }, [card?.inspectionId]);
  return <aside className={styles.inspector} aria-label="Card inspector" onKeyDown={event => {
    if (event.key === 'Escape' && card) { event.preventDefault(); onClose(); }
  }}>
    <div className={styles.top}><span className="eyebrow">Card inspector</span>{card && <button onClick={onClose}>Close</button>}</div>
    {card ? <>
      <h2 ref={heading} tabIndex={-1} className={styles.title}>{cardName(card)}</h2>
      <p className={styles.zone}>{card.zoneLabel}</p>
      <div className={styles.face}>
        <p className={styles.mana}>{card.printedMana ?? 'Mana cost unavailable'}</p>
        <p className={styles.type}>{card.printedType ?? 'Type unavailable'}</p>
        <p className={styles.oracle}>{card.oracleText ?? 'Source text is not available for this object.'}</p>
      </div>
      <p className={styles.caption}>Text view · artwork not loaded</p>
      <h3>Current characteristics</h3>
      {card.currentFacts.length > 0 ? <FactList className={styles.facts} items={card.currentFacts} />
        : <p className="muted">No current characteristics supplied.</p>}
      <details className={styles.source}><summary>Source & identity</summary><p>{card.sourceIdentity}</p><p>Object: {card.inspectionId}</p><p>Printed text is reference material. Current characteristics come from the permitted observation.</p></details>
    </> : <div className={styles.empty}><span aria-hidden="true">↖</span><h2>Look closer.</h2><p>Select a visible card to inspect its text and current characteristics.</p><p className="muted">Only information supplied for this seat appears here.</p></div>}
  </aside>;
}
