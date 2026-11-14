import type { DecisionRequest, ObjectLabel } from '../../decisions/controls.ts';
import type { DecisionIntent } from '../../session/command.ts';
import { liveDecisionBinding, originalDecisionIntent, type LiveDecisionDraft } from '../../session/live-decision.ts';
import { DecisionPanel } from '../decisions/DecisionPanel.tsx';
import styles from './LiveDecision.module.css';

export function LiveDecision({ draft, request, labels, enabled, onIntent }: {
  readonly draft: LiveDecisionDraft | null; readonly request: DecisionRequest | null;
  readonly labels: readonly ObjectLabel[]; readonly enabled: boolean;
  readonly onIntent: (intent: DecisionIntent) => void;
}) {
  if (!request) return <p>Waiting for your next permitted decision.</p>;
  const binding = draft && liveDecisionBinding(draft);
  if (!draft || !binding) return <p>Checking the saved choice and table identity before showing controls.</p>;
  const current = enabled && request.token === draft.request.token && request.player === draft.request.player;
  return <fieldset className={styles.decision} disabled={!current}>
    <legend>{request.request === 'mulligan' || request.request === 'bottom_opening' ? 'Opening hand actions' : 'Game decision'}</legend>
    <DecisionPanel live binding={binding} request={draft.request} context={draft.explanation ?? null} labels={labels} onComplete={completion => {
      if (!current) return;
      const intent = originalDecisionIntent(draft, completion);
      if (intent) onIntent(intent);
    }} />
  </fieldset>;
}
