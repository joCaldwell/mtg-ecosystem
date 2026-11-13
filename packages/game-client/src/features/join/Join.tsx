import { Link } from 'react-router';
import type { ReadSnapshot } from '../../session/controller.ts';
import { connectionDetail, connectionLabel } from '../../session/status.ts';
import styles from './Join.module.css';

export function Join({ snapshot, onImport, onStop }: { snapshot: ReadSnapshot; onImport(file: File): void; onStop(): void }) {
  return <section className={styles.join}>
    <div className={styles.intro}><span className="eyebrow">Local session / read only</span><h1>Join a Commander table</h1><p>Open the seat file your local operator exported for you. It connects this browser to your permitted view of the table.</p></div>
    <div className={styles.import}>
      <label htmlFor="seat-file">Single-seat bootstrap file</label>
      <input id="seat-file" type="file" accept=".json,application/json" aria-describedby="file-help" onChange={event => {
        const file = event.currentTarget.files?.[0];
        event.currentTarget.value = '';
        if (file) onImport(file);
      }} />
      <p id="file-help">Choose one seat’s exported JSON file, up to 4 KiB. Refreshing requires reimport. Use a separate browser profile for each player.</p>
    </div>
    <div className={styles.state} role="status" aria-live="polite"><h2>{connectionLabel(snapshot)}</h2><p>{connectionDetail(snapshot)}</p></div>
    <div className={styles.actions}>
      {snapshot.read && <Link to="/table">Open live table <span aria-hidden="true">→</span></Link>}
      {snapshot.connection !== 'idle' && <button onClick={onStop}>Release seat and clear view</button>}
      <Link to="/fixtures/opening">Inspect the fixture</Link>
    </div>
    <p className={styles.note}>This build reads the table and lets you inspect cards. Playing choices and persistent command recovery are separate work.</p>
  </section>;
}
