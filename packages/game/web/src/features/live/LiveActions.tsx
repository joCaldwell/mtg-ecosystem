import { useId, useState } from 'react';
import type { ActionQuerySnapshot } from '../../session/action-query.ts';
import type { LiveActionDraft, LiveActionScope } from '../../session/live-action.ts';
import { actionDescription, actionResponse } from '../../session/live-action.ts';
import styles from './LiveActions.module.css';

const messages: Record<ActionQuerySnapshot['phase'], string> = {
  idle: '', loading: 'Discovering actions.',
  incomplete: 'Partial results. Discovery is continuing; this is not a complete menu.',
  complete: 'Discovery complete.',
  stale: 'The decision changed. Discover actions again.', unavailable: 'Discovery is unavailable for this decision.',
  cancelled: 'Discovery stopped. No game command was cancelled.', limited: 'Discovery reached a local limit. Results are not a complete menu.',
  uncertain: 'Discovery could not finish. Results cannot be submitted.',
};
/** Native buttons/list, no ARIA menu keyboard model or focus stealing on pages. */
export function LiveActions({ snapshot, draft, enabled, objects, onStart, onCancel, onSubmit }: {
  readonly snapshot: ActionQuerySnapshot | null; readonly draft: LiveActionDraft | null; readonly enabled: boolean;
  readonly objects: readonly { readonly handle: string; readonly name: string | null }[];
  readonly onStart: (scope: LiveActionScope) => void; readonly onCancel: (generation: number) => void;
  readonly onSubmit: (draft: LiveActionDraft, index: number) => void;
}) {
  const heading = useId(), scopeId = useId();
  const [object, setObject] = useState('');
  const scopeExists = !object || objects.some(candidate => candidate.handle === object);
  const phase = snapshot?.phase ?? 'idle';
  const busy = phase === 'loading' || phase === 'incomplete';
  const ready = enabled && phase === 'complete' && draft?.generation === snapshot?.generation;
  return <section className={styles.actions} aria-labelledby={heading}>
    <h3 id={heading}>Available actions</h3>
    <label htmlFor={scopeId}>Discovery scope</label>
    <select id={scopeId} value={object} disabled={busy} onChange={event => setObject(event.target.value)}>
      <option value="">Entire decision window</option>
      {objects.map(candidate => <option key={candidate.handle} value={candidate.handle}>{candidate.name ?? 'Unidentified object'} · {candidate.handle}</option>)}
      {!scopeExists && <option value={object}>Previous object is no longer available</option>}
    </select>
    <div className={styles.buttons}>
      <button type="button" aria-disabled={!enabled || busy || !scopeExists} onClick={() => { if (enabled && !busy && scopeExists) onStart(object ? { scope: 'object', object } : { scope: 'window' }); }}>Discover actions</button>
      <button type="button" aria-disabled={!snapshot || !busy} onClick={() => { if (snapshot && busy) onCancel(snapshot.generation); }}>Stop discovery</button>
    </div>
    <p role="status" className={styles.message}>{messages[phase]}</p>
    {busy && <progress aria-label="Discovering available actions" />}
    {snapshot?.beginnings.length ? <ol className={styles.list}>{snapshot.beginnings.map((beginning, index) => {
      const response = actionResponse(beginning, snapshot.context, snapshot.scope);
      return <li key={index}><button type="button" disabled={!response} aria-disabled={!ready || !response} onClick={() => { if (ready && response && draft) onSubmit(draft, index); }}>{actionDescription(beginning, objects)}</button>
        {beginning.label !== 'unavailable' && beginning.label.available.source?.text && <p>{beginning.label.available.source.text}</p>}</li>;
    })}</ol> : phase === 'complete' ? <p>No beginnings were supplied for this scope.</p> : null}
    {snapshot?.cancellation === 'pending' && <p>Releasing discovery scratch.</p>}
    {snapshot?.cancellation === 'unknown' && <p>Scratch cancellation is unconfirmed; the host expires it. This does not affect command recovery.</p>}
  </section>;
}
