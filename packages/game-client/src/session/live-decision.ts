import type { DecisionBinding, DecisionCompletion, DecisionContext, DecisionRequest } from '../decisions/controls.ts';
import { bindingKey } from '../decisions/controls.ts';
import type { CommandContext, DecisionIntent } from './command.ts';

/** The sole App/read owner captures all three from one permitted observation.
 * Retain this context across a disabled/transient view; never refresh it in a
 * human completion callback. Explanation grants no command authority. */
export interface LiveDecisionDraft {
  readonly context: CommandContext;
  readonly request: DecisionRequest;
  readonly explanation?: DecisionContext | null;
}
export function liveDecisionBinding(draft: LiveDecisionDraft): DecisionBinding | null {
  const { context, request } = draft;
  if (context.token !== request.token || context.binding.seat !== String(request.player)) return null;
  return { origin: context.binding.origin, match: context.binding.match, seat: request.player, incarnation: context.incarnation, revision: context.revision, token: context.token };
}
export function originalDecisionIntent(draft: LiveDecisionDraft, completion: DecisionCompletion): DecisionIntent | null {
  const binding = liveDecisionBinding(draft);
  return binding && bindingKey(binding) === bindingKey(completion.binding)
    ? { kind: 'decision', source: 'human', context: draft.context, response: completion.response } : null;
}
