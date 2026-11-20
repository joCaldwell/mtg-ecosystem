import { cardName, type SeatView } from './table-model.ts';
import styles from './GameSeat.module.css';

export function GameSeat({ seat, hasPriority, selected, onInspect }: {
  seat: SeatView;
  hasPriority: boolean;
  selected: string | null;
  onInspect: (id: string, trigger: HTMLButtonElement) => void;
}) {
  return <article className={styles.seat} data-viewer={seat.isViewer || undefined} data-priority={hasPriority || undefined}>
    <div className={styles.seatTop}>
      <h2>{seat.label}{seat.isViewer && <span> · You</span>}</h2>
      <span className={styles.life}>{seat.life ?? '—'}<small>life</small></span>
    </div>
    <div className={styles.seatDetails}>
      {seat.commandZone.length > 0 && <div className={styles.commanders}>
        {seat.commandZone.map(commander => <button type="button" key={commander.inspectionId} className={styles.commanderLink}
          aria-label={`Inspect ${cardName(commander)} for ${seat.label}`} aria-pressed={selected === commander.inspectionId}
          onClick={event => onInspect(commander.inspectionId, event.currentTarget)}>
          {cardName(commander)}
        </button>)}
      </div>}
      <p>Hand <b>{seat.handCount ?? '—'}</b><span aria-hidden="true"> · </span>Library <b>{seat.libraryCount ?? '—'}</b></p>
    </div>
  </article>;
}
