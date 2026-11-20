import surface from './surface.module.css';
import { useId, useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { CardFace } from './CardFace.tsx';
import { GameSeat } from './GameSeat.tsx';
import { TableHeader, TableMat, TablePrompt, TableRail, TableSectionHeading } from './TableChrome.tsx';
import { inspectableCards, type CardView, type TableView } from './table-model.ts';
import { CardInspector } from './CardInspector.tsx';
import styles from './GameTable.module.css';

export function GameTable({ table, decisionControls, choiceStatus, toolbar, diagnostics = false, headingLevel = 1 }: { table: TableView; decisionControls?: ReactNode; choiceStatus?: ReactNode; toolbar?: ReactNode; diagnostics?: boolean; headingLevel?: 1 | 2 }) {
  const id = useId();
  const [selected, setSelected] = useState<string | null>(null);
  const trigger = useRef<HTMLButtonElement | null>(null);
  const decision = useRef<HTMLElement>(null);
  const focusDecision = useRef(false);
  const card = inspectableCards(table).find(value => value.inspectionId === selected) ?? null;
  const inspecting = card !== null;
  // A removed object cannot silently reopen and move focus if its handle is
  // present in a later permitted observation.
  if (selected !== null && !card) setSelected(null);
  useLayoutEffect(() => {
    if (inspecting || !focusDecision.current) return;
    focusDecision.current = false;
    decision.current?.focus();
  }, [inspecting]);
  function inspect(id: string, button: HTMLButtonElement) { trigger.current = button; setSelected(id); }
  function close() {
    setSelected(null);
    // A live update can remove the originating object while inspection is open.
    // Restore to the exact button when retained, otherwise the decision heading.
    if (trigger.current?.isConnected) trigger.current.focus();
    else focusDecision.current = true;
  }
  function jumpToDecision() {
    if (inspecting) {
      focusDecision.current = true;
      setSelected(null);
      return;
    }
    decision.current?.scrollIntoView({ block: 'nearest' });
    decision.current?.focus({ preventScroll: true });
  }
  function cards(items: readonly CardView[], empty: string, compact = false) {
    return items.length ? <ul role="list" className={compact ? styles.cardList : styles.cards}>{items.map(item => <li key={item.inspectionId}><CardFace card={item} compact={compact} selected={selected === item.inspectionId} onInspect={inspect} /></li>)}</ul> : <p className={styles.empty}>{empty}</p>;
  }
  return <div className={styles.container}><div className={`${surface.surface} ${styles.table}`}>
    <TableHeader title={table.title} headingLevel={headingLevel}
      status={<div className={styles.turnline}><span>Turn <strong>{table.turn ?? '—'}</strong></span><span>{table.phase ?? 'Phase unavailable'}</span><span>Priority: <strong>{table.priority ?? 'not supplied'}</strong></span></div>}
      actions={<>
        <button type="button" className={surface.control} onClick={jumpToDecision}>Jump to decision <span aria-hidden="true">↓</span></button>
        {toolbar}
      </>} />
    <section className={styles.seats} aria-label="Players">
      {table.seats.map(seat => <GameSeat key={seat.label} seat={seat} hasPriority={seat.label === table.priority} selected={selected} onInspect={inspect} />)}
    </section>
    <div className={styles.layout}>
      <div className={styles.playSurface}>
        <TableMat className={styles.board}>
          <section className={styles.battlefield} aria-labelledby={`${id}-battlefield-title`}>
            <TableSectionHeading id={`${id}-battlefield-title`} title="Battlefield" />
            {table.seats.some(seat => seat.battlefield.length) ? table.seats.map(seat => seat.battlefield.length > 0 && <div className={styles.playerBoard} key={seat.label}><h3>{seat.label}</h3>{cards(seat.battlefield, '')}</div>)
              : <div className={styles.boardEmpty}><p>Battlefield is empty</p></div>}
            {!!table.combat?.length && <div className={styles.combat}><h3>Observed combat</h3><ul>{table.combat.map(line => <li key={line}>{line}</li>)}</ul></div>}
          </section>
          <section className={styles.command} aria-labelledby={`${id}-command-title`}><TableSectionHeading id={`${id}-command-title`} title="Command zone" />{cards(table.commandZone, 'Command zone is empty.')}</section>
          {!!table.otherObjects?.length && <section className={styles.other} aria-label="Other permitted objects"><TableSectionHeading title="Other permitted objects" />{cards(table.otherObjects, '', true)}</section>}
        </TableMat>
        <section className={styles.hand} aria-labelledby={`${id}-hand-title`}><TableSectionHeading id={`${id}-hand-title`} title="Your hand" count={table.hand.length} />{cards(table.hand, 'Your hand is empty.')}</section>
      </div>
      <TableRail className={styles.rail}>
        <div className={styles.activity} hidden={inspecting}>
          <TablePrompt sectionRef={decision} tabIndex={-1} className={styles.decision} aria-labelledby={`${id}-decision-title`}>
            <h2 id={`${id}-decision-title`}>{table.decision?.title ?? (table.live ? 'Waiting for a decision' : 'No decision')}</h2>
            {table.decision?.description && <p>{table.decision.description}</p>}
            {!decisionControls && table.decision && table.decision.details.length > 0 && <details className={styles.decisionDetails}><summary>Decision details</summary><ul>{table.decision.details.map(detail => <li key={detail}>{detail}</li>)}</ul></details>}
            {choiceStatus}
            {decisionControls ?? <p className={styles.readOnly}>{table.live ? 'Opening controls unavailable.' : 'Read-only fixture. Choices cannot be submitted.'}</p>}
          </TablePrompt>
          {table.stack.length > 0 && <section className={styles.stack} aria-labelledby={`${id}-stack-title`}><TableSectionHeading id={`${id}-stack-title`} title="Stack" count={table.stack.length} />{cards(table.stack, '', true)}</section>}
          {diagnostics && <details className={styles.provenance}><summary>View source</summary><p>{table.sourceLabel}</p></details>}
        </div>
        {card && <CardInspector card={card} onClose={close} diagnostics={diagnostics} />}
      </TableRail>
    </div>
  </div></div>;
}
