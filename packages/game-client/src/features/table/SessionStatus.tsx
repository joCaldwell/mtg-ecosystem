import type { ReadSnapshot } from '../../session/controller.ts';
import { connectionLabel, connectionDetail } from '../../session/status.ts';
import styles from './Table.module.css';

export function SessionStatus({ snapshot, onStop }: { snapshot: ReadSnapshot; onStop(): void }) {
  const outcome = snapshot.read?.outcome;
  return <section className={styles.sessionStatus} aria-label="Live session">
    <div role="status" aria-live="polite">
      <h2>{connectionLabel(snapshot)}</h2><p>{connectionDetail(snapshot)}</p>
      {snapshot.read?.lifecycle && <p>Host status: <strong>{snapshot.read.lifecycle.status.replaceAll('_', ' ')}</strong></p>}
      {outcome && <p>Rules outcome: <strong>{outcome.outcome === 'draw' ? 'Draw' : `Seat ${outcome.player + 1} wins`}</strong></p>}
      {snapshot.metadataMissing && <p>Presentation metadata unavailable. Observed identity and characteristics remain available.</p>}
    </div>
    {snapshot.connection !== 'idle' && <button onClick={onStop}>Release seat and clear view</button>}
  </section>;
}
