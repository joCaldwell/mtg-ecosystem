import type { ReadSnapshot } from '../../session/controller.ts';
import { connectionLabel, connectionDetail } from '../../session/status.ts';
import styles from './SessionStatus.module.css';

export function SessionStatus({ snapshot, onStop, diagnostics = false }: { snapshot: ReadSnapshot; onStop(): void; diagnostics?: boolean }) {
  const outcome = snapshot.read?.outcome;
  return <section className={styles.sessionStatus} aria-label="Live session">
    <div role="status" aria-live="polite">
      <h2 data-connected={snapshot.connection === 'ready' || undefined}>{connectionLabel(snapshot)}</h2>
      {snapshot.connection !== 'ready' && <p>{connectionDetail(snapshot)}</p>}
      {diagnostics && snapshot.read?.lifecycle && <p>Host status: <strong>{snapshot.read.lifecycle.status.replaceAll('_', ' ')}</strong></p>}
      {outcome && <p>Rules outcome: <strong>{outcome.outcome === 'draw' ? 'Draw' : `Seat ${outcome.player + 1} wins`}</strong></p>}
      {snapshot.metadataMissing && <p>Printed card details unavailable.</p>}
    </div>
    {snapshot.connection !== 'idle' && <button type="button" className={styles.release} onClick={onStop}>Release seat and clear view</button>}
  </section>;
}
