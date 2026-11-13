import type { CommandContext, DecisionIntent } from '../../session/command.ts';
import type { DecisionRequest, ObjectLabel } from '../../decisions/controls.ts';
import { bindingKey, decisionSurface } from '../../decisions/controls.ts';
import { DecisionPanel } from '../decisions/DecisionPanel.tsx';

export interface OpeningDraft {
  readonly context: CommandContext;
  readonly request: DecisionRequest;
}
/** Permitted data and explicit callbacks only. The app retains the FULL original
 * context; this adapter never obtains a fresh context to stamp an old response. */
export function LiveOpening({ draft, request, labels, enabled, onIntent }: {
  readonly draft: OpeningDraft | null; readonly request: DecisionRequest | null;
  readonly labels: readonly ObjectLabel[]; readonly enabled: boolean;
  readonly onIntent: (intent: DecisionIntent) => void;
}) {
  if (!request) return <p>Waiting for your next permitted decision.</p>;
  if (request.request !== 'mulligan' && request.request !== 'bottom_opening') {
    const surface = decisionSurface(request, labels);
    return <div role="status"><h3>Opening actions complete or unavailable</h3><p>This build supports keeping, taking mulligans and ordering opening cards only.</p>
      <p>{request.request === 'priority' || request.request === 'mana' ? 'The action menu and payment choices are not yet enabled in this build.'
        : surface.status === 'supported' ? 'This choice is available in inspection fixtures, but is not enabled for live play.'
          : surface.status === 'context-missing' ? 'This choice needs more information about its source or effect before a control can be offered.'
            : 'Controls for this decision are not yet available.'}</p><p>No choice will be sent for this decision.</p></div>;
  }
  if (!draft) return <p>Checking the saved choice and table identity before opening controls.</p>;
  const { context } = draft;
  const binding = { origin: context.binding.origin, match: context.binding.match, seat: draft.request.player,
    incarnation: context.incarnation, revision: context.revision, token: context.token };
  return <fieldset disabled={!enabled} aria-label="Opening hand actions">
    <DecisionPanel live binding={binding} request={draft.request} labels={labels} onComplete={completion => {
      if (!enabled || bindingKey(completion.binding) !== bindingKey(binding)) return;
      onIntent({ kind: 'decision', source: 'human', context, response: completion.response });
    }} />
  </fieldset>;
}
