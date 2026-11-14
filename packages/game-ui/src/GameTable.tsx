import surface from './surface.module.css';
import { useRef, useState, type ReactNode } from 'react';
import { CardFace } from './CardFace.tsx';
import { cardName, inspectableCards, type CardView, type TableView } from './table-model.ts';
import { CardInspector } from './CardInspector.tsx';
import styles from './GameTable.module.css';

export function GameTable({ table, decisionControls, choiceStatus }: { table: TableView; decisionControls?: ReactNode; choiceStatus?: ReactNode }) {
  const [selected, setSelected] = useState<string | null>(null);
  const trigger = useRef<HTMLButtonElement | null>(null);
  const decision = useRef<HTMLElement>(null);
  const card = inspectableCards(table).find(value => value.inspectionId === selected) ?? null;
  function inspect(id: string, button: HTMLButtonElement) { trigger.current = button; setSelected(id); }
  function close() { setSelected(null); trigger.current?.focus(); }
  function jumpToDecision() {
    decision.current?.scrollIntoView({ block: 'nearest' });
    decision.current?.focus({ preventScroll: true });
  }
  function cards(items: readonly CardView[], empty: string, compact = false) {
    return items.length ? <div className={compact ? styles.cardList : styles.cards}>{items.map(item => <CardFace key={item.inspectionId} card={item} compact={compact} selected={selected === item.inspectionId} onInspect={inspect} />)}</div> : <p className={styles.empty}>{empty}</p>;
  }
  return <div className={`${surface.surface} ${styles.table}`}>
    <div className={styles.orientation}>
      <div className={styles.title}><h1>{table.title}</h1><span>{table.live ? 'Live · opening actions' : 'Inspection fixture'}</span></div>
      <div className={styles.turnline} aria-label="Turn and priority"><span>Turn <strong>{table.turn ?? '—'}</strong></span><span>{table.phase ?? 'Phase unavailable'}</span><span>Priority: <strong>{table.priority ?? 'not supplied'}</strong></span></div>
      <button className={surface.control} onClick={jumpToDecision}>Jump to decision <span aria-hidden="true">↓</span></button>
    </div>
    <section className={styles.seats} aria-label="Players">
      {table.seats.map(seat => <article className={styles.seat} key={seat.label}>
        <div className={styles.seatTop}><h2>{seat.label}{seat.isViewer && <span> · You</span>}</h2><span className={styles.life}>{seat.life ?? '—'}<small>life</small></span></div>
        <p>Hand <b>{seat.handCount ?? '—'}</b><span aria-hidden="true"> · </span>Library <b>{seat.libraryCount ?? '—'}</b></p>
        <div className={styles.commanders}>{seat.commandZone.map(commander => <button key={commander.inspectionId} className={styles.commanderLink}
          aria-label={`Inspect ${cardName(commander)} for ${seat.label}`} aria-pressed={selected === commander.inspectionId}
          onClick={event => inspect(commander.inspectionId, event.currentTarget)}>{cardName(commander)}</button>)}</div>
      </article>)}
    </section>
    <div className={styles.layout}>
      <div className={styles.playSurface}>
        <div className={styles.board}>
          <section className={styles.battlefield} aria-labelledby="battlefield-title">
            <div className={styles.sectionHeading}><h2 id="battlefield-title">Battlefield</h2><span>Permitted view</span></div>
            {table.seats.some(seat => seat.battlefield.length) ? table.seats.map(seat => seat.battlefield.length > 0 && <div className={styles.playerBoard} key={seat.label}><h3>{seat.label}</h3>{cards(seat.battlefield, '')}</div>)
              : <div className={styles.boardEmpty}><p>No battlefield objects in this view</p></div>}
            {!!table.combat?.length && <div className={styles.combat}><h3>Observed combat</h3><ul>{table.combat.map(line => <li key={line}>{line}</li>)}</ul></div>}
          </section>
          <section className={styles.command} aria-labelledby="command-title"><div className={styles.sectionHeading}><h2 id="command-title">Command zone</h2><span>Your seat</span></div>{cards(table.commandZone, 'No command-zone objects supplied.', true)}</section>
          {!!table.otherObjects?.length && <section className={styles.other} aria-label="Other permitted objects"><h2>Other permitted objects</h2>{cards(table.otherObjects, '', true)}</section>}
        </div>
        <section className={styles.hand} aria-labelledby="hand-title"><div className={styles.sectionHeading}><h2 id="hand-title">Your hand <span aria-hidden="true">{table.hand.length}</span></h2><span>Visible to your seat</span></div>{cards(table.hand, 'No hand contents supplied.')}</section>
      </div>
      <div className={styles.activity}>
        <section ref={decision} tabIndex={-1} className={styles.decision} aria-labelledby="decision-title">
          <span className={surface.eyebrow}>Current decision</span>
          <h2 id="decision-title">{table.decision?.title ?? 'No decision supplied'}</h2>
          <p>{table.decision?.description ?? (table.live ? 'No decision in the latest permitted observation.' : 'The table is not connected to a game.')}</p>
          {table.decision && table.decision.details.length > 0 && <details className={styles.decisionDetails}><summary>Decision details</summary><ul>{table.decision.details.map(detail => <li key={detail}>{detail}</li>)}</ul></details>}
          {choiceStatus}
          {decisionControls ?? <p className={styles.readOnly}>{table.live ? 'Opening controls unavailable.' : 'Read-only fixture. Choices cannot be submitted.'}</p>}
        </section>
        <section className={styles.stack} aria-labelledby="stack-title"><div className={styles.sectionHeading}><h2 id="stack-title">Stack <span>{table.stack.length}</span></h2></div>{cards(table.stack, 'No stack objects in this view.', true)}</section>
        <details className={styles.provenance}><summary>View source</summary><p>{table.sourceLabel}</p></details>
      </div>
      <CardInspector card={card} onClose={close} />
    </div>
  </div>;
}
