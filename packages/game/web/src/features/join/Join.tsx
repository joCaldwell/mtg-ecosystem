import { useId } from 'react';
import { Link } from 'react-router';
import type { ReadSnapshot } from '../../session/controller.ts';
import { connectionDetail, connectionLabel } from '../../session/status.ts';
import styles from './Join.module.css';

export function Join({ snapshot, onImport, onStop }: { snapshot: ReadSnapshot; onImport(file: File): void; onStop(): void }) {
  const id = useId();
  return <section className={styles.join}>
    <h1>Join a Commander table</h1>
    <div className={styles.import}>
      <label htmlFor={`${id}-seat-file`}>Single-seat bootstrap file</label>
      <input id={`${id}-seat-file`} name="seat-file" type="file" accept=".json,application/json" aria-describedby={`${id}-file-help`} onChange={event => {
        const file = event.currentTarget.files?.[0];
        event.currentTarget.value = '';
        if (file) onImport(file);
      }} />
      <p id={`${id}-file-help`}>Use the JSON file from your local operator (up to 4 KiB). Each player needs a separate browser profile.</p>
    </div>
    <div className={styles.state} role="status" aria-live="polite"><h2>{connectionLabel(snapshot)}</h2><p>{connectionDetail(snapshot)}</p></div>
    <div className={styles.actions}>
      {snapshot.read && <Link to="/table">Open live table <span aria-hidden="true">→</span></Link>}
      {snapshot.connection !== 'idle' && <button type="button" onClick={onStop}>Release seat and clear view</button>}
      <Link to="/fixtures/opening">Inspect the fixture</Link>
    </div>
  </section>;
}
