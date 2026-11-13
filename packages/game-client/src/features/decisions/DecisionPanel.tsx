import { useState } from 'react';
import { bindingKey, completeDecision, decisionSurface, emptyDraft, reconcileDraft, type DecisionBinding, type DecisionCompletion, type DecisionRequest, type LocalDraft, type ObjectLabel } from '../../decisions/controls.ts';
import { LocalControls } from './LocalControls.tsx';
import styles from './decisions.module.css';

/** onComplete is explicit local human intent only. Future app composition must
 * preserve its original session context and pass through durable-send gates. */
export function DecisionPanel({ binding, request, labels, onComplete }: {
  readonly binding: DecisionBinding; readonly request: DecisionRequest;
  readonly labels: readonly ObjectLabel[]; readonly onComplete: (completion: DecisionCompletion) => void;
}) {
  const surface = decisionSurface(request, labels);
  const identity = bindingKey(binding);
  const fingerprint = JSON.stringify(surface);
  const [state, setState] = useState({ identity, fingerprint, draft: emptyDraft, announcement: '' });
  // Adjust local state during rendering so stale selections cannot be confirmed
  // for one effect frame. Keep DOM identity: remote changes never call focus().
  if (identity !== state.identity || fingerprint !== state.fingerprint) {
    setState({ identity, fingerprint,
      draft: identity === state.identity && surface.status === 'supported' ? reconcileDraft(state.draft, surface.control) : emptyDraft,
      announcement: identity === state.identity ? 'Decision options updated. Review the remaining draft.' : 'Decision changed. Previous draft discarded.' });
  }
  const draft: LocalDraft = identity === state.identity ? state.draft : emptyDraft;
  const bound = binding.token === request.token && binding.seat === request.player;
  return <section className={styles.panel} aria-label="Local decision draft">
    <p className={styles.announcement} role="status">{state.announcement}</p>
    {!bound ? <><h2>Decision binding unavailable</h2><p>No draft can be completed for this seat or token.</p></>
      : surface.status === 'supported' ? <LocalControls control={surface.control} draft={draft} onChange={next => setState({ ...state, draft: next })}
        onComplete={() => { const response = completeDecision(request, labels, draft); if (response) onComplete({ binding: { ...binding }, response }); }} />
        : <><h2>{surface.title}</h2><p>{surface.reason}</p></>}
    <p className={styles.boundary}>Local draft only. No game action is sent.</p>
  </section>;
}
