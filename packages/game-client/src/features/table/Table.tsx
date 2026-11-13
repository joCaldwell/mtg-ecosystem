import { useRef, useState, type ReactNode } from 'react';
import { CardFace } from '../../cards/CardFace.tsx';
import { cardName, inspectableCards, type CardView, type TableView } from '../../presentation/table-model.ts';
import { Inspector } from '../inspector/Inspector.tsx';
import styles from './Table.module.css';

export function Table({ table, decisionControls }: { table: TableView; decisionControls?: ReactNode }) {
  const [selected, setSelected] = useState<string | null>(null);
  const trigger = useRef<HTMLButtonElement | null>(null);
  const decision = useRef<HTMLElement>(null);
  const card = inspectableCards(table).find(value => value.inspectionId === selected) ?? null;
  function inspect(id: string, button: HTMLButtonElement) { trigger.current = button; setSelected(id); }
  function close() { setSelected(null); trigger.current?.focus(); }
  function cards(items: readonly CardView[], empty: string) {
    return items.length ? <div className={styles.cards}>{items.map(item => <CardFace key={item.inspectionId} card={item} selected={selected === item.inspectionId} onInspect={inspect} />)}</div> : <p className={styles.empty}>{empty}</p>;
  }
  return <>
    <div className={styles.orientation}>
      <div><span className="eyebrow">Commander / {table.live ? 'live table · opening actions' : 'inspection fixture'}</span><h1>{table.title}</h1></div>
      <button onClick={() => decision.current?.focus()}>Jump to decision <span aria-hidden="true">↓</span></button>
    </div>
    <div className={styles.layout}>
      <div className={styles.workspace}>
        <section className={styles.seats} aria-label="Players">
          {table.seats.map(seat => <article className={styles.seat} key={seat.label}>
            <div className={styles.seatTop}><h2>{seat.label}{seat.isViewer && <span> · You</span>}</h2><span className={styles.life}>{seat.life ?? '—'}<small>life</small></span></div>
            <p>Hand {seat.handCount ?? '—'} <span aria-hidden="true">·</span> Library {seat.libraryCount ?? '—'}</p>
            {seat.commandZone.map(commander => <button key={commander.inspectionId} className={styles.commanderLink}
              aria-label={`Inspect ${cardName(commander)} for ${seat.label}`} aria-pressed={selected === commander.inspectionId}
              onClick={event => inspect(commander.inspectionId, event.currentTarget)}>{cardName(commander)}</button>)}
          </article>)}
        </section>
        <section className={styles.battlefield} aria-labelledby="battlefield-title">
          <div className={styles.sectionHeading}><h2 id="battlefield-title">Battlefield</h2><span>{table.phase ?? 'Phase unavailable'}</span></div>
          <div className={styles.turnline}><span>Turn {table.turn ?? '—'}</span><span>Priority: {table.priority ?? 'not supplied'}</span></div>
          {table.seats.some(seat => seat.battlefield.length) ? table.seats.map(seat => <div key={seat.label}>{seat.battlefield.length > 0 && <><h3>{seat.label}</h3>{cards(seat.battlefield, '')}</>}</div>)
            : <div className={styles.boardEmpty}><div aria-hidden="true" className={styles.tableMark}>M</div><p>No battlefield objects in this view</p><span>Inspect the visible cards below.</span></div>}
          {!!table.combat?.length && <div><h3>Observed combat</h3><ul>{table.combat.map(line => <li key={line}>{line}</li>)}</ul></div>}
        </section>
        <div className={styles.lower}>
          <section className={styles.hand} aria-labelledby="hand-title"><div className={styles.sectionHeading}><h2 id="hand-title">Your hand</h2><span>Visible to your seat</span></div>{cards(table.hand, 'No hand contents supplied.')}</section>
          <section className={styles.command} aria-labelledby="command-title"><h2 id="command-title">Command zone</h2>{cards(table.commandZone, 'No command-zone objects supplied.')}</section>
        </div>
        <section ref={decision} tabIndex={-1} className={styles.decision} aria-labelledby="decision-title">
          <span className="eyebrow">Decision inspector</span>
          <h2 id="decision-title">{table.decision?.title ?? 'No decision supplied'}</h2>
          <p>{table.decision?.description ?? (table.live ? 'No decision in the latest permitted observation.' : 'The table is not connected to a game.')}</p>
          {table.decision && <ul>{table.decision.details.map(detail => <li key={detail}>{detail}</li>)}</ul>}
          {decisionControls ?? <p className={styles.readOnly}>{table.live ? 'Opening controls unavailable.' : 'Read-only fixture. Choices cannot be submitted.'}</p>}
        </section>
        <section className={styles.stack} aria-labelledby="stack-title"><h2 id="stack-title">Stack <span>{table.stack.length}</span></h2>{cards(table.stack, 'No stack objects in this view.')}</section>
        {!!table.otherObjects?.length && <section className={styles.stack} aria-label="Other permitted objects"><h2>Other permitted objects</h2>{cards(table.otherObjects, '')}</section>}
        <p className={styles.provenance}>{table.sourceLabel}</p>
      </div>
      <Inspector card={card} onClose={close} />
    </div>
  </>;
}
